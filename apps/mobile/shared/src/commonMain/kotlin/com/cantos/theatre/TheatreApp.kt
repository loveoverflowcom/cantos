package com.cantos.theatre

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.cantos.theatre.design.CantosTheme
import com.cantos.theatre.design.GeneratedTokens
import com.cantos.theatre.design.TheatreBackHandler
import com.cantos.theatre.navigation.DemoState
import com.cantos.theatre.navigation.Navigation
import com.cantos.theatre.navigation.Screen
import com.cantos.theatre.screens.DiscoverScreen
import com.cantos.theatre.screens.Heading
import com.cantos.theatre.screens.Key
import com.cantos.theatre.screens.PlayerScreen
import com.cantos.theatre.screens.SearchScreen
import com.cantos.theatre.screens.WorkScreen
import com.cantos.theatre.screens.copy

private val navigationSaver = listSaver<Navigation, String>(
    save = { state -> state.stack.map(Screen::name) },
    restore = { names ->
        Navigation(names.mapNotNull { name -> Screen.entries.find { it.name == name } }.ifEmpty { listOf(Screen.Discover) })
    },
)

private fun title(screen: Screen): Key = when (screen) {
    Screen.Discover -> Key.Tagline
    Screen.Search -> Key.Search
    Screen.Work -> Key.Work
    Screen.Library -> Key.Library
    Screen.Player -> Key.Player
}

private fun stateTitle(state: DemoState): Key = when (state) {
    DemoState.Ready -> Key.Ready
    DemoState.Loading -> Key.Loading
    DemoState.Empty -> Key.Empty
    DemoState.Error -> Key.Error
    DemoState.NotImplemented -> Key.NotImplemented
}

@Composable
fun TheatreApp() {
    var navigation by rememberSaveable(stateSaver = navigationSaver) { mutableStateOf(Navigation()) }
    var english by rememberSaveable { mutableStateOf(false) }
    val systemDark = isSystemInDarkTheme()
    var dark by rememberSaveable { mutableStateOf(systemDark) }
    var query by rememberSaveable { mutableStateOf("") }
    var demoState by remember { mutableStateOf(DemoState.Ready) }
    var menuOpen by remember { mutableStateOf(false) }
    val open: (Screen) -> Unit = { screen -> navigation = navigation.open(screen); demoState = DemoState.Ready }
    TheatreBackHandler(enabled = navigation.canGoBack) { navigation = navigation.back(); demoState = DemoState.Ready }
    CantosTheme(dark = dark) {
        Scaffold(
            containerColor = MaterialTheme.colorScheme.background,
            topBar = {
                FlowRow(Modifier.fillMaxWidth().statusBarsPadding().padding(horizontal = GeneratedTokens.space_sm.dp)) {
                    TextButton(onClick = { navigation = Navigation(); demoState = DemoState.Ready }) { Text("cantos · Theatre") }
                    TextButton(onClick = { english = !english }) { Text(if (english) "Tiếng Việt" else "English") }
                    TextButton(onClick = { dark = !dark }) { Text(copy(if (dark) Key.Light else Key.Dark, english)) }
                }
            },
            bottomBar = {
                Column(Modifier.navigationBarsPadding()) {
                    Surface(color = MaterialTheme.colorScheme.primaryContainer) {
                        Column(Modifier.fillMaxWidth().padding(horizontal = GeneratedTokens.space_md.dp)) {
                            TextButton(onClick = { open(Screen.Player) }) { Text(copy(Key.Mini, english)) }
                            Text(copy(Key.Demo, english), style = MaterialTheme.typography.labelSmall)
                        }
                    }
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceEvenly) {
                        for ((screen, key) in listOf(Screen.Discover to Key.Discover, Screen.Search to Key.Search, Screen.Library to Key.Library)) {
                            TextButton(
                                onClick = { navigation = navigation.tab(screen); demoState = DemoState.Ready },
                                modifier = Modifier.weight(1f).semantics { selected = navigation.current == screen },
                            ) { Text(copy(key, english)) }
                        }
                    }
                }
            },
        ) { insets ->
            LazyColumn(
                Modifier.fillMaxSize().padding(insets),
                contentPadding = PaddingValues(GeneratedTokens.space_md.dp),
                verticalArrangement = Arrangement.spacedBy(GeneratedTokens.space_md.dp),
            ) {
                item {
                    if (navigation.canGoBack) TextButton(onClick = { navigation = navigation.back(); demoState = DemoState.Ready }) { Text(copy(Key.Back, english)) }
                    Heading(copy(title(navigation.current), english))
                    Text(copy(Key.Demo, english), style = MaterialTheme.typography.labelSmall)
                    TextButton(onClick = { menuOpen = true }) { Text(copy(Key.DemoStates, english)) }
                    DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                        DemoState.entries.forEach { state ->
                            DropdownMenuItem(text = { Text(copy(stateTitle(state), english)) }, onClick = { demoState = state; menuOpen = false })
                        }
                    }
                }
                item {
                    Column(verticalArrangement = Arrangement.spacedBy(GeneratedTokens.space_md.dp)) {
                        if (demoState != DemoState.Ready) {
                            Text(copy(stateTitle(demoState), english))
                            TextButton(onClick = { demoState = DemoState.Ready }) { Text(copy(Key.Reset, english)) }
                        } else {
                            when (navigation.current) {
                                Screen.Discover -> DiscoverScreen(english, open)
                                Screen.Search -> SearchScreen(english, query, { query = it }, open)
                                Screen.Work -> WorkScreen(english, open)
                                Screen.Library -> Text(copy(Key.LibraryUnavailable, english))
                                Screen.Player -> PlayerScreen(english)
                            }
                        }
                    }
                }
            }
        }
    }
}
