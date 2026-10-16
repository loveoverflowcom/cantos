package com.cantos.theatre.adapters.http

/** What a host's configured API origin means for composing the live public catalog. */
sealed interface CatalogOriginSelection {
    data class Configured(val origin: ApiOrigin) : CatalogOriginSelection
    /** Nothing configured: the host keeps its explicit unavailable state. */
    data object NotConfigured : CatalogOriginSelection
    /** A value was configured but is not admissible for this build; fail closed, never fall back. */
    data object Rejected : CatalogOriginSelection
}

/**
 * Release builds admit only `https://` origins. [allowLoopbackHttp] is true only for a debug
 * build, where [ApiOrigin] already limits `http://` to localhost, 127.0.0.1 and [::1]
 * (for example `adb reverse tcp:8080 tcp:8080`). The value is used exactly as configured.
 */
fun selectCatalogOrigin(raw: String?, allowLoopbackHttp: Boolean): CatalogOriginSelection {
    if (raw.isNullOrEmpty()) return CatalogOriginSelection.NotConfigured
    val origin = try {
        ApiOrigin.parse(raw)
    } catch (_: IllegalArgumentException) {
        return CatalogOriginSelection.Rejected
    }
    if (!origin.isHttps && !allowLoopbackHttp) return CatalogOriginSelection.Rejected
    return CatalogOriginSelection.Configured(origin)
}
