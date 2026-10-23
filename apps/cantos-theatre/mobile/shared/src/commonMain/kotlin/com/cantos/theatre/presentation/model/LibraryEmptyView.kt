package com.cantos.theatre.presentation.model

/** App-owned feedback vocabulary; glyph names resolve through the canonical offline registry. */
enum class LibraryEmptyKind(val title: CopyKey, val help: CopyKey, val label: CopyKey, val summary: CopyKey, val icon: String) {
    Library(CopyKey.LibraryEmpty, CopyKey.LibraryEmptyNote, CopyKey.Library, CopyKey.LibraryEmptyNote, "menu_book"),
    Continue(CopyKey.NoContinue, CopyKey.NoContinueHelp, CopyKey.Continuing, CopyKey.ContinueEmptySummary, "play_arrow"),
    Saved(CopyKey.NoSaved, CopyKey.NoSavedHelp, CopyKey.SavedTab, CopyKey.SavedEmptySummary, "menu_book"),
    Bookmarks(CopyKey.NoBookmarks, CopyKey.NoBookmarksHelp, CopyKey.Bookmarks, CopyKey.BookmarksEmptySummary, "star"),
    History(CopyKey.NoHistory, CopyKey.NoHistoryHelp, CopyKey.History, CopyKey.HistoryEmptySummary, "queue_music"),
}

/** Missing metadata is retained content, and a nonterminal read never establishes emptiness. */
fun emptyLibrarySections(state: ShellState, view: ShellLibraryView, capabilities: ShellCapabilities): Set<ShellLibrarySection> =
    ShellLibrarySection.entries.filter { section ->
        view.phase(section) == ShellLibraryPhase.Complete && when (section) {
            ShellLibrarySection.Continue -> state.progress.isEmpty()
            ShellLibrarySection.Saved -> state.savedWorks.isEmpty()
            ShellLibrarySection.History -> state.history.isEmpty()
            ShellLibrarySection.Bookmarks -> capabilities.bookmarks && state.bookmarks.isEmpty()
        }
    }.toSet()

fun isLibraryCompletelyEmpty(state: ShellState, view: ShellLibraryView, capabilities: ShellCapabilities): Boolean {
    val supported = ShellLibrarySection.entries.filter { it != ShellLibrarySection.Bookmarks || capabilities.bookmarks }
    return emptyLibrarySections(state, view, capabilities).containsAll(supported)
}
