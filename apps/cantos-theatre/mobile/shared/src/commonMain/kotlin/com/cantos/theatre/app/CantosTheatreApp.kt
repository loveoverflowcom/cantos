package com.cantos.theatre.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import com.cantos.theatre.application.errors.ErrorEvent
import com.cantos.theatre.application.identity.AccountView
import com.cantos.theatre.application.identity.toAccountView
import com.cantos.theatre.application.preferences.PreferenceTheme
import com.cantos.theatre.domain.library.LibraryState
import com.cantos.theatre.domain.membership.MembershipEvent
import com.cantos.theatre.features.auth.presentation.AccountContent
import com.cantos.theatre.features.auth.presentation.AuthHeader
import com.cantos.theatre.features.auth.presentation.AuthIntent
import com.cantos.theatre.features.auth.presentation.LoginScreen
import com.cantos.theatre.features.auth.presentation.RegisterScreen
import com.cantos.theatre.features.auth.presentation.RegistrationIntent
import com.cantos.theatre.features.auth.presentation.VerificationIntent
import com.cantos.theatre.features.auth.presentation.VerifyEmailScreen
import com.cantos.theatre.features.auth.presentation.authReturnFor
import com.cantos.theatre.features.membership.presentation.MembershipHeader
import com.cantos.theatre.features.membership.presentation.MembershipPreview
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import com.cantos.theatre.features.shell.presentation.LocalShellCapabilities
import com.cantos.theatre.features.shell.presentation.LocalShellCopyOverrides
import com.cantos.theatre.features.shell.presentation.LocalShellLibraryHeader
import com.cantos.theatre.features.shell.presentation.LocalShellNotice
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellCapabilities
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.application.preferences.PreferencesUpdate
import com.cantos.theatre.ports.CatalogCachePort
import com.cantos.theatre.ports.CatalogCacheStatus
import com.cantos.ui.Density
import com.cantos.ui.MotionPreference
import com.cantos.ui.tokenDp
import kotlinx.coroutines.delay

/**
 * The Cantos Theatre application root: the one listener experience (Discover, Search, Work,
 * Player, Queue, Library, History, Account, Settings and the mini-player) over the live public
 * catalog, the durable Library, the native player and the real account. Nothing here reads a
 * fixture; the process owners and [shell] arrive from the platform host, and an owner that is
 * absent shows its own honest state instead of a sample.
 */
