package com.cantos.theatre.navigation

enum class Screen { Discover, Search, Work, Library, Player }

// Presentation state only. No auth, progress or publication policy lives here.
data class Navigation(val stack: List<Screen> = listOf(Screen.Discover)) {
    val current: Screen get() = stack.lastOrNull() ?: Screen.Discover
    val canGoBack: Boolean get() = stack.size > 1

    fun open(screen: Screen): Navigation =
        if (screen == current) this else copy(stack = stack + screen)

    fun back(): Navigation = if (canGoBack) copy(stack = stack.dropLast(1)) else this

    fun tab(screen: Screen): Navigation = Navigation(listOf(screen))
}

enum class DemoState { Ready, Loading, Empty, Error, NotImplemented }

// There is no media implementation. Never model a successful play or download.
object PlaybackBoundary {
    val state: DemoState = DemoState.NotImplemented
}
