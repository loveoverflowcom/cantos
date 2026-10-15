package com.cantos.theatre.adapters.mock

import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey
import kotlin.test.Test
import kotlin.test.assertNull

class DemoMappingTest {
    @Test
    fun unknown_fixture_keys_never_gain_diagnostic_presentation_bindings() {
        listOf("", "future", "Sơn Hà Ký").forEach { key ->
            val work = key.takeIf { it.isNotBlank() }?.let(::WorkKey)
            assertNull(work?.toDemoWorkId())
            assertNull(work?.let { EpisodeKey(it, 0) }?.toDemoEpisodeId())
        }
    }
}
