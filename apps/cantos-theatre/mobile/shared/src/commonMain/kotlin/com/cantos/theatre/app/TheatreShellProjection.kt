package com.cantos.theatre.app

import com.cantos.theatre.application.catalog.CatalogFailure
import com.cantos.theatre.application.catalog.DetailPhase
import com.cantos.theatre.application.catalog.PageWalk
import com.cantos.theatre.application.catalog.PublicCatalogState
import com.cantos.theatre.application.catalog.WalkPhase
import com.cantos.theatre.application.playback.NativeSelection
import com.cantos.theatre.domain.catalog.PublicUnit
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkKind
import com.cantos.theatre.domain.catalog.PublicUnitKind
import com.cantos.theatre.domain.catalog.unitKind
import com.cantos.theatre.domain.library.LibraryState
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackIntent
import com.cantos.theatre.domain.playback.PlaybackState
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.PlaybackUiStatus
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellDetailPhase
import com.cantos.theatre.presentation.model.ShellEpisode
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellLibraryTab
import com.cantos.theatre.presentation.model.ShellListPhase
import com.cantos.theatre.presentation.model.ShellListeningFailure
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellTimer
import com.cantos.theatre.presentation.model.ShellUnitKind
import com.cantos.theatre.presentation.model.ShellWork
import com.cantos.theatre.presentation.model.ShellWorkId

/** What the listener last asked to hear. It selects the episode while the API grants access. */
data class UnitRequest(
    val workId: String,
    val unitId: String,
    val publicationId: String,
    val title: String,
    val workTitle: String,
)

/** Every authority the old presentation is a read-only view of; nothing here is writable state. */
class TheatreShellInputs(
    val navigation: NavigationState,
    val catalog: PublicCatalogState,
    val known: KnownCatalog,
    val playback: PlaybackState,
    val selection: NativeSelection?,
    val requested: UnitRequest?,
    val accessFailed: Boolean,
    val library: LibraryState,
    val theme: ShellTheme,
    val locale: ShellLocale,
    val query: String,
    val libraryTab: ShellLibraryTab,
    val timer: ShellTimer,
)

class ShellProjection(val state: ShellState, val catalog: ShellCatalogView)

/** The episode id used while nothing is selected; it never resolves to catalog metadata. */
internal val NoSelection = ShellEpisodeId("none", ShellWorkId("none"))

/**
 * Pure projection of the live catalog, Library, playback and navigation owners into the screens
 * the listener already knows. Metadata comes only from the API values in [TheatreShellInputs]:
 * a work or unit the process has not been shown is named with neutral wording, never invented.
 */
fun projectTheatreShell(i: TheatreShellInputs): ShellProjection {
    val copy = ShellCopy(i.locale)
    val session = i.playback.session
    val audio = session?.source?.audio
    val pending = i.requested?.takeIf { audio == null || it.unitId != audio.unitId }
    val selectedUnit = pending?.unitId ?: audio?.unitId
    val selectedWork = pending?.workId ?: audio?.workId

    val builder = WorkBuilder(i.known, copy)
    i.catalog.works.items.forEach { builder.add(it) }
    i.catalog.detail?.let { detail ->
        val work = (detail.phase as? DetailPhase.Ready)?.work ?: detail.summary ?: i.known.work(detail.id.value)
        if (work != null) builder.add(work) else builder.stub(detail.id.value)
        builder.detail(detail.id.value, detail.phase, detail.units)
    }
    val saved = i.library.saved.filter { it.saved }.map { it.workId }
    saved.forEach { builder.reference(it) }
    i.library.continueListening.forEach { builder.reference(it.audio.workId); builder.unit(it.audio, null) }
    i.library.history.forEach { builder.reference(it.audio.workId); builder.unit(it.audio, null) }
    if (audio != null) {
        builder.reference(audio.workId)
        builder.unit(audio, i.selection?.takeIf { it.title.isNotBlank() })
    }
    if (pending != null) builder.requested(pending)

    val listedIds = i.catalog.works.items.map { it.id.value }.toSet()
    val listed = i.catalog.works.items.mapNotNull { builder.build(it.id.value) }
    val referenced = builder.ids().filter { it !in listedIds }.mapNotNull { builder.build(it) }
    val worksWalk = i.catalog.works
    val view = ShellCatalogView(
        works = listed,
        worksPhase = if (worksWalk.items.isEmpty()) ShellListPhase.Complete else worksWalk.phase.toListPhase(),
        searchedBySource = true,
        referenced = referenced,
        referencedEpisodes = builder.extraEpisodes(),
    )

    val matchesSelection = selectedUnit != null && pending == null && audio?.unitId == selectedUnit
    val selected = selectedUnit?.let { unit -> ShellEpisodeId(unit, ShellWorkId(selectedWork ?: "none")) } ?: NoSelection
    val positionMs = if (matchesSelection) session?.positionMs ?: 0L else 0L
    val continueByPublication = i.library.continueListening.associateBy { it.audio.publicationId }
    val state = ShellState(
        stack = i.navigation.stack.map { it.toShellRoute() },
        selectedEpisode = selected,
        positionSeconds = (positionMs / 1000L).toInt(),
        positionMs = positionMs,
        savedWorks = saved.map(::ShellWorkId).toSet(),
        libraryTab = i.libraryTab,
        query = i.query,
        catalogMode = worksWalk.catalogMode(),
        theme = i.theme,
        locale = i.locale,
        speedPercent = if (matchesSelection) (session?.speedPermille ?: 1000) / 10 else 100,
        timer = i.timer,
        playbackStatus = playbackStatus(i.playback, pending != null, i.accessFailed, matchesSelection),
        transportRequested = if (pending != null) !i.accessFailed
            else matchesSelection && session?.intent == PlaybackIntent.Play && session.failure == null,
        // Newest first from the API; the presentation orders its progress oldest first.
        progress = i.library.continueListening.asReversed().map { it.audio.toProgress(it.positionMs) },
        history = i.library.history.map { entry ->
            entry.audio.toProgress(continueByPublication[entry.audio.publicationId]?.positionMs ?: 0L)
        },
        listeningFailure = if (i.accessFailed && pending != null) ShellListeningFailure.ReleaseUnavailable else null,
    )
    return ShellProjection(state, view)
}

