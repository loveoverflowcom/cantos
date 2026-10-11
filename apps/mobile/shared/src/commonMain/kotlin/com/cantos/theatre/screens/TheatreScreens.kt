package com.cantos.theatre.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.cantos.theatre.design.GeneratedTokens
import com.cantos.theatre.navigation.Screen

// Synthetic display data, not a listener API or a publication manifest.
val demoTitles: List<String> = listOf("Sơn Hà Ký", "Đèn khuya bên sông", "Mộng hồ điệp")

fun matchingTitles(query: String): List<String> =
    demoTitles.filter { it.lowercase().contains(query.trim().lowercase()) }

@Composable
fun Heading(value: String) {
    Text(value, style = MaterialTheme.typography.headlineMedium, modifier = Modifier.semantics { heading() })
}

@Composable
fun ArtworkPlaceholder(title: String, english: Boolean) {
    val description = copy(Key.MissingArt, english)
    Surface(
        modifier = Modifier.fillMaxWidth().semantics { role = Role.Image; contentDescription = description },
        color = MaterialTheme.colorScheme.surfaceContainer,
        shape = MaterialTheme.shapes.medium,
    ) {
        Column(Modifier.padding(GeneratedTokens.space_lg.dp), verticalArrangement = Arrangement.spacedBy(GeneratedTokens.space_md.dp)) {
            Text(title, style = MaterialTheme.typography.titleLarge)
            Text(description, style = MaterialTheme.typography.labelSmall)
        }
    }
}

@Composable
fun DiscoverScreen(english: Boolean, open: (Screen) -> Unit) {
    Text(copy(Key.Featured, english), style = MaterialTheme.typography.labelSmall)
    ArtworkPlaceholder("Sơn Hà Ký", english)
    Text(copy(Key.Synopsis, english))
    Button(onClick = { open(Screen.Work) }) { Text(copy(Key.Work, english)) }
    Heading(copy(Key.Stories, english))
    demoTitles.drop(1).forEach { title ->
        Text(title, style = MaterialTheme.typography.titleMedium)
        Text(copy(Key.NotImplemented, english), style = MaterialTheme.typography.labelSmall)
    }
}

@Composable
fun SearchScreen(english: Boolean, query: String, change: (String) -> Unit, open: (Screen) -> Unit) {
    OutlinedTextField(value = query, onValueChange = change, label = { Text(copy(Key.SearchHint, english)) }, modifier = Modifier.fillMaxWidth())
    val titles = matchingTitles(query)
    if (titles.isEmpty()) Text(copy(Key.Empty, english))
    titles.forEach { title ->
        if (title == demoTitles.first()) {
            TextButton(onClick = { open(Screen.Work) }) { Text(title) }
        } else {
            Text(title)
            Text(copy(Key.NotImplemented, english), style = MaterialTheme.typography.labelSmall)
        }
    }
}

@Composable
fun WorkScreen(english: Boolean, open: (Screen) -> Unit) {
    ArtworkPlaceholder("Sơn Hà Ký", english)
    Text(copy(Key.Synopsis, english))
    Heading(copy(Key.Episodes, english))
    Text("01 · ${copy(Key.Episode, english)}")
    Button(onClick = { open(Screen.Player) }) { Text(copy(Key.OpenPlayer, english)) }
}

@Composable
fun PlayerScreen(english: Boolean) {
    ArtworkPlaceholder("Sơn Hà Ký", english)
    Heading(copy(Key.Episode, english))
    Text(copy(Key.Unavailable, english))
    for (key in listOf(Key.Play, Key.Save, Key.Download)) {
        Button(onClick = {}, enabled = false) { Text(copy(key, english)) }
    }
}
