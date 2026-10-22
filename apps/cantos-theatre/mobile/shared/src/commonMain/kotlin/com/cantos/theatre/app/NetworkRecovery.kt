package com.cantos.theatre.app

/**
 * Which networks an OS registration reports. A current-default registration follows one network
 * at a time: a new default replaces the old one and the old one may never be reported lost.
 * An all-networks registration reports every matching network and each one's loss.
 */
enum class NetworkReporting { CurrentDefault, AllNetworks }

/** Android 7.0 (API 24) is the first release with a current-default registration. */
fun networkReportingForSdk(sdkInt: Int): NetworkReporting =
    if (sdkInt >= 24) NetworkReporting.CurrentDefault else NetworkReporting.AllNetworks

/**
 * Before Android 8.0 (API 26) the platform does not guarantee a capabilities callback right after
 * a network becomes available, so the adapter must read the capabilities itself then.
 */
fun networkCapabilitiesNeedProbe(sdkInt: Int): Boolean = sdkInt < 26

/**
 * Turns platform connectivity facts into at most one recovery opportunity per outage. It is a
 * scheduling hint only: it grants no authority and never touches the account, player or outbox.
 * Not thread-safe: the owner serializes every call on one scope.
 */
class NetworkRecoveryTracker(private val reporting: NetworkReporting) {
    private var closed = false
    private var defaultNetwork: Long? = null
    private var defaultUsable = false
    private val usableNetworks = mutableSetOf<Long>()

    private fun hasUsableNetwork(): Boolean = when (reporting) {
        NetworkReporting.CurrentDefault -> defaultNetwork != null && defaultUsable
        NetworkReporting.AllNetworks -> usableNetworks.isNotEmpty()
    }

    /** True exactly when no usable network was known and this fact provides one. */
    fun capabilities(network: Long, usable: Boolean): Boolean {
        if (closed) return false
        val hadUsable = hasUsableNetwork()
        when (reporting) {
            NetworkReporting.CurrentDefault -> {
                defaultNetwork = network
                defaultUsable = usable
            }
            NetworkReporting.AllNetworks -> if (usable) usableNetworks.add(network) else usableNetworks.remove(network)
        }
        return !hadUsable && hasUsableNetwork()
    }

    /** A loss of a network that is no longer tracked is stale and has no effect. */
    fun lost(network: Long) {
        if (closed) return
        when (reporting) {
            NetworkReporting.CurrentDefault -> if (defaultNetwork == network) {
                defaultNetwork = null
                defaultUsable = false
            }
            NetworkReporting.AllNetworks -> usableNetworks.remove(network)
        }
    }

    fun close() {
        closed = true
        defaultNetwork = null
        defaultUsable = false
        usableNetworks.clear()
    }
}
