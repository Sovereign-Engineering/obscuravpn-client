use std::net::IpAddr;
use winreg::{RegKey, enums::HKEY_LOCAL_MACHINE, types::ToRegValue};

// A fixed GUID ensures NRPT rules are cleaned up between runs without relying on the hardcoded Comment, Display arbitrary metadata.
const NRPT_RULE_GUID: &str = "fb157da8-6578-4f53-81ea-0a9168e96c1f";

const NRPT_COMMENT_VALUE: &str = "Redirect all DNS queries to nameservers provided by Obscura VPN";
const NRPT_DISPLAY_NAME_VALUE: &str = "Obscura VPN Rule";

const NRPT_RULES_PATH: &str = r"SYSTEM\CurrentControlSet\Services\Dnscache\Parameters\DnsPolicyConfig";

fn get_rule_path() -> String {
    format!(r"{NRPT_RULES_PATH}\{{{NRPT_RULE_GUID}}}")
}

fn set_nrpt_value(key: &RegKey, name: &str, value: &impl ToRegValue) -> Result<(), ()> {
    key.set_value(name, value)
        .map_err(|error| tracing::error!(message_id = "KhjZ3ZuG", ?error, name, "failed to set NRPT registry value"))
}

/// Creates an NRPT rule that forces all DNS queries (domain ".") through the specified name servers.
pub fn create_rule(name_servers: &[IpAddr]) -> Result<(), ()> {
    if name_servers.is_empty() {
        tracing::warn!(message_id = "lvns9NsX", "no name_servers provided");
        return Err(());
    }
    let name_servers_str = name_servers.iter().map(|ip| ip.to_string()).collect::<Vec<_>>().join(";");

    let rule_path = get_rule_path();

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (rule_key, _disposition) = hklm
        .create_subkey(&rule_path)
        .map_err(|error| tracing::error!(message_id = "zKKMdYSA", ?error, rule_path, "failed to create NRPT registry subkey"))?;
    tracing::debug!(?rule_key, rule_path, "opened NRPT registry subkey");
    set_nrpt_value(&rule_key, "Comment", &NRPT_COMMENT_VALUE)?;
    set_nrpt_value(&rule_key, "DisplayName", &NRPT_DISPLAY_NAME_VALUE)?;
    // See Section 2.2.2.2 of MS-GPNRPT,
    // https://learn.microsoft.com/openspecs/windows_protocols/ms-gpnrpt/8cc31cb9-20cb-4140-9e85-3e08703b4745
    set_nrpt_value(&rule_key, "ConfigOptions", &8u32)?;
    set_nrpt_value(&rule_key, "GenericDNSServers", &name_servers_str)?;
    set_nrpt_value(&rule_key, "IPSECCARestriction", &"")?;
    let all_domains = vec![".".to_string()];
    set_nrpt_value(&rule_key, "Name", &all_domains)?;
    set_nrpt_value(&rule_key, "Version", &2u32)?;

    tracing::info!(
        message_id = "pXtmTXfo",
        name_servers = %name_servers_str,
        "created NRPT rule"
    );

    Ok(())
}

/// Deletes the NRPT rule we created, identified by our fixed GUID.
///
/// Returns `Ok(true)` if the rule was deleted, `Ok(false)` if it didn't exist.
pub fn delete_rules() -> Result<bool, ()> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let rule_path = get_rule_path();

    match hklm.delete_subkey(&rule_path) {
        Ok(()) => {
            tracing::info!(message_id = "YkowItk3", "deleted NRPT rule");
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => {
            tracing::error!(message_id = "9X2pcgLU", ?error, rule_path, "failed to delete NRPT rule subkey");
            Err(())
        }
    }
}

#[test]
fn test_rule_path() {
    let path = get_rule_path();
    assert!(path.contains(&NRPT_RULE_GUID));
    println!("{path}");
}
