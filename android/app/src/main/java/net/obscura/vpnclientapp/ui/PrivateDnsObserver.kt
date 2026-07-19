package net.obscura.vpnclientapp.ui

import android.content.Context
import android.net.ConnectivityManager
import android.net.LinkProperties
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import net.obscura.lib.util.Logger

private val log = Logger(PrivateDnsObserver::class)

internal class PrivateDnsObserver(context: Context, private val callback: Callback) :
    ConnectivityManager.NetworkCallback() {
    interface Callback {
        fun onPrivateDnsChanged(strictMode: Boolean)
    }

    private val strictModeNetworks = HashSet<Network>()
    private var strictMode = false
    private val connectivityManager = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager

    init {
        val networkRequest =
            NetworkRequest.Builder()
                .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
                .addCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)
                .build()
        this.connectivityManager.registerNetworkCallback(networkRequest, this)
    }

    override fun onLinkPropertiesChanged(network: Network, linkProperties: LinkProperties) {
        if (linkProperties.privateDnsServerName != null) {
            this.strictModeNetworks.add(network)
        } else {
            this.strictModeNetworks.remove(network)
        }
        this.notifyCallback()
    }

    override fun onLost(network: Network) {
        this.strictModeNetworks.remove(network)
        this.notifyCallback()
    }

    private fun notifyCallback() {
        val strictMode = this.strictModeNetworks.isNotEmpty()
        if (strictMode != this.strictMode) {
            this.strictMode = strictMode
            log.debug("private DNS strict mode: $strictMode")
            this.callback.onPrivateDnsChanged(strictMode)
        }
    }
}
