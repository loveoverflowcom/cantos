package com.cantos.theatre.domain.catalog

import com.cantos.theatre.domain.catalog.PublicCatalogSamples.cursor
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.publicationId
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.unit
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.unitId
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.valid
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.work
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.workId
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

class PublicCatalogTest {
    private fun assertRefused(expected: CatalogViolation, check: CatalogCheck<*>) =
        assertEquals(CatalogCheck.Invalid(expected), check)

    @Test
    fun identities_accept_only_canonical_lower_case_uuids() {
        val canonical = "abcdef01-0000-4000-8000-00000000000a"
        assertEquals(canonical, PublicWorkId.parse(canonical).valid().value)
        for (raw in listOf("", canonical.uppercase(), "{$canonical}", "$canonical ", "10000000000040008000000000000001",
            "10000000-0000-4000-8000-00000000000g", "1", "slug-of-a-title")) {
            assertRefused(CatalogViolation.MalformedWorkId, PublicWorkId.parse(raw))
            assertRefused(CatalogViolation.MalformedUnitId, PublicUnitId.parse(raw))
            assertRefused(CatalogViolation.MalformedPublicationId, PublicationId.parse(raw))
        }
    }

    @Test
    fun cursors_are_opaque_lc1_references_only() {
        assertIs<CatalogCheck.Valid<CatalogCursor>>(CatalogCursor.parse("lc1_" + "a".repeat(64)))
        for (raw in listOf("", "lc1_", "lc1_" + "a".repeat(63), "lc1_" + "A".repeat(64), "lc2_" + "a".repeat(64),
            "lc1_" + "a".repeat(64) + "&limit=50", "eyJvZmZzZXQiOjIwfQ==")) {
            assertRefused(CatalogViolation.MalformedCursor, CatalogCursor.parse(raw))
        }
    }

    @Test
    fun work_and_unit_values_check_title_duration_position_and_time() {
        assertRefused(CatalogViolation.BlankTitle, PublicWork.of(workId(1), PublicWorkKind.Story, " \n", "", "vi-VN", 0))
        assertRefused(CatalogViolation.TimestampOutOfRange, PublicWork.of(workId(1), PublicWorkKind.Story, "A", "", "vi", -1))
        fun unitOf(position: Long = 0, title: String = "Chương một", duration: Long = 1, published: Long = 0) =
            PublicUnit.of(unitId(1), PublicUnitKind.Chapter, position, title, "", publicationId(1), published, duration)
        assertIs<CatalogCheck.Valid<PublicUnit>>(unitOf(duration = 1))
        assertIs<CatalogCheck.Valid<PublicUnit>>(unitOf(position = Int.MAX_VALUE.toLong(), duration = 3_600_000))
        assertRefused(CatalogViolation.NonPositiveDuration, unitOf(duration = 0))
        assertRefused(CatalogViolation.NonPositiveDuration, unitOf(duration = -5))
        assertRefused(CatalogViolation.NonPositiveDuration, unitOf(duration = MaxSafeMillis + 1))
        assertRefused(CatalogViolation.PositionOutOfRange, unitOf(position = -1))
        assertRefused(CatalogViolation.PositionOutOfRange, unitOf(position = Int.MAX_VALUE.toLong() + 1))
        assertRefused(CatalogViolation.BlankTitle, unitOf(title = ""))
        assertRefused(CatalogViolation.TimestampOutOfRange, unitOf(published = MaxSafeMillis + 1))
    }