private fun AudioIdentity.toProgress(positionMs: Long): ShellProgress {
    val bounded = positionMs.coerceIn(0L, durationMs)
    return ShellProgress(ShellEpisodeId(unitId, ShellWorkId(workId)), (bounded / 1000L).toInt(), bounded)
}

private fun TheatreRoute.toShellRoute(): ShellRoute = when (this) {
    TheatreRoute.Discovery -> ShellRoute.Discovery
    TheatreRoute.Search -> ShellRoute.Search
    TheatreRoute.Library -> ShellRoute.Library
    TheatreRoute.Account -> ShellRoute.Account
    TheatreRoute.MembershipPreview -> ShellRoute.MembershipPreview
    TheatreRoute.Login -> ShellRoute.Login
    TheatreRoute.Register -> ShellRoute.Register
    TheatreRoute.History -> ShellRoute.History
    TheatreRoute.Settings -> ShellRoute.Settings
    TheatreRoute.Queue -> ShellRoute.Queue
    TheatreRoute.Player -> ShellRoute.Player
    TheatreRoute.Unavailable -> ShellRoute.Unavailable
    is TheatreRoute.Work -> ShellRoute.Work(ShellWorkId(key.value))
}

private fun WalkPhase.toListPhase(): ShellListPhase = when (this) {
    WalkPhase.NotStarted, WalkPhase.Complete -> ShellListPhase.Complete
    WalkPhase.Loading -> ShellListPhase.Loading
    WalkPhase.More -> ShellListPhase.More
    is WalkPhase.Failed -> ShellListPhase.Failed
}

/** An empty list is Loading, Error or Empty; a non-empty one is Ready with its footer below. */
private fun PageWalk<PublicWork>.catalogMode(): ShellCatalogMode = when {
    items.isNotEmpty() -> ShellCatalogMode.Ready
    phase is WalkPhase.Failed -> ShellCatalogMode.Error
    isEmptyResult -> ShellCatalogMode.Empty
    else -> ShellCatalogMode.Loading
}

private fun playbackStatus(playback: PlaybackState, pending: Boolean, accessFailed: Boolean, matches: Boolean): PlaybackUiStatus = when {
    pending -> if (accessFailed) PlaybackUiStatus.Failed else PlaybackUiStatus.Loading
    !matches -> PlaybackUiStatus.Idle
    else -> when (playback.status) {
        PlaybackStatus.Idle -> PlaybackUiStatus.Idle
        PlaybackStatus.Loading -> PlaybackUiStatus.Loading
        PlaybackStatus.Paused -> PlaybackUiStatus.Paused
        PlaybackStatus.Playing -> PlaybackUiStatus.Playing
        PlaybackStatus.Buffering -> PlaybackUiStatus.Buffering
        PlaybackStatus.Ended -> PlaybackUiStatus.Ended
        PlaybackStatus.Failed -> PlaybackUiStatus.Failed
        PlaybackStatus.Simulating -> PlaybackUiStatus.Failed
    }
}

/**
 * Collects the works the screens need and the units known for them. One entry per work id: the
 * authoritative API value when the process has one, otherwise a stub named with neutral wording.
 */
private class WorkBuilder(private val known: KnownCatalog, private val copy: ShellCopy) {
    private class Entry(var work: PublicWork?) {
        val units = LinkedHashMap<String, ShellEpisode>()
        var unitsPhase = ShellListPhase.More
        var detailPhase = ShellDetailPhase.Ready
        var fallbackTitle: String? = null
        /** The open detail's list is exactly what the API walked; nothing else joins it. */
        var authoritative = false
    }

    private val entries = LinkedHashMap<String, Entry>()
    private val extras = LinkedHashMap<String, ShellEpisode>()

    /** Units named by Library or the player that are not part of an authoritative list. */
    fun extraEpisodes(): List<ShellEpisode> = extras.values.toList()

