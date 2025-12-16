use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
