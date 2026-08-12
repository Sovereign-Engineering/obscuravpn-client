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
    #[serde(rename = "DotNETFramework")]
    pub dotnet_framework: Option<String>,
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
    #[serde(rename = "SourceID")]
    pub source_id: Option<String>,
    pub thermal_state: Option<String>,
    pub uptime_hours: Option<f64>,
}
