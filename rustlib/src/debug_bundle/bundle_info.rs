use crate::version::release_version;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct BundleInfo {
    #[serde(rename = "AndroidSDK")]
    pub android_sdk: Option<i32>,
    pub app_version: String,
    pub boot_timestamp: Option<String>,
    pub brand: Option<String>,
    pub bundle_timestamp: Option<String>,
    pub build_number: Option<String>,
    pub desktop_environment: Option<String>,
    #[serde(rename = "DotNETFramework")]
    pub dotnet_framework: Option<String>,
    pub kernel_version: Option<String>,
    pub log_start_timestamp: Option<String>,
    pub low_power_mode: Option<bool>,
    pub model: Option<String>,
    #[serde(rename = "OSArchitecture")]
    pub os_architecture: Option<String>,
    #[serde(rename = "OSVersion")]
    pub os_version: Option<Vec<i32>>,
    #[serde(rename = "OSVersionString")]
    pub os_version_string: Option<String>,
    #[serde(rename = "PID")]
    pub pid: Option<i32>,
    pub process_architecture: Option<String>,
    pub process_name: Option<String>,
    pub process_path: Option<String>,
    pub processor_count_active: Option<i32>,
    pub processor_count_physical: Option<i32>,
    pub processor_name: Option<String>,
    #[serde(rename = "RAMAvailableGiB")]
    pub ram_available_gib: Option<f64>,
    #[serde(rename = "RAMLogicalGiB")]
    pub ram_logical_gib: Option<f64>,
    #[serde(rename = "RAMPhysicalGiB")]
    pub ram_physical_gib: Option<f64>,
    pub session_type: Option<String>,
    #[serde(rename = "SourceID")]
    pub source_id: Option<String>,
    pub thermal_state: Option<String>,
    pub uptime_hours: Option<f64>,
}

impl BundleInfo {
    pub fn collect(bundle_timestamp: String) -> Self {
        let exe = std::env::current_exe().ok();
        Self {
            app_version: release_version().to_owned(),
            boot_timestamp: boot_timestamp(),
            bundle_timestamp: Some(bundle_timestamp),
            brand: read_trimmed("/sys/class/dmi/id/sys_vendor"),
            desktop_environment: env_nonempty("XDG_CURRENT_DESKTOP"),
            kernel_version: read_trimmed("/proc/sys/kernel/osrelease"),
            model: read_trimmed("/sys/class/dmi/id/product_name"),
            os_architecture: read_trimmed("/proc/sys/kernel/arch"),
            os_version_string: os_release_pretty_name(),
            pid: i32::try_from(std::process::id()).ok(),
            process_architecture: Some(std::env::consts::ARCH.to_owned()),
            process_name: exe.as_deref().and_then(|path| Some(path.file_name()?.to_str()?.to_owned())),
            process_path: exe.as_deref().and_then(|path| Some(path.to_str()?.to_owned())),
            processor_count_active: std::thread::available_parallelism()
                .ok()
                .and_then(|count| i32::try_from(count.get()).ok()),
            processor_name: cpuinfo_model_name(),
            ram_available_gib: meminfo_gib("MemAvailable:"),
            ram_physical_gib: meminfo_gib("MemTotal:"),
            session_type: env_nonempty("XDG_SESSION_TYPE"),
            uptime_hours: uptime_hours(),
            ..Self::default()
        }
    }
}

fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn read_trimmed(path: &str) -> Option<String> {
    Some(std::fs::read_to_string(path).ok()?.trim().to_owned()).filter(|value| !value.is_empty())
}

fn os_release_pretty_name() -> Option<String> {
    let contents = std::fs::read_to_string("/etc/os-release").ok()?;
    let value = contents.lines().find_map(|line| line.strip_prefix("PRETTY_NAME="))?;
    Some(value.trim().trim_matches('"').to_owned()).filter(|value| !value.is_empty())
}

fn cpuinfo_model_name() -> Option<String> {
    let contents = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    let line = contents.lines().find(|line| line.starts_with("model name"))?;
    Some(line.split_once(':')?.1.trim().to_owned()).filter(|value| !value.is_empty())
}

fn meminfo_gib(field: &str) -> Option<f64> {
    let contents = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = contents.lines().find(|line| line.starts_with(field))?;
    let kib: f64 = line.strip_prefix(field)?.trim().strip_suffix("kB")?.trim().parse().ok()?;
    Some(kib / (1024.0 * 1024.0))
}

fn boot_timestamp() -> Option<String> {
    let contents = std::fs::read_to_string("/proc/stat").ok()?;
    let line = contents.lines().find_map(|line| line.strip_prefix("btime "))?;
    let secs: i64 = line.trim().parse().ok()?;
    Some(chrono::DateTime::from_timestamp(secs, 0)?.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

fn uptime_hours() -> Option<f64> {
    let contents = std::fs::read_to_string("/proc/uptime").ok()?;
    let secs: f64 = contents.split_whitespace().next()?.parse().ok()?;
    Some(secs / 3600.0)
}
