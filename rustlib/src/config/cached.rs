use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigCached<T> {
    version: Version,
    pub last_updated: SystemTime,
    pub value: T,
}

impl<T> ConfigCached<T> {
    pub fn new(value: T, version: Version) -> Self {
        ConfigCached { version, last_updated: SystemTime::now(), value }
    }

    pub fn etag(&self) -> Option<&[u8]> {
        match &self.version {
            Version::Artificial(_) => None,
            Version::ETag(items) => Some(items),
        }
    }

    pub fn staleness(&self) -> Duration {
        self.last_updated.elapsed().unwrap_or(Duration::ZERO)
    }

    pub fn version(&self) -> &[u8] {
        match &self.version {
            Version::Artificial(uuid) => uuid.as_bytes(),
            Version::ETag(items) => items,
        }
    }
}

impl<T> PartialEq for ConfigCached<Arc<T>> {
    fn eq(&self, other: &Self) -> bool {
        Arc::as_ptr(&self.value) == Arc::as_ptr(&other.value)
    }
}

impl<T> Eq for ConfigCached<Arc<T>> {}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Version {
    Artificial(Uuid),
    ETag(Vec<u8>),
}

impl Version {
    pub fn artificial() -> Self {
        Version::Artificial(Uuid::new_v4())
    }
}
