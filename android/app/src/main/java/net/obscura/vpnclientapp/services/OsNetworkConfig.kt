package net.obscura.vpnclientapp.services

import kotlinx.serialization.Serializable

// Keep synchronized with rustlib/src/network_config.rs
@Serializable
data class OsNetworkConfig(
    val dns: List<String>,
    val ipv4: String,
    val ipv6: String,
    val mtu: Int,
    val useSystemDns: Boolean,
)