@Composable
fun CantosTheatreApp(
    shell: TheatreShell,
    modifier: Modifier = Modifier,
    textScale: Float = 1f,
    onThemeChanged: (Boolean) -> Unit = {},
    backHandler: @Composable (Boolean, () -> Unit) -> Unit = { _, _ -> },
    cache: CatalogCachePort? = null,
    density: Density = Density.Comfortable,
    motionPreference: MotionPreference = MotionPreference.Reduced,
) {
    val navigation by shell.navigation.collectAsState()
    val catalog by shell.catalog.state.collectAsState()
    val known by shell.known.collectAsState()
    val playback by shell.runtime.listening.playback.state.collectAsState()
    val selection by shell.runtime.listening.selection.collectAsState()
    val failure by shell.runtime.listening.failure.collectAsState()
    val requested by shell.requested.collectAsState()
    val errors by shell.errors.collectAsState()
    val library by shell.library.collectAsState()
    val ui by shell.ui.collectAsState()
    val prefs by shell.preferences.state.collectAsState()
    val localeUpdate by shell.preferences.localeUpdate.collectAsState()
    val accountState = shell.runtime.account?.state?.collectAsState()?.value
    val registration = shell.registration?.state?.collectAsState()?.value
    val verification = shell.verification?.state?.collectAsState()?.value
    val cacheStatus = cache?.status?.collectAsState()?.value
    val locale = prefs.locale.toShellLocale()
    val theme = if (prefs.theme == PreferenceTheme.Dark) ShellTheme.Dark else ShellTheme.Light
    val projection = remember(navigation, catalog, known, playback, selection, failure, requested, library, ui, theme, locale) {
        projectTheatreShell(TheatreShellInputs(navigation, catalog, known, playback, selection, requested,
            failure != null, library, theme, locale, ui.query, ui.libraryTab, ui.timer))
    }
    val accountView = accountState.toAccountView()
    // A private draft must not outlive the account that made it; a guest draft survives sign-in.
    val signedIn = accountView == AccountView.SignedIn
    var wasSignedIn by remember(shell) { mutableStateOf(signedIn) }
    LaunchedEffect(signedIn) {
        if (wasSignedIn && !signedIn) shell.clearMembership()
        wasSignedIn = signedIn
    }
    // Feedback for an explicit restore that found nothing; it belongs to the Login entry only.
    var nothingSaved by remember(shell) { mutableStateOf(false) }
    LaunchedEffect(navigation.route) { nothingSaved = false }

    val state = projection.state
    val dispatch: (ShellEvent) -> Unit = shell::dispatch
    val onAuth: (AuthIntent) -> Unit = { intent ->
        when (intent) {
            is AuthIntent.Login -> shell.login(intent.email, intent.password)
            AuthIntent.CancelLogin -> shell.cancelLogin()
            AuthIntent.RestoreSession -> nothingSaved = shell.restoreSession() == RestoreOutcome.NothingSaved
            AuthIntent.RetryCleanup, AuthIntent.Logout -> shell.logout()
            AuthIntent.OpenLogin -> shell.openLogin()
            AuthIntent.OpenRegister -> shell.openRegister()
            AuthIntent.OpenHome -> dispatch(ShellEvent.Navigate(ShellRoute.Discovery))
            AuthIntent.OpenLibrary -> dispatch(ShellEvent.Navigate(ShellRoute.Library))
            AuthIntent.OpenMembership -> dispatch(ShellEvent.Navigate(ShellRoute.MembershipPreview))
            AuthIntent.OpenSettings -> dispatch(ShellEvent.Navigate(ShellRoute.Settings))
            AuthIntent.ToggleTheme -> dispatch(ShellEvent.SetTheme(if (theme == ShellTheme.Dark) ShellTheme.Light else ShellTheme.Dark))
            AuthIntent.Back -> dispatch(ShellEvent.Back)
        }
    }
    CompositionLocalProvider(
        LocalShellCopyOverrides provides TheatreShellCopyOverrides,
        // Real bookmarks exist when the durable Library owner does; a guest is guided to sign in, not shown a fake save.
        LocalShellCapabilities provides ShellCapabilities(bookmarks = shell.runtime.library != null),
        LocalShellNotice provides { _, copy -> CatalogNotice(cacheStatus, copy, dispatch) },
        LocalShellLibraryHeader provides {
            LibraryHeader(library, accountView, shell.runtime.library != null, ShellCopy(locale, TheatreShellCopyOverrides), shell, dispatch)
        },
    ) {
        CantosPresentation(
            state, projection.catalog, dispatch, modifier, textScale, onThemeChanged, backHandler,
            membershipContent = {
                MembershipPreview(ui.membership, locale, shell::membership,
                    onExplore = { dispatch(ShellEvent.Navigate(ShellRoute.Discovery)) })
            },
            membershipHeader = {
                MembershipHeader(locale, theme,
                    onBack = { dispatch(ShellEvent.Back) },
                    onToggleTheme = { dispatch(ShellEvent.SetTheme(if (theme == ShellTheme.Dark) ShellTheme.Light else ShellTheme.Dark)) })
            },
            membershipScreen = ui.membership.screen,
            accountContent = { loginFocus -> AccountContent(locale, theme, accountView, onAuth, loginFocus) },
            authContent = {
                val verifier = shell.verification
                if (state.route == ShellRoute.VerifyEmail && verifier != null && verification != null) {
                    // Ticks only while a 429 holds the retained request on this screen.
                    val wait by produceState(verifier.retryWaitSeconds(), verification.phase) {
                        while (value != null) {
                            delay(1_000)
                            value = verifier.retryWaitSeconds()
                        }
                    }
                    VerifyEmailScreen(locale, verification, accountView, wait) { intent ->
                        when (intent) {
                            is VerificationIntent.Password -> verifier.editPassword(intent.value)
                            is VerificationIntent.Repeat -> verifier.editRepeat(intent.value)
                            is VerificationIntent.Confirm -> verifier.confirm(intent.checked)
                            VerificationIntent.Create -> verifier.complete()
                            VerificationIntent.Retry -> verifier.retry()
                            VerificationIntent.ChooseAnother -> verifier.editAfterRefusal()
                            VerificationIntent.Continue -> shell.continueAfterVerification()
                            VerificationIntent.RequestAnother -> shell.openRegister()
                        }
                    }
                } else if (state.route == ShellRoute.Register) RegisterScreen(locale, onAuth,
                    registration = registration, onRegistration = { intent ->
                        shell.registration?.let { owner -> when (intent) {
                            is RegistrationIntent.Email -> owner.editEmail(intent.value)
                            RegistrationIntent.Submit -> owner.submit()
                            RegistrationIntent.Cancel -> owner.cancel()
                            RegistrationIntent.Restart -> owner.restart()
                        } }
                    })
                else LoginScreen(locale, accountView, nothingSaved, onAuth)
            },
            authHeader = { AuthHeader(locale, theme, authReturnFor(state.stack), onAuth) },
            density = density,
            motionPreference = motionPreference,
            localeSaveUnavailable = localeUpdate == PreferencesUpdate.LocaleStoreUnavailable,
            activeError = errors.active,
            onErrorAction = { shell.onErrorEvent(ErrorEvent.ActionChosen(it)) },
            onErrorDismiss = { shell.onErrorEvent(ErrorEvent.Dismissed) },
        )
    }
}

