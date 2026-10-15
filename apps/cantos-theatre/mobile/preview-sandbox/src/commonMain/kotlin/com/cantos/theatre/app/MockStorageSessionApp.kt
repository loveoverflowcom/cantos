package com.cantos.theatre.app

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import com.cantos.theatre.adapters.mock.sandbox.database.MockStorageDatabase
import com.cantos.theatre.adapters.mock.sandbox.sessions.MockStorageSession
import com.cantos.theatre.adapters.mock.sandbox.sessions.seed
import com.cantos.theatre.presentation.model.ShellState
import kotlinx.coroutines.CoroutineScope

class MockStorageSessionMount internal constructor(
    val seed: ShellState,
    val app: AppContainer,
    val database: MockStorageDatabase,
)

/** Composition-root wiring: the database starts with exactly the controller's start snapshot. */
fun mountMockStorageSession(scope: CoroutineScope, session: MockStorageSession): MockStorageSessionMount {
    val seed = session.seed()
    var database: MockStorageDatabase? = null
    val app = mockAppContainer(scope, seed) { start -> MockStorageDatabase(start).also { database = it } }
    return MockStorageSessionMount(seed, app, checkNotNull(database))
}

/** The diagnostic app root over one mock storage session; it owns and closes that mount. */
@Composable
fun MockStorageSessionApp(session: MockStorageSession, modifier: Modifier = Modifier, textScale: Float = 1f) {
    val scope = rememberCoroutineScope()
    val mount = remember(session) { mountMockStorageSession(scope, session) }
    DisposableEffect(mount) {
        onDispose { mount.app.close() }
    }
    CantosDemoApp(modifier, initialState = mount.seed, textScale = textScale, appContainer = mount.app)
}