    @Test
    fun unit_pages_bind_the_requested_work_and_its_kind() {
        val story = work(1, PublicWorkKind.Story)
        val novel = work(2, PublicWorkKind.Novel)
        val podcast = work(3, PublicWorkKind.Podcast)
        val chapter = unit(1, PublicUnitKind.Chapter)
        val episode = unit(2, PublicUnitKind.Episode)
        assertIs<CatalogCheck.Valid<PublicUnitPage>>(PublicUnitPage.of(story, story.id, listOf(chapter), null))
        assertIs<CatalogCheck.Valid<PublicUnitPage>>(PublicUnitPage.of(novel, novel.id, listOf(chapter), null))
        assertIs<CatalogCheck.Valid<PublicUnitPage>>(PublicUnitPage.of(podcast, podcast.id, listOf(episode), cursor(1)))
        assertRefused(CatalogViolation.UnitKindMismatch, PublicUnitPage.of(story, story.id, listOf(episode), null))
        assertRefused(CatalogViolation.UnitKindMismatch, PublicUnitPage.of(podcast, podcast.id, listOf(chapter), null))
        assertRefused(CatalogViolation.WorkMismatch, PublicUnitPage.of(story, novel.id, listOf(chapter), null))
        // Same id twice, or two units claiming one release, is never two rows.
        assertRefused(CatalogViolation.DuplicateUnit, PublicUnitPage.of(story, story.id, listOf(chapter, unit(1, publication = 9)), null))
        assertRefused(CatalogViolation.DuplicatePublication, PublicUnitPage.of(story, story.id, listOf(chapter, unit(5, publication = 1)), null))
        assertRefused(CatalogViolation.PageTooLarge, PublicUnitPage.of(story, story.id, (1..51).map { unit(it) }, null))
        // Display order only: equal and gapped positions are admitted.
        assertIs<CatalogCheck.Valid<PublicUnitPage>>(
            PublicUnitPage.of(story, story.id, listOf(unit(1, position = 7), unit(2, position = 7), unit(3, position = 40)), null),
        )
    }

    @Test
    fun work_pages_refuse_duplicates_and_snapshot_their_items() {
        assertRefused(CatalogViolation.DuplicateWork, PublicWorkPage.of(listOf(work(1), work(1, title = "Khác")), null))
        assertRefused(CatalogViolation.PageTooLarge, PublicWorkPage.of((1..51).map { work(it) }, null))
        val source = mutableListOf(work(1))
        val page = PublicWorkPage.of(source, null).valid()
        source += work(2)
        assertEquals(listOf(work(1)), page.items)
    }

    private fun cover(work: PublicWorkId = workId(1), sha: String = "a".repeat(64), url: String? = null) = PublicCover.of(
        work, url ?: "https://api.cantos.example/api/v1/theatre/works/${work.value}/cover/$sha", sha,
        CoverContentType.Png, 1024, 1024, 4096, "Bìa Ánh đèn cuối sân khấu",
    )

    @Test
    fun a_cover_binds_one_work_one_digest_and_the_exact_cover_route() {
        val shown = cover().valid()
        assertEquals(workId(1), shown.work)
        assertRefused(CatalogViolation.CoverUrlMismatch, cover(url = "https://api.cantos.example/cover/${"a".repeat(64)}"))
        assertRefused(CatalogViolation.CoverUrlMismatch, cover(sha = "b".repeat(64), url = cover().valid().url))
        assertRefused(CatalogViolation.InvalidCover, cover(sha = "A".repeat(64)))
        // A cover is part of the Work's value: another Work's cover can never be attached to it.
        assertRefused(CatalogViolation.CoverWorkMismatch,
            PublicWork.of(workId(2), PublicWorkKind.Story, "Tên", "", "vi-VN", 5_000, shown))
        val withCover = PublicWork.of(workId(1), PublicWorkKind.Story, "Tên", "", "vi-VN", 5_000, shown).valid()
        assertEquals(shown, withCover.cover)
        // A changed digest is a different Work value, so a refreshed list replaces the cached one.
        val other = PublicWork.of(workId(1), PublicWorkKind.Story, "Tên", "", "vi-VN", 5_000, cover(sha = "c".repeat(64)).valid()).valid()
        assertEquals(false, withCover == other)
        assertEquals(false, withCover == work(1, title = "Tên", description = ""))
        assertEquals(null, work(1).cover)
    }
}
