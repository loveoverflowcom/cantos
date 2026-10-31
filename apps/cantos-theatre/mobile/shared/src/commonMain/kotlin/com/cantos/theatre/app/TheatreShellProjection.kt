package com.cantos.theatre.app

import com.cantos.theatre.application.catalog.CatalogFailure
import com.cantos.theatre.application.catalog.DetailPhase
import com.cantos.theatre.application.catalog.PageWalk
import com.cantos.theatre.application.catalog.PublicCatalogState
import com.cantos.theatre.application.catalog.WalkPhase
import com.cantos.theatre.application.playback.NativeSelection
import com.cantos.theatre.domain.catalog.PublicUnit
import com.cantos.theatre.domain.catalog.PublicCover
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkKind
import com.cantos.theatre.domain.catalog.PublicUnitKind
import com.cantos.theatre.domain.catalog.unitKind
import com.cantos.theatre.domain.library.BookmarkEntry
import com.cantos.theatre.domain.library.BookmarkSync
import com.cantos.theatre.domain.library.LibraryLoad
import com.cantos.theatre.domain.library.LibraryPresentation
import com.cantos.theatre.domain.library.LibraryResume
import com.cantos.theatre.domain.library.LibrarySection
import com.cantos.theatre.domain.library.LibraryState
import com.cantos.theatre.domain.library.LibraryUnitInfo
import com.cantos.theatre.domain.library.LibraryWorkInfo
import com.cantos.theatre.domain.library.LibraryWorkKind
import com.cantos.theatre.domain.library.valueOrNull
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackIntent
import com.cantos.theatre.domain.playback.PlaybackState
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.PlaybackUiStatus
import com.cantos.theatre.presentation.model.ShellBookmark
import com.cantos.theatre.presentation.model.ShellBookmarkSync
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellDetailPhase
import com.cantos.theatre.presentation.model.ShellEpisode
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellLibraryPhase
import com.cantos.theatre.presentation.model.ShellLibraryTab
import com.cantos.theatre.presentation.model.ShellLibraryView
import com.cantos.theatre.presentation.model.ShellListPhase
import com.cantos.theatre.presentation.model.ShellListeningFailure
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellResume
import com.cantos.theatre.presentation.model.ShellRowTitles
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.WorkListenRelease
import com.cantos.theatre.presentation.model.WorkListenPhase
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellTimer
import com.cantos.theatre.presentation.model.ShellSearchKind
import com.cantos.theatre.presentation.model.ShellUnitKind
import com.cantos.theatre.presentation.model.ShellCover
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

