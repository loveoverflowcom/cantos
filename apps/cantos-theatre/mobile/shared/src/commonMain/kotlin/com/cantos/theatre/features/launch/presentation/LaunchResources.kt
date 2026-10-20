package com.cantos.theatre.features.launch.presentation

import com.cantos.theatre.presentation.model.HostConfigurationFault
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.resources.Res
import com.cantos.theatre.resources.host_fault_network_not_declared
import com.cantos.theatre.resources.host_fault_network_not_declared_en
import com.cantos.theatre.resources.host_fault_origin_cleartext_host
import com.cantos.theatre.resources.host_fault_origin_cleartext_host_en
import com.cantos.theatre.resources.host_fault_origin_empty
import com.cantos.theatre.resources.host_fault_origin_empty_en
import com.cantos.theatre.resources.host_fault_origin_malformed
import com.cantos.theatre.resources.host_fault_origin_malformed_en
import com.cantos.theatre.resources.host_fault_origin_not_https
import com.cantos.theatre.resources.host_fault_origin_not_https_en
import com.cantos.theatre.resources.host_fault_title
import com.cantos.theatre.resources.host_fault_title_en
import com.cantos.theatre.resources.listening_unavailable_body
import com.cantos.theatre.resources.listening_unavailable_body_en
import com.cantos.theatre.resources.listening_unavailable_title
import com.cantos.theatre.resources.listening_unavailable_title_en
import com.cantos.theatre.resources.splash_category
import com.cantos.theatre.resources.splash_category_en
import com.cantos.theatre.resources.splash_status
import com.cantos.theatre.resources.splash_status_en
import com.cantos.theatre.resources.splash_tagline
import com.cantos.theatre.resources.splash_tagline_en
import org.jetbrains.compose.resources.StringResource

internal data class LaunchResources(
    val status: StringResource,
    val tagline: StringResource,
    val category: StringResource,
    val unavailableTitle: StringResource,
    val unavailableBody: StringResource,
)

// CMP 1.9 has no public subtree locale override. Select these existing XML resources
// explicitly instead of changing the process locale or remounting the application.
internal fun launchResources(locale: ShellLocale): LaunchResources = when (locale) {
    ShellLocale.Vietnamese -> LaunchResources(
        Res.string.splash_status, Res.string.splash_tagline, Res.string.splash_category,
        Res.string.listening_unavailable_title, Res.string.listening_unavailable_body,
    )
    ShellLocale.English -> LaunchResources(
        Res.string.splash_status_en, Res.string.splash_tagline_en, Res.string.splash_category_en,
        Res.string.listening_unavailable_title_en, Res.string.listening_unavailable_body_en,
    )
}

/** Title and body naming one typed host configuration fault; the listener never sees a raw exception. */
internal fun faultResources(fault: HostConfigurationFault, locale: ShellLocale): Pair<StringResource, StringResource> {
    val vietnamese = locale == ShellLocale.Vietnamese
    val title = if (vietnamese) Res.string.host_fault_title else Res.string.host_fault_title_en
    val body = when (fault) {
        HostConfigurationFault.OriginEmpty ->
            if (vietnamese) Res.string.host_fault_origin_empty else Res.string.host_fault_origin_empty_en
        HostConfigurationFault.OriginMalformed ->
            if (vietnamese) Res.string.host_fault_origin_malformed else Res.string.host_fault_origin_malformed_en
        HostConfigurationFault.OriginNotHttps ->
            if (vietnamese) Res.string.host_fault_origin_not_https else Res.string.host_fault_origin_not_https_en
        HostConfigurationFault.OriginCleartextHostNotAllowed ->
            if (vietnamese) Res.string.host_fault_origin_cleartext_host else Res.string.host_fault_origin_cleartext_host_en
        HostConfigurationFault.NetworkNotDeclared ->
            if (vietnamese) Res.string.host_fault_network_not_declared else Res.string.host_fault_network_not_declared_en
    }
    return title to body
}
