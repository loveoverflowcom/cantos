package com.cantos.theatre.domain.publication

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull

class MockCatalogTest {
    private val key = WorkKey("son-ha-ky")
    private val first = CatalogEpisode(EpisodeKey(key, 0), "Bến sông", 60_000, CatalogOrigin.SyntheticDemo)
    private val second = CatalogEpisode(EpisodeKey(key, 1), "Đường qua núi", 90_000, CatalogOrigin.SyntheticDemo)

    @Test
    fun catalog_snapshots_do_not_alias_constructor_or_returned_collection_views() {
        val episodes = mutableListOf(first, second)
        val work = CatalogWork(key, "Sơn Hà Ký", "Tiên hiệp", "Nguyên bản minh họa", episodes, CatalogOrigin.SyntheticDemo)
        episodes.clear()
        assertEquals(listOf(first, second), work.episodes)
        (work.episodes as MutableList<*>).clear()
        assertEquals(listOf(first, second), work.episodes)

        val otherKey = WorkKey("den-khuya")
        val other = CatalogWork(otherKey, "Đèn Khuya", "Đời thường", "Minh họa", listOf(
            CatalogEpisode(EpisodeKey(otherKey, 0), "Ánh đèn", 60_000),
        ))
        val works = mutableListOf(work, other)
        val inventory = CatalogInventory(works)
        works.clear()
        (inventory.works as MutableList<*>).clear()
        assertEquals(listOf(work, other), inventory.works)
        assertEquals(first, inventory.episode(first.key))
        assertNull(inventory.episode(EpisodeKey(key, 3)))
        assertEquals(CatalogOrigin.SyntheticDemo, work.origin)
        assertEquals(CatalogOrigin.SyntheticDemo, first.origin)
    }

    @Test
    fun malformed_and_duplicate_mock_identity_cannot_enter_catalog() {
        assertFailsWith<IllegalArgumentException> { WorkKey(" ") }
        assertFailsWith<IllegalArgumentException> { EpisodeKey(key, -1) }
        assertFailsWith<IllegalArgumentException> { CatalogEpisode(first.key, "", 60_000) }
        assertFailsWith<IllegalArgumentException> { CatalogEpisode(first.key, "Bến sông", 0) }
        assertFailsWith<IllegalArgumentException> { CatalogWork(key, "Sơn Hà", "", "", emptyList()) }
        assertFailsWith<IllegalArgumentException> { CatalogWork(key, "Sơn Hà", "", "", listOf(first, first)) }
        assertFailsWith<IllegalArgumentException> {
            CatalogWork(WorkKey("den-khuya"), "Đèn Khuya", "", "", listOf(first))
        }
        val work = CatalogWork(key, "Sơn Hà", "", "", listOf(first))
        assertFailsWith<IllegalArgumentException> { CatalogInventory(listOf(work, work)) }
    }
}
