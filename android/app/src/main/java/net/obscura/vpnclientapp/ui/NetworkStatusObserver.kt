package net.obscura.vpnclientapp.ui

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import net.obscura.lib.util.Logger

private val log = Logger(NetworkStatusObserver::class)

// Network callbacks run on the "connectivity thread" by default:
// https://developer.android.com/develop/connectivity/network-ops/reading-network-state#listening-events
internal class NetworkStatusObserver(context: Context, private val callback: Callback) :
    ConnectivityManager.NetworkCallback() {
    interface Callback {
        fun onAvailableNetworksChanged(availableNetworks: Int)
    }

    private var availableNetworks = 0
    private val connectivityManager = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager

    init {
        val networkRequest =
            NetworkRequest.Builder()
                .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
                .addCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)
                .addCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED)
                .build()
        this.connectivityManager.registerNetworkCallback(networkRequest, this)
    }

    override fun onAvailable(network: Network) {
        this.availableNetworks += 1
        log.debug("network available: $network (available networks: ${this.availableNetworks})")
        this.callback.onAvailableNetworksChanged(this.availableNetworks)
    }

    override fun onLost(network: Network) {
        this.availableNetworks = (this.availableNetworks - 1).coerceAtLeast(0)
        log.debug("network lost: $network (available networks: ${this.availableNetworks})")
        this.callback.onAvailableNetworksChanged(this.availableNetworks)
    }
}
