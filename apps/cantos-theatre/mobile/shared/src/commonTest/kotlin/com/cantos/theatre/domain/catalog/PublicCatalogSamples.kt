package com.cantos.theatre.domain.catalog

/** Test-only builders over the real factories; an invalid sample fails the test immediately. */
object PublicCatalogSamples {
    fun <T> CatalogCheck<T>.valid(): T = when (this) {
        is CatalogCheck.Valid -> value
        is CatalogCheck.Invalid -> error("Expected a valid sample, got $violation")
    }

    fun uuid(prefix: Int, n: Int): String =
        "${prefix.toString().padStart(8, '0')}-0000-4000-8000-${n.toString().padStart(12, '0')}"

    fun workId(n: Int): PublicWorkId = PublicWorkId.parse(uuid(10_000_000, n)).valid()
    fun unitId(n: Int): PublicUnitId = PublicUnitId.parse(uuid(20_000_000, n)).valid()
    fun publicationId(n: Int): PublicationId = PublicationId.parse(uuid(80_000_000, n)).valid()
    fun cursor(n: Int): CatalogCursor = CatalogCursor.parse("lc1_" + n.toString(16).padStart(64, '0')).valid()

    fun work(
        n: Int,
        kind: PublicWorkKind = PublicWorkKind.Story,
        title: String = "Ánh đèn cuối sân khấu $n",
        description: String = "Một lời hẹn chưa bao giờ nói ra.",
        cover: PublicCover? = null,
    ): PublicWork = PublicWork.of(workId(n), kind, title, description, "vi-VN", 5_000L + n, cover).valid()

    /** A cover the Server would authorize for work [n]: its own route and digest. */
    fun cover(n: Int, alt: String = "Bìa của tác phẩm $n: ngọn đèn vàng trên sân khấu tối."): PublicCover {
        val sha = n.toString(16).padStart(64, 'a')
        return PublicCover.of(
            workId(n), "https://api.cantos.example/api/v1/theatre/works/${workId(n).value}/cover/$sha", sha,
            CoverContentType.Png, 1024, 1536, 4_096, alt,
        ).valid()
    }

    fun unit(
        n: Int,
        kind: PublicUnitKind = PublicUnitKind.Chapter,
        publication: Int = n,
        position: Long = n.toLong(),
        title: String = "Chương $n: Một tiếng chuông trong sương",
        durationMs: Long = 61_000,
    ): PublicUnit = PublicUnit.of(
        unitId(n), kind, position, title, "Chương của bản thử hư cấu.", publicationId(publication), 5_000, durationMs,
    ).valid()

    fun worksPage(works: List<PublicWork>, next: CatalogCursor? = null): PublicWorkPage = PublicWorkPage.of(works, next).valid()

    fun unitsPage(work: PublicWork, units: List<PublicUnit>, next: CatalogCursor? = null): PublicUnitPage =
        PublicUnitPage.of(work, work.id, units, next).valid()
}
