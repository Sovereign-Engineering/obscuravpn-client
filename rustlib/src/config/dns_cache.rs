use crate::constants::DNS_CACHE_SEED;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsCache {
    entries: HashMap<String, Vec<SocketAddr>>,
}

impl DnsCache {
    pub fn get(&self, name: &str) -> Vec<SocketAddr> {
        self.entries.get(name).cloned().unwrap_or_default()
    }

    pub fn set(&mut self, name: &str, addr: &[SocketAddr]) {
        self.entries.insert(name.to_string(), addr.to_vec());
    }
}

impl Default for DnsCache {
    fn default() -> Self {
        Self {
            entries: HashMap::from_iter(DNS_CACHE_SEED.iter().map(|(name, addrs)| (name.to_string(), addrs.to_vec()))),
        }
    }
}
