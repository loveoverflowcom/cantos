package com.cantos.theatre.app

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import com.cantos.theatre.adapters.mock.FakePersistentProgressPort
import com.cantos.theatre.adapters.mock.FakeStorageDatabase
import com.cantos.theatre.demo.DemoBookmark
import com.cantos.theatre.demo.DemoEpisodeId
import com.cantos.theatre.demo.DemoProgress
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoWorkId
import kotlinx.coroutines.CoroutineScope

/** Static diagnostic sessions over the fake storage simulator; neither is durable persistence. */
enum class FakeStorageSession { Fresh, Restored }

/** Fresh holds no listening record; Restored holds every kind in one consistent snapshot. */
fun FakeStorageSession.seed(): DemoState = when (this) {
    FakeStorageSession.Fresh -> DemoState(
        stack = listOf(DemoRoute.Library),
        selectedEpisode = DemoEpisodeId.SonHa01,
        positionSeconds = 0,
        savedWorks = emptySet(),
        progress = emptyList(),
    )
    FakeStorageSession.Restored -> {
        val points = listOf(DemoProgress(DemoEpisodeId.DenKhua01, 91), DemoProgress(DemoEpisodeId.SonHa03, 768))
        DemoState(
            stack = listOf(DemoRoute.Library),
            selectedEpisode = DemoEpisodeId.SonHa03,
            positionSeconds = 768,
            savedWorks = setOf(DemoWorkId.SonHa, DemoWorkId.DenKhua),
            bookmarks = listOf(DemoBookmark(DemoEpisodeId.SonHa03, 600)),
            progress = points,
            history = points.reversed(),
        )
    }
}

class FakeStorageSessionMount internal constructor(
    val seed: DemoState,
    val app: AppContainer,
    val database: FakeStorageDatabase,
)

/** The database starts with exactly the listening snapshot the controller starts with. */
fun mountFakeStorageSession(scope: CoroutineScope, session: FakeStorageSession): FakeStorageSessionMount {
    val seed = session.seed()
    var database: FakeStorageDatabase? = null
    val app = AppContainer.mock(scope, seed) { start ->
        FakePersistentProgressPort(FakeStorageDatabase(start).also { database = it })
    }
    return FakeStorageSessionMount(seed, app, checkNotNull(database))
}

/** The production app root over one fake storage session; it owns and closes that mount. */
@Composable
fun FakeStorageSessionApp(session: FakeStorageSession, modifier: Modifier = Modifier, textScale: Float = 1f) {
    val scope = rememberCoroutineScope()
    val mount = remember(session) { mountFakeStorageSession(scope, session) }
    DisposableEffect(mount) {
        onDispose { mount.app.close() }
    }
    CantosDemoApp(modifier, initialState = mount.seed, textScale = textScale, appContainer = mount.app)
}
