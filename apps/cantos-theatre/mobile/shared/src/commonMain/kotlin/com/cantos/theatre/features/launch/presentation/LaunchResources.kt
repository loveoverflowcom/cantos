package com.cantos.theatre.features.launch.presentation

import com.cantos.theatre.presentation.model.HostConfigurationFault
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.generated.LaunchKey

internal data class LaunchResources(
    val status: String,
    val tagline: String,
    val category: String,
    val unavailableTitle: String,
    val unavailableBody: String,
)

/** Same selected locale as the shell; no process-global resource environment or remount. */
internal fun launchResources(locale: ShellLocale): LaunchResources = LaunchResources(
    LaunchKey.SplashStatus.text(locale), LaunchKey.SplashTagline.text(locale),
    LaunchKey.SplashCategory.text(locale), LaunchKey.ListeningUnavailableTitle.text(locale),
    LaunchKey.ListeningUnavailableBody.text(locale),
)

/** The host maps a closed fault, never exception text or an implicit device language. */
internal fun faultResources(fault: HostConfigurationFault, locale: ShellLocale): Pair<String, String> {
    val body = when (fault) {
        HostConfigurationFault.OriginEmpty -> LaunchKey.HostFaultOriginEmpty
        HostConfigurationFault.OriginMalformed -> LaunchKey.HostFaultOriginMalformed
        HostConfigurationFault.OriginNotHttps -> LaunchKey.HostFaultOriginNotHttps
        HostConfigurationFault.OriginCleartextHostNotAllowed -> LaunchKey.HostFaultOriginCleartextHost
        HostConfigurationFault.NetworkNotDeclared -> LaunchKey.HostFaultNetworkNotDeclared
    }
    return LaunchKey.HostFaultTitle.text(locale) to body.text(locale)
}
