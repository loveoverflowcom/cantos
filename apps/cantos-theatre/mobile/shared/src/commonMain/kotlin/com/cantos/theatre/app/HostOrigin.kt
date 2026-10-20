package com.cantos.theatre.app

import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.ApiOriginRefusal
import com.cantos.theatre.adapters.http.CatalogOriginSelection
import com.cantos.theatre.adapters.http.OriginSource
import com.cantos.theatre.adapters.http.selectPackagedApiOrigin
import com.cantos.theatre.presentation.model.HostConfigurationFault

/** What a native host's origin configuration means for composing the live application. */
sealed interface HostOrigin {
    data class Ready(val origin: ApiOrigin, val source: OriginSource) : HostOrigin
    /** Nothing live is composed; the host shows the typed [fault]. */
    data class Refused(val fault: HostConfigurationFault) : HostOrigin
}

/** [networkDeclared] is false when the platform packaging grants this app no network access. */
fun hostOrigin(selection: CatalogOriginSelection, networkDeclared: Boolean = true): HostOrigin = when (selection) {
    is CatalogOriginSelection.Rejected -> HostOrigin.Refused(selection.refusal.toFault())
    is CatalogOriginSelection.Configured ->
        if (networkDeclared) HostOrigin.Ready(selection.origin, selection.source)
        else HostOrigin.Refused(HostConfigurationFault.NetworkNotDeclared)
}

internal fun ApiOriginRefusal.toFault(): HostConfigurationFault = when (this) {
    ApiOriginRefusal.Empty -> HostConfigurationFault.OriginEmpty
    ApiOriginRefusal.Malformed -> HostConfigurationFault.OriginMalformed
    ApiOriginRefusal.NotHttps -> HostConfigurationFault.OriginNotHttps
    ApiOriginRefusal.CleartextHostNotAllowed -> HostConfigurationFault.OriginCleartextHostNotAllowed
}

/**
 * The origin text a native platform adapter (the iOS Swift transports) binds its requests to: the
 * packaged override, else the production default, or null when the configuration is refused.
 * Release hosts never admit cleartext, so no loopback exception applies here.
 */
fun packagedApiOriginText(packaged: String?): String? =
    (hostOrigin(selectPackagedApiOrigin(packaged, allowLoopbackHttp = false)) as? HostOrigin.Ready)?.origin?.toString()
