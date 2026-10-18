package com.cantos.theatre.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import com.cantos.theatre.features.catalog.presentation.PublicCatalogScreen
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.domain.catalog.CatalogCheck
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.library.LibraryResume
import com.cantos.theatre.domain.library.LibrarySection
import com.cantos.theatre.domain.library.LibraryState
import com.cantos.theatre.domain.library.LibrarySync
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackIntent
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.features.auth.presentation.NativeAccountPanel
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.clockTime
import com.cantos.theatre.ports.CatalogCachePort
import com.cantos.theatre.ports.CatalogCacheStatus
import com.cantos.ui.UiPreferences
import com.cantos.ui.tokenDp

/** The production graph is passed in once by the platform process owner. No demo projection. */
@Composable
fun NativeCatalogScreen(catalog: PublicCatalogController, runtime: NativeSessionRuntime,
                        preferences: UiPreferences, locale: ShellLocale,
                        backHandler: @Composable (Boolean, () -> Unit) -> Unit = { _, _ -> },
                        cache: CatalogCachePort? = null) {
    val state by catalog.state.collectAsState()
    val cacheStatus = cache?.status?.collectAsState()?.value
    val verified = runtime.account?.verifiedSession?.collectAsState()?.value
    val stored = runtime.library?.state?.collectAsState()?.value
    val library = stored?.takeIf { verified != null && it.account == verified.session.actor } ?: LibraryState()
    val copy = ShellCopy(locale)
    PublicCatalogScreen(state, catalog::dispatch, preferences, locale = locale, backHandler = backHandler,
        onListen = { work, unit -> runtime.listening.open(work.id.value, unit.id.value, unit.publicationId.value, unit.title, work.title) },
        player = {
            if (cacheStatus == CatalogCacheStatus.Cached) Text(copy.text(CopyKey.CachedCatalog), Modifier.padding(tokenDp("space.sm")))
            if (cacheStatus == CatalogCacheStatus.StorageUnavailable) Text(copy.text(CopyKey.ListeningStoreFailure))
            NativePlayerPanel(runtime, copy)
        },
        accountContent = { openLibrary ->
            Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(tokenDp("space.md"))) {
                NativeAccountPanel(runtime.account, locale, openLibrary)
            }
        },
        libraryContent = { if (runtime.library == null) Text(copy.text(CopyKey.ListeningStoreFailure), Modifier.padding(tokenDp("space.md")))
            else NativeLibraryPanel(library, runtime, copy,
            titles = state.works.items.associate { it.id.value to it.title },
            openWork = { raw -> when (val id = PublicWorkId.parse(raw)) {
                is CatalogCheck.Valid -> catalog.dispatch(PublicCatalogIntent.OpenWorkId(id.value))
                is CatalogCheck.Invalid -> Unit
            } }) },
        workActions = { work ->
            if (library.account != null) {
                val entry = library.saved.firstOrNull { it.workId == work.id.value }
                val saved = entry?.saved == true
                TextButton({ runtime.library?.setSaved(work.id.value, !saved) },
                    modifier = Modifier.heightIn(min = 48.dp).testTag("native-save")) {
                    Text(copy.text(if (saved) CopyKey.Saved else CopyKey.Save))
                }
                entry?.let { Text(copy.text(syncKey(it.sync))) }
            }
        })
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
private fun NativePlayerPanel(runtime: NativeSessionRuntime, copy: ShellCopy) {
    val state by runtime.listening.playback.state.collectAsState()
    val selection by runtime.listening.selection.collectAsState()
    val failure by runtime.listening.failure.collectAsState()
    var expanded by remember { mutableStateOf(false) }
    val session = state.session
    if (failure != null) Text(copy.text(CopyKey.ReleaseUnavailable), Modifier.padding(tokenDp("space.md")).testTag("native-selection-failed"))
    if (session == null) return
    val status = copy.text(when (state.status) {
        PlaybackStatus.Idle -> CopyKey.PlaybackIdle
        PlaybackStatus.Loading -> CopyKey.PlaybackLoading
        PlaybackStatus.Paused -> CopyKey.PlaybackPaused
        PlaybackStatus.Playing -> CopyKey.PlaybackPlaying
        PlaybackStatus.Buffering -> CopyKey.PlaybackBuffering
        PlaybackStatus.Ended -> CopyKey.PlaybackEnded
        PlaybackStatus.Failed -> CopyKey.PlaybackFailed
        PlaybackStatus.Simulating -> CopyKey.PlaybackFailed
    })
    val title = selection?.title?.takeIf(String::isNotBlank) ?: copy.text(CopyKey.Episodes)
    Surface(color = MaterialTheme.colorScheme.secondaryContainer, modifier = Modifier.fillMaxWidth().testTag("native-mini-player")) {
        Column(Modifier.padding(tokenDp("space.sm"))) {
            Text(title, style = MaterialTheme.typography.titleSmall)
            Text(status, modifier = Modifier.testTag("native-playback-status"), style = MaterialTheme.typography.bodySmall)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                TextButton({ runtime.listening.playback.toggleIntent() }, modifier = Modifier.heightIn(min = 48.dp).testTag("native-toggle")) {
                    Text(copy.text(if (session.intent == PlaybackIntent.Play && session.failure == null) CopyKey.Pause else CopyKey.Play))
                }
                TextButton({ expanded = true }, modifier = Modifier.heightIn(min = 48.dp).testTag("native-open-player")) { Text(copy.text(CopyKey.FullPlayer)) }
            }
        }
    }
    if (expanded) Dialog(onDismissRequest = { expanded = false }) {
        Surface(shape = MaterialTheme.shapes.large, modifier = Modifier.testTag("native-full-player")) {
            Column(Modifier.verticalScroll(rememberScrollState()).padding(tokenDp("space.md")),
                verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                Text(title, style = MaterialTheme.typography.headlineSmall)
                Text(status)
                var seek by remember(session.source) { mutableStateOf<Float?>(null) }
                val seekInteraction = remember { MutableInteractionSource() }
                Slider(value = seek ?: session.positionMs.toFloat(), onValueChange = { seek = it },
                    valueRange = 0f..session.source.durationMs.toFloat(),
                    interactionSource = seekInteraction,
                    thumb = {
                        SliderDefaults.Thumb(seekInteraction, modifier = Modifier.heightIn(min = 48.dp))
                    },
                    onValueChangeFinished = {
                        seek?.let { runtime.listening.playback.dispatch(PlaybackEvent.SeekRequested(it.toLong())) }
                        seek = null
                    }, modifier = Modifier.heightIn(min = 48.dp).testTag("native-seek").semantics { contentDescription = copy.text(CopyKey.Position) })
                Text(copy.text(CopyKey.PositionCaption,
                    "elapsed" to clockTime((session.positionMs / 1000).toInt()), "total" to clockTime((session.source.durationMs / 1000).toInt())))
                FlowRow(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                    TextButton({ runtime.listening.playback.dispatch(PlaybackEvent.SkipRequested(-15_000)) },
                        modifier = Modifier.heightIn(min = 48.dp).testTag("native-full-rewind")) { Text(copy.text(CopyKey.Rewind)) }
                    Button({ runtime.listening.playback.toggleIntent() }, modifier = Modifier.heightIn(min = 48.dp).testTag("native-full-toggle")) { Text(copy.text(if (session.intent == PlaybackIntent.Play && session.failure == null) CopyKey.Pause else CopyKey.Play)) }
                    TextButton({ runtime.listening.playback.dispatch(PlaybackEvent.SkipRequested(15_000)) },
                        modifier = Modifier.heightIn(min = 48.dp).testTag("native-full-forward")) { Text(copy.text(CopyKey.Forward)) }
                }
                FlowRow {
                    listOf(750, 1000, 1250, 1500, 2000).forEach { speed ->
                        TextButton({ runtime.listening.playback.dispatch(PlaybackEvent.SpeedRequested(speed)) },
                            modifier = Modifier.heightIn(min = 48.dp).testTag("native-speed-$speed")) { Text(copy.speed(speed / 10)) }
                    }
                }
                TextButton({ expanded = false }, modifier = Modifier.heightIn(min = 48.dp).testTag("native-full-back")) { Text(copy.text(CopyKey.Back)) }
            }
        }
    }
}

