use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use serde_with::skip_serializing_none;
use strum::{EnumString, IntoStaticStr, VariantNames};

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Eq, Debug)]
#[skip_serializing_none]
#[serde(rename_all = "camelCase", default)]
pub struct FeatureFlags {
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub quic_frame_padding: Option<bool>,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub kill_switch: Option<bool>,
    #[serde(deserialize_with = "crate::serde_safe::deserialize")]
    pub tcp_tls_tunnel: Option<bool>,
    #[serde(flatten)]
    other: Map<String, Value>,
}

impl FeatureFlags {
    pub const KEYS: &'static [&'static str] = FeatureFlagKey::VARIANTS;

    pub fn set(&mut self, flag: &str, active: bool) {
        self.change(flag, active.then_some(true));
    }

    fn change(&mut self, flag: &str, value: Option<bool>) {
        let Ok(flag) = FeatureFlagKey::from_str(flag) else {
            tracing::error!("unknown feature flag: {:?}", flag);
            return;
        };
        match flag {
            FeatureFlagKey::QuicFramePadding => self.quic_frame_padding = value,
            FeatureFlagKey::KillSwitch => self.kill_switch = value,
            FeatureFlagKey::TcpTlsTunnel => self.tcp_tls_tunnel = value,
        }
    }
}

#[derive(VariantNames, Clone, Copy, EnumString, IntoStaticStr)]
#[strum(serialize_all = "camelCase")]
pub enum FeatureFlagKey {
    QuicFramePadding,
    KillSwitch,
    TcpTlsTunnel,
}

#[cfg(test)]
mod test {
    use super::FeatureFlags;

    #[test]
    fn check_flag_list() {
        let _: FeatureFlags = serde_json::from_str("{}").unwrap();
        for flag in FeatureFlags::KEYS {
            dbg!(flag);
            let feature_flags: FeatureFlags = serde_json::from_str(&format!(r#"{{ "{flag}": true }}"#)).unwrap();
            assert_eq!(feature_flags.other.len(), 0)
        }
    }
}
