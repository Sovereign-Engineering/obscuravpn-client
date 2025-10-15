use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use obscuravpn_api::cmd::ExitList;
use obscuravpn_api::types::AccountId;
use obscuravpn_api::types::CityCode;
use obscuravpn_api::types::CountryCode;
use obscuravpn_api::types::OneExit;
use tempfile::tempdir;
use uuid::Uuid;

use crate::config::CONFIG_FILE;
use crate::config::Config;
use crate::config::PinnedLocation;
use crate::config::cached::ConfigCached;
use crate::config::load;
use crate::config::save;
use crate::exit_selection::ExitSelector;

fn random_config() -> Config {
    Config {
        api_url: Some(Uuid::new_v4().to_string()),
        account_id: Some(AccountId::from_string_unchecked(Uuid::new_v4().to_string())),
        old_account_ids: vec![AccountId::from_string_unchecked(Uuid::new_v4().to_string())],
        local_tunnels_ids: vec![Uuid::new_v4().to_string()],
        ..Default::default()
    }
}

#[test]
fn load_no_config() {
    let dir = Path::new("/var/empty/");
    assert_eq!(load(dir, None).unwrap(), Default::default());
}

#[test]
fn load_config() {
    let config = random_config();

    let dir = tempdir().unwrap();

    save(dir.as_ref(), &config).unwrap();

    assert_eq!(load(dir.as_ref(), None).unwrap(), config);
}

#[test]
fn load_invalid_json() {
    let dir = tempdir().unwrap();
    let file = dir.as_ref().join(CONFIG_FILE);

    let corrupted = "{";
    fs::write(&file, corrupted).unwrap();

    // Load returns a default config.
    assert_eq!(load(dir.as_ref(), None).unwrap(), Default::default());

    let backup_files = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("config-backup-"))
        .collect::<Vec<_>>();

    assert_eq!(backup_files.len(), 1);
    let backup = &backup_files[0];

    // Original file was saved.
    let backup_contents = fs::read_to_string(backup.path()).unwrap();
    assert_eq!(backup_contents, corrupted);

    // Config file was eagerly updated to the default.
    let config_contents = fs::read_to_string(&file).unwrap();
    assert_ne!(config_contents, corrupted);
}

#[test]
fn load_empty() {
    let dir = tempdir().unwrap();
    let file = dir.as_ref().join(CONFIG_FILE);

    let empty = r#"{"removed_field":"abc"}"#;
    fs::write(&file, empty).unwrap();

    // Load returns a default config.
    assert_eq!(load(dir.as_ref(), None).unwrap(), Default::default());

    let backup_files = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("config-backup-"))
        .collect::<Vec<_>>();

    assert_eq!(backup_files.len(), 0);
}

#[test]
fn load_no_permission() {
    let config = random_config();

    let dir = tempdir().unwrap();

    save(dir.as_ref(), &config).unwrap();

    let file = dir.as_ref().join(CONFIG_FILE);

    // Mess up the file permissions so that reads fail.
    let original_permissions = fs::metadata(&file).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    permissions.set_mode(0o0);
    fs::set_permissions(&file, permissions).unwrap();

    // Load returns a default config.
    assert_eq!(load(dir.as_ref(), None).unwrap(), Default::default());

    let backup_files = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("config-backup-"))
        .collect::<Vec<_>>();

    assert_eq!(backup_files.len(), 1);
    let backup = &backup_files[0];
    let backup_path = backup.path();

    // Fix permissions to check that the backed up file is pristine.
    fs::set_permissions(&backup_path, original_permissions).unwrap();
    fs::rename(backup_path, file).unwrap();

    // The backup file contained the original config.
    assert_eq!(load(dir.as_ref(), None).unwrap(), config);
}

#[test]
fn test_ignore_invalid_fields() {
    let example_config = Config {
        api_host_alternate: Some("relay.example".into()),
        api_url: Some("myapi".into()),
        account_id: Some(AccountId::from_string_unchecked("myaccount".into())),
        old_account_ids: vec![AccountId::from_string_unchecked("oldaccount".into())],
        local_tunnels_ids: vec!["oldtunnel".into()],
        exit: (),
        in_new_account_flow: true,
        cached_auth_token: Some("myauth".into()),
        cached_exits: Some(ConfigCached::new(
            Arc::new(ExitList {
                exits: vec![OneExit {
                    id: "ABC-123".into(),
                    city_code: CityCode { country_code: CountryCode("ca".into()), city_code: "yyz".into() },
                    city_name: "Toronto".into(),
                    datacenter_id: 34,
                    provider_id: "foo123".into(),
                    provider_url: "https://servers.example/foo123".into(),
                    provider_name: "Cheap Server Rentals".into(),
                    provider_homepage_url: "https://example.com".into(),
                    tier: 0,
                }],
            }),
            super::cached::Version::artificial(),
        )),
        pinned_locations: vec![PinnedLocation { country_code: "CA".into(), city_code: "yyz".into(), pinned_at: SystemTime::UNIX_EPOCH }],
        last_chosen_exit: Some("mylastexit".into()),
        last_chosen_exit_selector: ExitSelector::City { city_code: CityCode { country_code: CountryCode("ca".into()), city_code: "yyz".into() } },
        last_exit_selector: ExitSelector::City { city_code: CityCode { country_code: CountryCode("ca".into()), city_code: "yyz".into() } },
        sni_relay: Some("relay.obscura.net".into()),
        wireguard_key_cache: Default::default(),
        use_wireguard_key_cache: (),
        cached_account_status: Default::default(),
        auto_connect: true,
        feature_flags: Default::default(),
        force_tcp_tls_relay_transport: (),
    };
    let example_json = match serde_json::to_value(&example_config).unwrap() {
        serde_json::Value::Object(m) => m,
        other => panic!("Expected map, got {:?}", other),
    };

    let corrupt_values = [
        serde_json::Value::Object(Default::default()),
        serde_json::Value::Number(7.into()),
        serde_json::Value::Null,
        serde_json::Value::Array(vec![serde_json::Value::Bool(true)]),
    ];

    for (field, _) in &example_json {
        let mut mutated = example_json.clone();
        for corrupt in &corrupt_values {
            eprintln!("Setting {field:?} = {corrupt:?}");
            mutated[field] = corrupt.clone();

            let reserialized = serde_json::to_string_pretty(&mutated).unwrap();
            let parsed = serde_json::from_str::<Config>(&reserialized).unwrap();
            let json_repr = match serde_json::to_value(&parsed).unwrap() {
                serde_json::Value::Object(m) => m,
                other => panic!("Expected map, got {:?}", other),
            };
            for (k, v) in json_repr {
                if &k == field {
                    continue;
                }
                assert_eq!(v, example_json[&k], "Field {k:?} doesn't match.");
            }
        }

        eprintln!("Removing {field:?}");
        mutated.remove(field);
        let reserialized = serde_json::to_string_pretty(&mutated).unwrap();
        let _ = serde_json::from_str::<Config>(&reserialized).unwrap();
    }
}
