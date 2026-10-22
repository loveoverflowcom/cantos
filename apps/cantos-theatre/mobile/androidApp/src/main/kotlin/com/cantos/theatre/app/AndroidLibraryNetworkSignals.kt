package com.cantos.theatre.app

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.os.Build
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

/** Process-owned OS hints, serialized on the same main scope as the account/flusher. */
internal class AndroidLibraryNetworkSignals(
    context: Context,
    private val scope: CoroutineScope,
    private val restored: () -> Unit,
) {
    private val manager = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
    private val available = mutableSetOf<Network>()
    private var closed = false
    private val callback = object : ConnectivityManager.NetworkCallback() {
        override fun onCapabilitiesChanged(network: Network, capabilities: NetworkCapabilities) {
            val usable = capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) &&
                capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED)
            scope.launch {
                if (closed) return@launch
                val wasAvailable = available.isNotEmpty()
                if (usable) available.add(network) else available.remove(network)
                if (!wasAvailable && available.isNotEmpty()) restored()
            }
        }
        override fun onLost(network: Network) {
            scope.launch { if (!closed) available.remove(network) }
        }
    }

    init {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) manager.registerDefaultNetworkCallback(callback)
        else manager.registerNetworkCallback(NetworkRequest.Builder()
            .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET).build(), callback)
    }

    fun close() {
        if (closed) return
        closed = true
        manager.unregisterNetworkCallback(callback)
        available.clear()
    }
}
