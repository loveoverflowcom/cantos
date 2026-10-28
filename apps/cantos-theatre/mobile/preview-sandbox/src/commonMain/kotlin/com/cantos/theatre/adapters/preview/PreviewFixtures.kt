package com.cantos.theatre.adapters.preview

import com.cantos.theatre.domain.catalog.CatalogCheck
import com.cantos.theatre.domain.catalog.PublicUnit
import com.cantos.theatre.domain.catalog.PublicUnitId
import com.cantos.theatre.domain.catalog.PublicUnitKind
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.catalog.PublicWorkKind
import com.cantos.theatre.domain.catalog.PublicationId
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellWorkId

/*
 * Synthetic Theatre Discovery values for the Desktop Preview. They use the same canonical server
 * identities (lower-case UUIDs) as the live API, so the shared controllers never see a fixture-only
 * identity. Titles are reference text only; there is no manuscript, media or provider content.
 */

/** Presentation identities of the three sample works. */
object PreviewWorkIds {
    val SonHa = ShellWorkId(PreviewFixtures.SonHa.work.id.value)
    val DenKhua = ShellWorkId(PreviewFixtures.DenKhua.work.id.value)
    val CanhBuom = ShellWorkId(PreviewFixtures.CanhBuom.work.id.value)
    val entries: List<ShellWorkId> = listOf(SonHa, DenKhua, CanhBuom)
}

/** Presentation identities of the sample units, in catalog order. */
object PreviewEpisodeIds {
    val SonHa01 = PreviewFixtures.SonHa.episode(0)
    val SonHa02 = PreviewFixtures.SonHa.episode(1)
    val SonHa03 = PreviewFixtures.SonHa.episode(2)
    val DenKhua01 = PreviewFixtures.DenKhua.episode(0)
    val DenKhua02 = PreviewFixtures.DenKhua.episode(1)
    val CanhBuom01 = PreviewFixtures.CanhBuom.episode(0)
    val entries: List<ShellEpisodeId> = listOf(SonHa01, SonHa02, SonHa03, DenKhua01, DenKhua02, CanhBuom01)
}

/** One sample work with the units the preview catalog lists for it, in catalog order. */
class PreviewWork internal constructor(val work: PublicWork, val units: List<PublicUnit>) {
    fun episode(index: Int): ShellEpisodeId = ShellEpisodeId(units[index].id.value, ShellWorkId(work.id.value))
}

/** A released sample unit together with the only audio identity it can be played as. */
class PreviewPlayable internal constructor(val work: PreviewWork, val unit: PublicUnit, val audio: AudioIdentity)

object PreviewFixtures {
    /** Fixed publication instant for every sample value (2026-10-01T00:00:00Z). */
    private const val PublishedAtMs = 1_790_812_800_000L

    val SonHa: PreviewWork = work(
        number = 1, kind = PublicWorkKind.Story, title = "Sơn Hà Ký",
        description = "Theo tiếng chuông qua miền sơn thủy, một người trẻ tìm lại lời hẹn cũ.",
        units = listOf(
            "Bến sông đầu mùa" to 1_560, "Con đường qua núi" to 1_640, "Người giữ tiếng chuông" to 1_722,
        ),
    )
    val DenKhua: PreviewWork = work(
        number = 2, kind = PublicWorkKind.Podcast, title = "Đèn Khuya",
        description = "Một ô cửa còn sáng và những cuộc trò chuyện dịu dàng giữa đêm yên.",
        units = listOf("Ánh đèn bên cửa sổ" to 1_260, "Lời hẹn chưa gửi" to 1_380),
    )
    val CanhBuom: PreviewWork = work(
        number = 3, kind = PublicWorkKind.Story, title = "Cánh Bướm",
        description = "Một cánh bướm dẫn lối qua khu vườn ký ức, nơi mỗi bước chân mở một lời chào.",
        units = listOf("Một mùa gió mới" to 1_140),
    )

