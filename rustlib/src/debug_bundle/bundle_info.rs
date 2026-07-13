use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleInfo {
    pub android_sdk: Option<i32>,
    pub app_version: String,
    pub boot_timestamp: Option<String>,
    pub brand: Option<String>,
    pub bundle_timestamp: Option<String>,
    pub dotnet_framework: Option<String>,
    pub low_power_mode: Option<bool>,
    pub memory_avail_gib: Option<f64>,
    pub memory_total_gib: Option<f64>,
    pub model: Option<String>,
    pub os_architecture: Option<String>,
    pub os_version: Option<String>,
    pub process_architecture: Option<String>,
    pub process_id: Option<i32>,
    pub process_name: Option<String>,
    pub process_path: Option<String>,
    pub processor_count_active: Option<i32>,
    pub processor_count_physical: Option<i32>,
    pub processor_name: Option<String>,
    pub thermal_state: Option<String>,
    pub uptime_hours: Option<f64>,
}
