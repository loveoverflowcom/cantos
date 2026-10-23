package com.cantos.theatre.app

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.os.Build
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

/**
 * Process-owned OS hints, serialized on the same main scope as the account/flusher. The decision
 * of when connectivity returned is [NetworkRecoveryTracker]; this adapter only translates
 * platform callbacks into its neutral facts. It adds no authority and no second flusher.
 */
internal class AndroidLibraryNetworkSignals(
    context: Context,
    private val scope: CoroutineScope,
    private val restored: () -> Unit,
) {
    private val manager = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
    private val tracker = NetworkRecoveryTracker(networkReportingForSdk(Build.VERSION.SDK_INT))
    private var closed = false
    private val callback = object : ConnectivityManager.NetworkCallback() {
        // Before API 26 a new network is not guaranteed a capabilities callback; read it once
        // here. The read may be stale, which only mis-times a hint: a later callback corrects it.
        override fun onAvailable(network: Network) {
            if (networkCapabilitiesNeedProbe(Build.VERSION.SDK_INT)) {
                report(network, manager.getNetworkCapabilities(network))
            }
        }

        override fun onCapabilitiesChanged(network: Network, capabilities: NetworkCapabilities) =
            report(network, capabilities)

        override fun onLost(network: Network) {
            val id = network.networkHandle
            scope.launch { if (!closed) tracker.lost(id) }
        }
    }

    private fun report(network: Network, capabilities: NetworkCapabilities?) {
        val id = network.networkHandle
        val usable = capabilities != null &&
            capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) &&
            capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED)
        scope.launch { if (!closed && tracker.capabilities(id, usable)) restored() }
    }

    init {
        if (networkReportingForSdk(Build.VERSION.SDK_INT) == NetworkReporting.CurrentDefault) {
            manager.registerDefaultNetworkCallback(callback)
        } else {
            manager.registerNetworkCallback(NetworkRequest.Builder()
                .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET).build(), callback)
        }
    }

    fun close() {
        if (closed) return
        closed = true
        manager.unregisterNetworkCallback(callback)
        tracker.close()
    }
}
