package com.cantos.theatre.app

import com.cantos.theatre.adapters.preview.previewLibraryStore
import com.cantos.theatre.ports.LocalePreferenceStore
import kotlinx.coroutines.CoroutineScope

/** The real owners over the preview ports, built exactly as the Desktop Preview builds them. */
internal class PreviewHarness(
    scope: CoroutineScope,
    seed: PreviewSeed = PreviewSession.Guest.seed(),
    localeStore: LocalePreferenceStore? = null,
    pageSize: Int = 50,
) {
    val owners = PreviewOwners(scope, seed, previewLibraryStore(), localeStore, pageSize = pageSize)
    val shell get() = owners.shell
    val identity get() = owners.identity
    val account get() = owners.account
    val route get() = shell.navigation.value.route
    val stack get() = shell.navigation.value.stack

    fun close() = owners.close()
}
