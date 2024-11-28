use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use tempfile::tempdir;
use uuid::Uuid;

use crate::config::load;
use crate::config::load_one;
use crate::config::save;
use crate::config::Config;
use crate::config::CONFIG_FILE;

fn random_config() -> Config {
    Config {
        api_url: Some(Uuid::new_v4().to_string()),
        account_id: Some(Uuid::new_v4().to_string()),
        old_account_ids: vec![Uuid::new_v4().to_string()],
        local_tunnels_ids: vec![Uuid::new_v4().to_string()],
        ..Default::default()
    }
}

#[test]
fn load_no_config() {
    let dir = Path::new("/var/empty/");
    assert_eq!(load_one(dir).unwrap(), Default::default());
}

#[test]
fn load_config() {
    let config = random_config();

    let dir = tempdir().unwrap();

    save(dir.as_ref(), &config).unwrap();

    assert_eq!(load_one(dir.as_ref()).unwrap(), Some(config));
}

#[test]
fn load_invalid_json() {
    let dir = tempdir().unwrap();
    let file = dir.as_ref().join(CONFIG_FILE);

    let corrupted = "{";
    fs::write(&file, corrupted).unwrap();

    // Load returns a default config.
    assert_eq!(load_one(dir.as_ref()).unwrap(), Some(Default::default()));

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
    assert_eq!(load_one(dir.as_ref()).unwrap(), Some(Default::default()));

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
    assert_eq!(load_one(dir.as_ref()).unwrap(), Some(Default::default()));

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
    assert_eq!(load_one(dir.as_ref()).unwrap(), Some(config));
}

#[test]
fn test_ignore_invalid_fields() {
    let example_config = Config {
        api_url: Some("myapi".into()),
        account_id: Some("myaccount".into()),
        old_account_ids: vec!["oldaccount".into()],
        local_tunnels_ids: vec!["oldtunnel".into()],
        exit: (),
        in_new_account_flow: true,
        cached_auth_token: Some("myauth".into()),
        pinned_exits: vec!["mypinnedexit".into()],
        last_chosen_exit: Some("mylastexit".into()),
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

#[test]
fn migrate_path_none() {
    let dir = tempdir().unwrap();
    let old_dir = dir.path().join("old");
    let new_dir = dir.path().join("new");

    assert_eq!(load(new_dir.as_ref(), old_dir.as_ref()).unwrap(), Default::default());
}

#[test]
fn migrate_path_old_only() {
    let config = random_config();

    let dir = tempdir().unwrap();
    let old_dir = dir.path().join("old");
    let new_dir = dir.path().join("new");

    save(old_dir.as_ref(), &config).unwrap();

    assert_eq!(load(new_dir.as_ref(), old_dir.as_ref()).unwrap(), config);
}

#[test]
fn migrate_path_new_only() {
    let config = random_config();

    let dir = tempdir().unwrap();
    let old_dir = dir.path().join("old");
    let new_dir = dir.path().join("new");

    save(new_dir.as_ref(), &config).unwrap();

    assert_eq!(load(new_dir.as_ref(), old_dir.as_ref()).unwrap(), config);
}

#[test]
fn migrate_path_both() {
    let config_new = random_config();
    let config_old = random_config();

    let dir = tempdir().unwrap();
    let old_dir = dir.path().join("old");
    let new_dir = dir.path().join("new");

    save(new_dir.as_ref(), &config_new).unwrap();
    save(old_dir.as_ref(), &config_old).unwrap();

    assert_eq!(load(new_dir.as_ref(), old_dir.as_ref()).unwrap(), config_new);
}
