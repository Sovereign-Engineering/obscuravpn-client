use std::time::SystemTime;

use base64::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use serde_with::serde_as;

#[serde_with::serde_as]
#[derive(derive_more::Debug, Deserialize, Serialize)]
pub struct CachedValue<T> {
    #[debug("{:?}", BASE64_STANDARD.encode(version))]
    #[serde_as(as = "serde_with::base64::Base64")]
    pub version: Vec<u8>,
    #[serde_as(as = "serde_with::TimestampSeconds")]
    pub last_updated: SystemTime,
    pub value: T,
}
