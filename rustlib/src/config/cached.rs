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

    pub fn revalidated(self, etag: Option<Vec<u8>>) -> Self {
        let version = match etag {
            Some(etag) => Version::ETag(etag),
            None => self.version,
        };
        ConfigCached { version, last_updated: SystemTime::now(), value: self.value }
    }

    pub fn staleness(&self) -> Duration {
        self.last_updated.elapsed().unwrap_or(Duration::ZERO)
    }

    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn version_bytes(&self) -> &[u8] {
        match &self.version {
            Version::Artificial(uuid) => uuid.as_bytes(),
            Version::ETag(items) => items,
        }
    }
}

impl<T> PartialEq for ConfigCached<Arc<T>> {
    fn eq(&self, other: &Self) -> bool {
        let Self { last_updated, value, version } = self;
        last_updated == &other.last_updated && Arc::as_ptr(value) == Arc::as_ptr(&other.value) && version == &other.version
    }
}

impl<T> Eq for ConfigCached<Arc<T>> {}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum Version {
    Artificial(Uuid),
    ETag(Vec<u8>),
}

impl Version {
    pub fn artificial() -> Self {
        Version::Artificial(Uuid::new_v4())
    }
}

impl std::fmt::Debug for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Version::Artificial(uuid) => write!(f, "Version::Artificial({})", uuid),
            Version::ETag(etag) => write!(f, "Version::ETag({:?})", String::from_utf8_lossy(etag)),
        }
    }
}