/** Metadata without an eligible publication cannot become a playback request. */
internal fun unitRequest(work: PublicWork, unit: PublicUnit): UnitRequest? =
    unit.publicationId?.let { UnitRequest(work.id.value, unit.id.value, it.value, unit.title, work.title) }

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
    val accessFailure: com.cantos.theatre.ports.PlaybackAccessFailure? = null,
    val searchKind: ShellSearchKind = ShellSearchKind.All,
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
    val savedEntries = i.library.saved.filter { it.saved }
    val saved = savedEntries.map { it.workId }
    // Saved first: its representative describes the Work; an exact release only describes its own row.
    savedEntries.forEach {
        builder.reference(it.workId)
        it.presentation.valueOrNull?.let(builder::describe)
        if (it.presentation == LibraryPresentation.Unavailable) builder.markUnavailable(it.workId)
    }
    i.library.continueListening.forEach { builder.reference(it.audio.workId); builder.unit(it.audio, null, it.presentation); it.presentation.valueOrNull?.let { unit -> builder.describe(unit.work) } }
    i.library.history.forEach { builder.reference(it.audio.workId); builder.unit(it.audio, null, it.presentation); it.presentation.valueOrNull?.let { unit -> builder.describe(unit.work) } }
    i.library.bookmarks.forEach { builder.reference(it.audio.workId); builder.unit(it.audio, null, it.presentation); it.presentation.valueOrNull?.let { unit -> builder.describe(unit.work) } }
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
        request = when {
            worksWalk.phase != WalkPhase.Loading -> com.cantos.theatre.presentation.model.ShellCatalogRequest.Idle
            worksWalk.replaceOnFirstPage && worksWalk.items.isNotEmpty() -> com.cantos.theatre.presentation.model.ShellCatalogRequest.Replacement
            worksWalk.requested != null -> com.cantos.theatre.presentation.model.ShellCatalogRequest.Append
            else -> com.cantos.theatre.presentation.model.ShellCatalogRequest.Initial
        },
        appliedSearch = i.catalog.appliedQuery?.text,
        referenced = referenced,
        referencedEpisodes = builder.extraEpisodes(),
        library = i.library.toLibraryView(),
    )

    val matchesSelection = selectedUnit != null && pending == null && audio?.unitId == selectedUnit
    val selected = selectedUnit?.let { unit -> ShellEpisodeId(unit, ShellWorkId(selectedWork ?: "none")) } ?: NoSelection
    val positionMs = if (matchesSelection) session?.positionMs ?: 0L else 0L
    val continueByPublication = i.library.continueListening.associateBy { it.audio.publicationId }
    fun row(audio: AudioIdentity, positionMs: Long, resume: LibraryResume, presentation: LibraryPresentation<LibraryUnitInfo>) =
        audio.toProgress(positionMs, resume.toShell(), builder.rowTitles(audio, presentation))
    val state = ShellState(
        stack = i.navigation.stack.map { it.toShellRoute() },
        selectedEpisode = selected,
        positionSeconds = (positionMs / 1000L).toInt(),
        positionMs = positionMs,
        savedWorks = saved.map(::ShellWorkId).toSet(),
        confirmedSavedWorks = i.library.saved.filter { it.confirmedSaved }.map { ShellWorkId(it.workId) }.toSet(),
        pendingSavedWorks = i.library.saved.filter { it.sync == com.cantos.theatre.domain.library.LibrarySync.Pending }.map { ShellWorkId(it.workId) }.toSet(),
        failedSavedWorks = i.library.saved.filter { it.sync == com.cantos.theatre.domain.library.LibrarySync.Failed }.map { ShellWorkId(it.workId) }.toSet(),
        retryableSavedWorks = i.library.saved.filter { it.sync == com.cantos.theatre.domain.library.LibrarySync.Failed && it.retryableFailure }.map { ShellWorkId(it.workId) }.toSet(),
        savedAccount = i.library.account,
        libraryTab = i.libraryTab,
        query = i.query,
        searchKind = i.searchKind,
        catalogMode = worksWalk.catalogMode(),
        theme = i.theme,
        locale = i.locale,
        speedPercent = if (matchesSelection) (session?.speedPermille ?: 1000) / 10 else 100,
        timer = i.timer,
        playbackStatus = playbackStatus(i.playback, pending != null, i.accessFailed, matchesSelection),
        transportRequested = if (pending != null) !i.accessFailed
            else matchesSelection && session?.intent == PlaybackIntent.Play && session.failure == null,
        // Newest first from the API; the presentation orders its progress oldest first.
        progress = i.library.continueListening.asReversed().map { row(it.audio, it.positionMs, it.resume, it.presentation) },
        history = i.library.history.map { entry ->
            row(entry.audio, continueByPublication[entry.audio.publicationId]?.positionMs ?: 0L, entry.resume, entry.presentation)
        },
        bookmarks = i.library.bookmarks.map { it.toShell(builder.rowTitles(it.audio, it.presentation)) },
        listeningFailure = if (i.accessFailed && pending != null) ShellListeningFailure.ReleaseUnavailable else null,
        workListenRelease = workListenRelease(i),
    )
    return ShellProjection(state, view)
}

private fun AudioIdentity.toProgress(positionMs: Long, resume: ShellResume, titles: ShellRowTitles): ShellProgress {
    val bounded = positionMs.coerceIn(0L, durationMs)
    return ShellProgress(ShellEpisodeId(unitId, ShellWorkId(workId)), (bounded / 1000L).toInt(), bounded, resume, titles,
        ((durationMs + 999L) / 1000L).coerceIn(0L, Int.MAX_VALUE.toLong()).toInt())
}

/** The exact release the bookmark names, with that release's own titles; nothing is remapped or borrowed. */
private fun BookmarkEntry.toShell(titles: ShellRowTitles): ShellBookmark {
    val bounded = positionMs.coerceIn(0L, audio.durationMs)
    return ShellBookmark(ShellEpisodeId(audio.unitId, ShellWorkId(audio.workId)), (bounded / 1000L).toInt(), bounded, id,
        sync.toShell(), resume.toShell(), titles, ((audio.durationMs + 999L) / 1000L).coerceIn(0L, Int.MAX_VALUE.toLong()).toInt())
}

private fun BookmarkSync.toShell(): ShellBookmarkSync = when (this) {
    BookmarkSync.Synced -> ShellBookmarkSync.Synced
    BookmarkSync.Pending -> ShellBookmarkSync.Pending
    BookmarkSync.Offline -> ShellBookmarkSync.Offline
    BookmarkSync.Failed -> ShellBookmarkSync.Failed
    BookmarkSync.DeleteFailed -> ShellBookmarkSync.DeleteFailed
}