/** An offline or cached catalog says so, and offers the one way to ask the API again. */
@Composable
private fun CatalogNotice(status: CatalogCacheStatus?, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    when (status) {
        CatalogCacheStatus.Cached -> Column(Modifier.fillMaxWidth().testTag("catalog-cached"),
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
            Text(copy.text(CopyKey.CachedCatalog), style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
            TextButton({ dispatch(ShellEvent.Refresh) }, modifier = Modifier.heightIn(min = 48.dp).testTag("catalog-refresh")) {
                Text(copy.text(CopyKey.Refresh))
            }
        }
        CatalogCacheStatus.StorageUnavailable -> Text(copy.text(CopyKey.ListeningStoreFailure),
            style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error,
            modifier = Modifier.testTag("catalog-storage-failure"))
        CatalogCacheStatus.Idle, CatalogCacheStatus.Network, null -> Unit
    }
}

/**
 * What the durable Library adds to the known Library screen: sign-in for a guest, sync state and
 * conflict choices. Rows, their reasons and each section's paging belong to the sections themselves.
 */
@Composable
private fun LibraryHeader(
    library: LibraryState,
    account: AccountView,
    storeAvailable: Boolean,
    copy: ShellCopy,
    shell: TheatreShell,
    dispatch: (ShellEvent) -> Unit,
) {
    val controller = shell.runtime.library
    Column(Modifier.fillMaxWidth().testTag("library-sync"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        if (!storeAvailable) Text(copy.text(CopyKey.ListeningStoreFailure), color = MaterialTheme.colorScheme.error)
        if (library.account == null) {
            Text(copy.text(CopyKey.LibrarySignIn))
            if (account != AccountView.Unavailable) TextButton({ shell.dispatch(ShellEvent.Navigate(ShellRoute.Login)) },
                modifier = Modifier.heightIn(min = 48.dp).testTag("library-sign-in")) {
                Text(copy.text(CopyKey.Account))
            }
            return@Column
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            TextButton({ controller?.refresh() }, modifier = Modifier.heightIn(min = 48.dp).testTag("library-refresh")) {
                Text(copy.text(CopyKey.Refresh))
            }
            if (library.pending > 0) TextButton({ controller?.drain() }, modifier = Modifier.heightIn(min = 48.dp).testTag("library-drain")) {
                Text(copy.text(CopyKey.RetryListening))
            }
        }
        if (library.failure != null || library.failures.isNotEmpty()) Text(copy.text(CopyKey.SyncFailed), Modifier.testTag("library-sync-failed"))
        if (library.pending > 0) Text(copy.text(CopyKey.SyncPending), Modifier.testTag("library-sync-pending"))
        library.conflicts.filter { it.resourceKey.startsWith("progress:") }.forEach { conflict ->
            Text(copy.text(CopyKey.ProgressConflict))
            val publication = conflict.resourceKey.removePrefix("progress:")
            FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                TextButton({ controller?.resolveProgress(publication, true) }, modifier = Modifier.heightIn(min = 48.dp).testTag("library-keep-here")) {
                    Text(copy.text(CopyKey.KeepHere))
                }
                TextButton({ controller?.resolveProgress(publication, false) }, modifier = Modifier.heightIn(min = 48.dp).testTag("library-use-other")) {
                    Text(copy.text(CopyKey.UseOther))
                }
            }
        }
    }
}
