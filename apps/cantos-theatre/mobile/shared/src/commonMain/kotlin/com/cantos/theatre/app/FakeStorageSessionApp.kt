package com.cantos.theatre.app

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import com.cantos.theatre.adapters.mock.sandbox.database.FakeStorageDatabase
import com.cantos.theatre.adapters.mock.sandbox.sessions.FakeStorageSession
import com.cantos.theatre.adapters.mock.sandbox.sessions.seed
import com.cantos.theatre.demo.DemoState
import kotlinx.coroutines.CoroutineScope

class FakeStorageSessionMount internal constructor(
    val seed: DemoState,
    val app: AppContainer,
    val database: FakeStorageDatabase,
)

/** Composition-root wiring: the database starts with exactly the controller's start snapshot. */
fun mountFakeStorageSession(scope: CoroutineScope, session: FakeStorageSession): FakeStorageSessionMount {
    val seed = session.seed()
    var database: FakeStorageDatabase? = null
    val app = AppContainer.mock(scope, seed) { start -> FakeStorageDatabase(start).also { database = it } }
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
