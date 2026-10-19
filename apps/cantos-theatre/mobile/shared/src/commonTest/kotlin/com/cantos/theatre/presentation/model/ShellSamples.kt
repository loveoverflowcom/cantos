package com.cantos.theatre.presentation.model

/** Test-local sample identities and a state builder; the Desktop Preview has its own, separate seeds. */
object SampleWorkIds {
    val SonHa = ShellWorkId("SonHa")
    val DenKhua = ShellWorkId("DenKhua")
    val CanhBuom = ShellWorkId("CanhBuom")
}

object SampleEpisodeIds {
    val SonHa01 = ShellEpisodeId("SonHa01", SampleWorkIds.SonHa)
    val SonHa02 = ShellEpisodeId("SonHa02", SampleWorkIds.SonHa)
    val SonHa03 = ShellEpisodeId("SonHa03", SampleWorkIds.SonHa)
    val DenKhua01 = ShellEpisodeId("DenKhua01", SampleWorkIds.DenKhua)
    val DenKhua02 = ShellEpisodeId("DenKhua02", SampleWorkIds.DenKhua)
    val CanhBuom01 = ShellEpisodeId("CanhBuom01", SampleWorkIds.CanhBuom)
}

fun sampleState(
    stack: List<ShellRoute> = listOf(ShellRoute.Discovery),
    selectedEpisode: ShellEpisodeId = SampleEpisodeIds.SonHa03,
    positionSeconds: Int = 768,
    simulating: Boolean = false,
    savedWorks: Set<ShellWorkId> = setOf(SampleWorkIds.SonHa),
    bookmarks: List<ShellBookmark> = emptyList(),
    libraryTab: ShellLibraryTab = ShellLibraryTab.Continuing,
    query: String = "",
    catalogMode: ShellCatalogMode = ShellCatalogMode.Ready,
    theme: ShellTheme = ShellTheme.Light,
    locale: ShellLocale = ShellLocale.Vietnamese,
    speedPercent: Int = 100,
    timer: ShellTimer = ShellTimer.Off,
    playbackStatus: PlaybackUiStatus = if (simulating) PlaybackUiStatus.Simulating else PlaybackUiStatus.Paused,
    transportRequested: Boolean = simulating,
    positionMs: Long = positionSeconds.toLong() * 1000L,
    progress: List<ShellProgress> = listOf(ShellProgress(selectedEpisode, positionSeconds, positionMs)),
    history: List<ShellProgress> = emptyList(),
    listeningFailure: ShellListeningFailure? = null,
): ShellState = ShellState(
    stack = stack, selectedEpisode = selectedEpisode, positionSeconds = positionSeconds, simulating = simulating,
    savedWorks = savedWorks, bookmarks = bookmarks, libraryTab = libraryTab, query = query, catalogMode = catalogMode,
    theme = theme, locale = locale, speedPercent = speedPercent, timer = timer, playbackStatus = playbackStatus,
    transportRequested = transportRequested, positionMs = positionMs, progress = progress, history = history,
    listeningFailure = listeningFailure,
)

/** Three sample works with Vietnamese reference titles; presentation-only, never an API value. */
val sampleWorks: List<ShellWork> = listOf(
    ShellWork(SampleWorkIds.SonHa, "Sơn Hà Ký", "Tiên hiệp · Phiêu lưu",
        "Theo tiếng chuông qua miền sơn thủy, một người trẻ tìm lại lời hẹn cũ.", listOf(
            ShellEpisode(SampleEpisodeIds.SonHa01, SampleWorkIds.SonHa, 1, "Bến sông đầu mùa", 1560),
            ShellEpisode(SampleEpisodeIds.SonHa02, SampleWorkIds.SonHa, 2, "Con đường qua núi", 1640),
            ShellEpisode(SampleEpisodeIds.SonHa03, SampleWorkIds.SonHa, 3, "Người giữ tiếng chuông", 1722),
        )),
    ShellWork(SampleWorkIds.DenKhua, "Đèn Khuya", "Đời thường · Tâm tình",
        "Một ô cửa còn sáng và những cuộc trò chuyện dịu dàng giữa đêm yên.", listOf(
            ShellEpisode(SampleEpisodeIds.DenKhua01, SampleWorkIds.DenKhua, 1, "Ánh đèn bên cửa sổ", 1260),
            ShellEpisode(SampleEpisodeIds.DenKhua02, SampleWorkIds.DenKhua, 2, "Lời hẹn chưa gửi", 1380),
        )),
    ShellWork(SampleWorkIds.CanhBuom, "Cánh Bướm", "Kỳ ảo · Chữa lành",
        "Một cánh bướm dẫn lối qua khu vườn ký ức, nơi mỗi bước chân mở một lời chào.", listOf(
            ShellEpisode(SampleEpisodeIds.CanhBuom01, SampleWorkIds.CanhBuom, 1, "Một mùa gió mới", 1140),
        )),
)