    /**
     * Disclosed metadata (#574): a Work whose first unit has no eligible release yet, followed by a
     * released one, and a Work with no released unit at all. Only an explicit metadata opt-in of the
     * catalog lists them ([PreviewCatalogPort.disclosesMetadata]); the default walk stays released-only.
     */
    val GioMua: PreviewWork = work(
        number = 4, kind = PublicWorkKind.Story, title = "Gió Mùa",
        description = "Những lá thư gửi theo gió mùa, có lá đã đến và có lá còn đang trên đường.",
        units = listOf("Lá thư chưa mở" to null, "Cơn gió đầu mùa" to 1_320, "Bến đợi cuối năm" to null),
    )
    val SongDai: PreviewWork = work(
        number = 5, kind = PublicWorkKind.Podcast, title = "Sông Dài",
        description = "Một chương trình về những dòng sông, đang được chuẩn bị cho mùa phát sóng đầu tiên.",
        units = listOf("Thượng nguồn" to null),
    )

    /** Display order of the default catalog walk: released units only. */
    val works: List<PreviewWork> = listOf(SonHa, DenKhua, CanhBuom)

    /** Works the catalog lists only under an explicit metadata opt-in. */
    val metadataWorks: List<PreviewWork> = listOf(GioMua, SongDai)

    /** Every sample Work an owner may name, whichever catalog walk listed it. */
    private val known: List<PreviewWork> = works + metadataWorks

    fun work(id: String): PreviewWork? = known.firstOrNull { it.work.id.value == id }
    fun unit(id: String): Pair<PreviewWork, PublicUnit>? =
        known.firstNotNullOfOrNull { entry -> entry.units.firstOrNull { it.id.value == id }?.let { entry to it } }
    fun unitByPublication(id: String): Pair<PreviewWork, PublicUnit>? =
        known.firstNotNullOfOrNull { entry -> entry.units.firstOrNull { it.publicationId?.value == id }?.let { entry to it } }

    /** The released unit named [unitId] with its audio; a metadata-only or unknown unit has none. */
    fun playable(unitId: String): PreviewPlayable? =
        unit(unitId)?.let { (work, unit) -> audio(work, unit)?.let { PreviewPlayable(work, unit, it) } }

    /**
     * The immutable audio identity the playback manifest returns for [unit]; digest and rendition are
     * synthetic. A unit without an eligible release has no playback identity, so this is absent, never
     * a placeholder publication or duration.
     */
    fun audio(owner: PreviewWork, unit: PublicUnit): AudioIdentity? {
        val publication = unit.publicationId ?: return null
        val duration = unit.durationMs ?: return null
        // Only the released sample values above reach this point; they are built to satisfy the contract.
        return requireNotNull(AudioIdentity.of(
            owner.work.id.value, unit.id.value, publication.value,
            "6" + publication.value.drop(1), digest(unit.id.value), duration,
        )) { "Preview audio fixture must satisfy the production identity contract" }
    }

    private fun digest(seed: String): String {
        val hex = seed.filter { it != '-' }
        return (hex + hex).take(64)
    }

    /** [units] pairs each title with its released duration in seconds, or null for metadata without a release. */
    private fun work(number: Int, kind: PublicWorkKind, title: String, description: String, units: List<Pair<String, Int?>>): PreviewWork {
        val workId = valid(PublicWorkId.parse(uuid(1, number)))
        val published = valid(
            PublicWork.of(workId, kind, title, description, "vi", PublishedAtMs + number),
        )
        val unitKind = if (kind == PublicWorkKind.Podcast) PublicUnitKind.Episode else PublicUnitKind.Chapter
        val listed = units.mapIndexed { index, (unitTitle, seconds) ->
            val key = number * 10 + index
            val id = valid(PublicUnitId.parse(uuid(2, key)))
            valid(
                if (seconds == null) {
                    PublicUnit.withoutAudio(id, unitKind, index.toLong(), unitTitle, "$title · $unitTitle")
                } else {
                    PublicUnit.of(
                        id = id, kind = unitKind, position = index.toLong(),
                        title = unitTitle, description = "$title · $unitTitle",
                        publicationId = valid(PublicationId.parse(uuid(3, key))),
                        publishedAtMs = PublishedAtMs + key, durationMs = seconds * 1_000L,
                    )
                },
            )
        }
        return PreviewWork(published, listed)
    }

    private fun uuid(group: Int, number: Int): String =
        "${group}0000000-0000-4000-8000-${number.toString().padStart(12, '0')}"

    private fun <T> valid(check: CatalogCheck<T>): T = when (check) {
        is CatalogCheck.Valid -> check.value
        is CatalogCheck.Invalid -> error("Preview fixture violates the public catalog contract: ${check.violation}")
    }
}
