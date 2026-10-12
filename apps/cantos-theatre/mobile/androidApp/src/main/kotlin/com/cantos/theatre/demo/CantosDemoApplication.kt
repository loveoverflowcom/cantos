package com.cantos.theatre.demo

import android.app.Application
import com.cantos.theatre.app.AppContainer
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel

/** Process-lifetime mock owner, retained when an Activity or a route is recreated. */
class CantosDemoApplication : Application() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    val container: AppContainer by lazy { AppContainer.mock(scope) }

    override fun onTerminate() {
        // Emulator shutdown only; ordinary process death loses all mock memory.
        container.close()
        scope.cancel()
        super.onTerminate()
    }
}