    fun ids(): List<String> = entries.keys.toList()

    fun add(work: PublicWork) {
        entries.getOrPut(work.id.value) { Entry(work) }.work = work
    }

    fun stub(id: String) { entries.getOrPut(id) { Entry(null) } }

    fun reference(id: String) {
        entries.getOrPut(id) { Entry(known.work(id)) }
    }

    /** Titles of a unit named only by its audio identity: known metadata, a selection or neutral wording. */
    fun unit(audio: AudioIdentity, selection: NativeSelection?) {
        val entry = entries.getOrPut(audio.workId) { Entry(known.work(audio.workId)) }
        if (selection != null && entry.work == null) entry.fallbackTitle = selection.workTitle.takeIf(String::isNotBlank)
        if (audio.unitId in entry.units || audio.unitId in extras) return
        val knownUnit = known.unit(audio.unitId)?.unit
        val workId = ShellWorkId(audio.workId)
        (if (entry.authoritative) extras else entry.units)[audio.unitId] = ShellEpisode(
            id = ShellEpisodeId(audio.unitId, workId), workId = workId, index = entry.units.size + 1,
            title = knownUnit?.title ?: selection?.title ?: entry.work?.title ?: copy.text(CopyKey.Work),
            durationSeconds = seconds(audio.durationMs), numbered = false,
        )
    }

    fun requested(request: UnitRequest) {
        val entry = entries.getOrPut(request.workId) { Entry(known.work(request.workId)) }
        if (entry.work == null) entry.fallbackTitle = request.workTitle.takeIf(String::isNotBlank)
        if (request.unitId in entry.units || request.unitId in extras) return
        val unit = known.unit(request.unitId)?.unit
        val workId = ShellWorkId(request.workId)
        // Without a verified duration nothing can be played from here; the list row stays disabled.
        (if (entry.authoritative) extras else entry.units)[request.unitId] = ShellEpisode(
            ShellEpisodeId(request.unitId, workId), workId, entry.units.size + 1,
            request.title.ifBlank { unit?.title ?: copy.text(CopyKey.Work) },
            unit?.let { seconds(it.durationMs) } ?: 0, numbered = false,
        )
    }

    fun detail(id: String, phase: DetailPhase, units: PageWalk<PublicUnit>?) {
        val entry = entries.getOrPut(id) { Entry(known.work(id)) }
        entry.authoritative = true
        entry.units.clear()
        when (phase) {
            DetailPhase.Loading -> entry.detailPhase = ShellDetailPhase.Loading
            is DetailPhase.Failed -> entry.detailPhase =
                if (phase.failure == CatalogFailure.NotFound) ShellDetailPhase.NotFound else ShellDetailPhase.Failed
            is DetailPhase.Ready -> {
                entry.detailPhase = ShellDetailPhase.Ready
                units?.items?.forEach { unit -> entry.units[unit.id.value] = episode(id, unit, entry.units.size + 1) }
                entry.unitsPhase = units?.phase?.toListPhase() ?: ShellListPhase.Loading
                // A walk not yet started is still loading, never a completed empty list.
                if (units?.phase == WalkPhase.NotStarted) entry.unitsPhase = ShellListPhase.Loading
            }
        }
    }

    fun build(id: String): ShellWork? {
        val entry = entries[id] ?: return null
        val work = entry.work
        val workId = ShellWorkId(id)
        // Units from elsewhere (library, player) join those the API listed, without duplicates.
        val episodes = LinkedHashMap<String, ShellEpisode>()
        if (!entry.authoritative) known.unitsOf(id).forEach { episodes[it.id.value] = episode(id, it, episodes.size + 1) }
        entry.units.forEach { (unitId, episode) -> episodes[unitId] = episode }
        val ordered = episodes.values.mapIndexed { index, episode -> episode.copy(index = index + 1) }
        return if (work == null) {
            ShellWork(workId, entry.fallbackTitle ?: copy.text(CopyKey.Work), "", "", ordered,
                ShellUnitKind.Episode, entry.unitsPhase, entry.detailPhase)
        } else {
            ShellWork(workId, work.title, copy.text(work.kind.label()), work.description, ordered,
                if (work.kind.unitKind == PublicUnitKind.Chapter) ShellUnitKind.Chapter else ShellUnitKind.Episode,
                entry.unitsPhase, entry.detailPhase)
        }
    }

    private fun episode(workId: String, unit: PublicUnit, index: Int): ShellEpisode {
        val id = ShellWorkId(workId)
        return ShellEpisode(ShellEpisodeId(unit.id.value, id), id, index, unit.title, seconds(unit.durationMs), numbered = false)
    }

    private fun seconds(durationMs: Long): Int = ((durationMs + 999L) / 1000L).coerceIn(0L, Int.MAX_VALUE.toLong()).toInt()
}

internal fun PublicWorkKind.label(): CopyKey = when (this) {
    PublicWorkKind.Novel -> CopyKey.KindNovel
    PublicWorkKind.Story -> CopyKey.KindStory
    PublicWorkKind.Podcast -> CopyKey.KindPodcast
}