private fun LibraryResume.toShell(): ShellResume = when (this) {
    LibraryResume.ExactAvailable -> ShellResume.Available
    LibraryResume.NeedsRefresh -> ShellResume.NeedsRefresh
    LibraryResume.ReleaseReplaced -> ShellResume.ReleaseReplaced
    LibraryResume.AccessUnavailable -> ShellResume.Unavailable
}

/** A guest has no Library, so nothing is loading or failed for it. */
private fun LibraryState.toLibraryView(): ShellLibraryView =
    if (account == null) ShellLibraryView()
    else ShellLibraryView(phase(LibrarySection.Continue, continueListening.isNotEmpty()), phase(LibrarySection.Saved, saved.any { it.saved }),
        phase(LibrarySection.History, history.isNotEmpty()), phase(LibrarySection.Bookmarks, bookmarks.isNotEmpty()))

private fun LibraryState.phase(section: LibrarySection, hasRows: Boolean): ShellLibraryPhase {
    val state = section(section)
    return when {
        state.loading == LibraryLoad.NextPage -> ShellLibraryPhase.LoadingMore
        state.loading == LibraryLoad.Refreshing -> ShellLibraryPhase.Loading
        state.failure?.nextPage == true -> ShellLibraryPhase.MoreFailed
        state.failure != null -> if (hasRows) ShellLibraryPhase.Stale else ShellLibraryPhase.Failed
        !state.loaded -> ShellLibraryPhase.Loading
        cursors[section] != null -> ShellLibraryPhase.More
        else -> ShellLibraryPhase.Complete
    }
}