@Composable
private fun NativeLibraryPanel(state: LibraryState, runtime: NativeSessionRuntime, copy: ShellCopy,
                               titles: Map<String, String>, openWork: (String) -> Unit) {
    var clear by remember { mutableStateOf(false) }
    val controller = runtime.library
    Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(tokenDp("space.md")).testTag("native-library"),
        verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Text(copy.text(CopyKey.Library), style = MaterialTheme.typography.headlineSmall)
        if (state.account == null) { Text(copy.text(CopyKey.LibrarySignIn)); return@Column }
        TextButton({ controller?.refresh() }, modifier = Modifier.heightIn(min = 48.dp)) { Text(copy.text(CopyKey.Refresh)) }
        if (state.refreshing) Text(copy.text(CopyKey.Loading))
        if (state.failure != null || state.failures.isNotEmpty()) Text(copy.text(CopyKey.SyncFailed))
        if (state.pending > 0) {
            Text(copy.text(CopyKey.SyncPending))
            TextButton({ controller?.drain() }) { Text(copy.text(CopyKey.RetryListening)) }
        }
        Text(copy.text(CopyKey.Continuing), style = MaterialTheme.typography.titleLarge)
        if (state.continueListening.isEmpty()) Text(copy.text(CopyKey.NoContinue))
        state.continueListening.forEach { entry ->
            val title = titles[entry.audio.workId] ?: copy.text(CopyKey.Work)
            Text(title)
            Text(copy.text(syncKey(entry.sync)))
            if (entry.resume == LibraryResume.ExactAvailable) TextButton({ runtime.listening.restore(entry.audio, entry.positionMs) }) {
                Text(copy.text(CopyKey.ResumeSampleName, "title" to title, "time" to clockTime((entry.positionMs / 1000).toInt())))
            } else if (entry.resume == LibraryResume.NeedsRefresh) {
                Text(copy.text(CopyKey.ResumeNeedsRefresh))
                TextButton({ controller?.refresh() }) { Text(copy.text(CopyKey.Refresh)) }
            } else Text(copy.text(CopyKey.ReleaseUnavailable))
        }
        Text(copy.text(CopyKey.SavedTab), style = MaterialTheme.typography.titleLarge)
        if (state.saved.none { it.saved }) Text(copy.text(CopyKey.NoSaved))
        state.saved.filter { it.saved }.forEach { entry ->
            Row {
                TextButton({ openWork(entry.workId) }, modifier = Modifier.weight(1f)) { Text(titles[entry.workId] ?: copy.text(CopyKey.Work)) }
                TextButton({ controller?.setSaved(entry.workId, false) }) { Text(copy.text(CopyKey.Saved)) }
            }
            Text(copy.text(syncKey(entry.sync)))
        }
        Text(copy.text(CopyKey.History), style = MaterialTheme.typography.titleLarge)
        if (state.history.isEmpty()) Text(copy.text(CopyKey.NoHistory))
        state.history.forEach { entry ->
            TextButton({ openWork(entry.audio.workId) }) { Text(titles[entry.audio.workId] ?: copy.text(CopyKey.Work)) }
        }
        if (state.history.isNotEmpty()) TextButton({ clear = true }) { Text(copy.text(CopyKey.ClearHistory)) }
        LibrarySection.entries.filter { state.cursors[it] != null }.forEach { section ->
            TextButton({ controller?.nextPage(section) }) { Text(copy.text(CopyKey.LoadMore)) }
        }
        state.conflicts.filter { it.resourceKey.startsWith("progress:") }.forEach { conflict ->
            Text(copy.text(CopyKey.ProgressConflict))
            val publication = conflict.resourceKey.removePrefix("progress:")
            TextButton({ controller?.resolveProgress(publication, true) }) { Text(copy.text(CopyKey.KeepHere)) }
            TextButton({ controller?.resolveProgress(publication, false) }) { Text(copy.text(CopyKey.UseOther)) }
        }
    }
    if (clear) AlertDialog(onDismissRequest = { clear = false },
        title = { Text(copy.text(CopyKey.ClearHistoryConfirmTitle)) }, text = { Text(copy.text(CopyKey.ClearHistoryHelp)) },
        confirmButton = { TextButton({ clear = false; controller?.clearHistory() }) { Text(copy.text(CopyKey.ConfirmClearHistory)) } },
        dismissButton = { TextButton({ clear = false }) { Text(copy.text(CopyKey.Cancel)) } })
}

private fun syncKey(sync: LibrarySync): CopyKey = when (sync) {
    LibrarySync.Pending -> CopyKey.SyncPending
    LibrarySync.Confirmed -> CopyKey.SyncConfirmed
    LibrarySync.Failed -> CopyKey.SyncFailed
}