private fun TheatreRoute.toShellRoute(): ShellRoute = when (this) {
    TheatreRoute.Discovery -> ShellRoute.Discovery
    TheatreRoute.Search -> ShellRoute.Search
    TheatreRoute.Library -> ShellRoute.Library
    TheatreRoute.Account -> ShellRoute.Account
    TheatreRoute.MembershipPreview -> ShellRoute.MembershipPreview
    TheatreRoute.Login -> ShellRoute.Login
    TheatreRoute.Register -> ShellRoute.Register
    TheatreRoute.VerifyEmail -> ShellRoute.VerifyEmail
    TheatreRoute.History -> ShellRoute.History
    TheatreRoute.Settings -> ShellRoute.Settings
    TheatreRoute.Queue -> ShellRoute.Queue
    TheatreRoute.Player -> ShellRoute.Player
    TheatreRoute.Unavailable -> ShellRoute.Unavailable
    is TheatreRoute.Work -> ShellRoute.Work(ShellWorkId(key.value))
    is TheatreRoute.SharedWork -> ShellRoute.SharedWork(ShellWorkId(id.value))
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

private fun workListenRelease(i: TheatreShellInputs): WorkListenRelease? {
    val session = i.playback.session
    val audio = session?.source?.audio
    val pending = i.requested?.takeIf { audio == null || it.workId != audio.workId || it.unitId != audio.unitId || it.publicationId != audio.publicationId }
    if (pending != null) return WorkListenRelease(ShellEpisodeId(pending.unitId, ShellWorkId(pending.workId)), pending.publicationId,
        when (i.accessFailure) {
            com.cantos.theatre.ports.PlaybackAccessFailure.NotFound,
            com.cantos.theatre.ports.PlaybackAccessFailure.IdentityChanged -> WorkListenPhase.Unavailable
            else -> if (i.accessFailed) WorkListenPhase.ResolveFailed else WorkListenPhase.Resolving
        })
    if (session == null || audio == null) return null
    val phase = when {
        session.failure != null -> WorkListenPhase.TransportFailed
        session.intent == PlaybackIntent.Play -> when (i.playback.status) {
            PlaybackStatus.Playing -> WorkListenPhase.PausePlaying
            PlaybackStatus.Buffering -> WorkListenPhase.PauseBuffering
            else -> WorkListenPhase.PauseLoading
        }
        i.playback.status == PlaybackStatus.Paused && session.positionMs > 0L -> WorkListenPhase.Resume
        else -> WorkListenPhase.Ready
    }
    return WorkListenRelease(ShellEpisodeId(audio.unitId, ShellWorkId(audio.workId)), audio.publicationId, phase)
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
        /** The API's released metadata for a Work the catalog has not shown; the catalog value wins when present. */
        var described: LibraryWorkInfo? = null
        /** The API's explicit null for this Work's retained record; an unread record says nothing. */
        var unavailable = false
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

    fun markUnavailable(id: String) {
        entries.getOrPut(id) { Entry(known.work(id)) }.unavailable = true
    }

    fun describe(info: LibraryWorkInfo) {
        val entry = entries.getOrPut(info.workId) { Entry(known.work(info.workId)) }
        if (entry.described == null) entry.described = info
    }

    /** A catalog unit names [audio] only when it is the same release; a newer release is another recording. */
    private fun exactUnit(audio: AudioIdentity): PublicUnit? =
        known.unit(audio.unitId)?.unit?.takeIf { it.publicationId?.value == audio.publicationId }

    /** Row titles, resolved once: the API's metadata for this exact release, else only what is provably it. */
    fun rowTitles(audio: AudioIdentity, presentation: LibraryPresentation<LibraryUnitInfo>): ShellRowTitles {
        val info = presentation.valueOrNull
        // The API's null says it cannot name this release: no catalog title (of a newer release) fills in.
        val catalogWork = if (presentation == LibraryPresentation.Unavailable) null else known.work(audio.workId)
        val workTitle = info?.work?.title ?: catalogWork?.title ?: copy.text(CopyKey.Work)
        val exact = if (presentation == LibraryPresentation.Unavailable) null else exactUnit(audio)
        return ShellRowTitles(workTitle, info?.unitTitle ?: exact?.title ?: workTitle)
    }

    /** Titles of a unit named only by its audio identity: API metadata, known metadata of that release, a selection or neutral wording. */
    fun unit(audio: AudioIdentity, selection: NativeSelection?, presentation: LibraryPresentation<LibraryUnitInfo> = LibraryPresentation.Unread) {
        val entry = entries.getOrPut(audio.workId) { Entry(known.work(audio.workId)) }
        if (selection != null && entry.work == null) entry.fallbackTitle = selection.workTitle.takeIf(String::isNotBlank)
        if (audio.unitId in entry.units || audio.unitId in extras) return
        val workId = ShellWorkId(audio.workId)
        (if (entry.authoritative) extras else entry.units)[audio.unitId] = ShellEpisode(
            id = ShellEpisodeId(audio.unitId, workId), workId = workId, index = entry.units.size + 1,
            title = presentation.valueOrNull?.unitTitle ?: exactUnit(audio)?.title ?: selection?.title ?: entry.work?.title ?: copy.text(CopyKey.Work),
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
            unit?.durationMs?.let(::seconds) ?: 0, numbered = false,
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
        val described = entry.described
        return if (work == null && described != null) {
            val kind = described.kind.toPublic()
            ShellWork(workId, described.title, copy.text(kind.label()), "", ordered,
                if (kind.unitKind == PublicUnitKind.Chapter) ShellUnitKind.Chapter else ShellUnitKind.Episode,
                entry.unitsPhase, entry.detailPhase)
        } else if (work == null) {
            ShellWork(workId, entry.fallbackTitle ?: copy.text(CopyKey.Work), "", "", ordered,
                ShellUnitKind.Episode, entry.unitsPhase, entry.detailPhase, unavailable = entry.unavailable && entry.fallbackTitle == null)
        } else {
            ShellWork(workId, work.title, copy.text(work.kind.label()), work.description, ordered,
                if (work.kind.unitKind == PublicUnitKind.Chapter) ShellUnitKind.Chapter else ShellUnitKind.Episode,
                entry.unitsPhase, entry.detailPhase, cover = work.cover?.let(::shellCover))
        }
    }

    private fun episode(workId: String, unit: PublicUnit, index: Int): ShellEpisode {
        val id = ShellWorkId(workId)
        return ShellEpisode(ShellEpisodeId(unit.id.value, id), id, index, unit.title, unit.durationMs?.let(::seconds) ?: 0, numbered = false, publicationId = unit.publicationId?.value)
    }

    private fun seconds(durationMs: Long): Int = ((durationMs + 999L) / 1000L).coerceIn(0L, Int.MAX_VALUE.toLong()).toInt()
}

private fun shellCover(cover: PublicCover) = ShellCover(
    cover.url, cover.sha256, cover.byteLength, cover.contentType.mime, cover.widthPx, cover.heightPx, cover.altText,
)

private fun LibraryWorkKind.toPublic(): PublicWorkKind = when (this) {
    LibraryWorkKind.Novel -> PublicWorkKind.Novel
    LibraryWorkKind.Story -> PublicWorkKind.Story
    LibraryWorkKind.Podcast -> PublicWorkKind.Podcast
}

internal fun PublicWorkKind.label(): CopyKey = when (this) {
    PublicWorkKind.Novel -> CopyKey.KindNovel
    PublicWorkKind.Story -> CopyKey.KindStory
    PublicWorkKind.Podcast -> CopyKey.KindPodcast
}
