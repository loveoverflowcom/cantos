package com.cantos.theatre.demo

enum class DemoWorkId { SonHa, DenKhua, CanhBuom }

enum class DemoEpisodeId { SonHa01, SonHa02, SonHa03, DenKhua01, DenKhua02, CanhBuom01 }

data class DemoEpisode(
    val id: DemoEpisodeId,
    val workId: DemoWorkId,
    val index: Int,
    val title: String,
    val durationSeconds: Int,
)

data class DemoWork(
    val id: DemoWorkId,
    val title: String,
    val category: String,
    val description: String,
    val episodes: List<DemoEpisode>,
)

/** Synthetic presentation fixtures, with reference titles only; no manuscript or media. */
object DemoFixtures {
    val initialEpisodeId = DemoEpisodeId.SonHa03
    const val initialPositionSeconds = 768
    val speedPercents = listOf(75, 100, 125, 150, 175, 200)

    val works: List<DemoWork> = listOf(
        DemoWork(
            id = DemoWorkId.SonHa,
            title = "Sơn Hà Ký",
            category = "Tiên hiệp · Phiêu lưu",
            description = "Theo tiếng chuông qua miền sơn thủy, một người trẻ tìm lại lời hẹn cũ.",
            episodes = listOf(
                DemoEpisode(
                    id = DemoEpisodeId.SonHa01,
                    workId = DemoWorkId.SonHa,
                    index = 1,
                    title = "Bến sông đầu mùa",
                    durationSeconds = 1560,
                ),
                DemoEpisode(
                    id = DemoEpisodeId.SonHa02,
                    workId = DemoWorkId.SonHa,
                    index = 2,
                    title = "Con đường qua núi",
                    durationSeconds = 1640,
                ),
                DemoEpisode(
                    id = DemoEpisodeId.SonHa03,
                    workId = DemoWorkId.SonHa,
                    index = 3,
                    title = "Người giữ tiếng chuông",
                    durationSeconds = 1722,
                ),
            ),
        ),
        DemoWork(
            id = DemoWorkId.DenKhua,
            title = "Đèn Khuya",
            category = "Đời thường · Tâm tình",
            description = "Một ô cửa còn sáng và những cuộc trò chuyện dịu dàng giữa đêm yên.",
            episodes = listOf(
                DemoEpisode(
                    id = DemoEpisodeId.DenKhua01,
                    workId = DemoWorkId.DenKhua,
                    index = 1,
                    title = "Ánh đèn bên cửa sổ",
                    durationSeconds = 1260,
                ),
                DemoEpisode(
                    id = DemoEpisodeId.DenKhua02,
                    workId = DemoWorkId.DenKhua,
                    index = 2,
                    title = "Lời hẹn chưa gửi",
                    durationSeconds = 1380,
                ),
            ),
        ),
        DemoWork(
            id = DemoWorkId.CanhBuom,
            title = "Cánh Bướm",
            category = "Kỳ ảo · Chữa lành",
            description = "Một cánh bướm dẫn lối qua khu vườn ký ức, nơi mỗi bước chân mở một lời chào.",
            episodes = listOf(
                DemoEpisode(
                    id = DemoEpisodeId.CanhBuom01,
                    workId = DemoWorkId.CanhBuom,
                    index = 1,
                    title = "Một mùa gió mới",
                    durationSeconds = 1140,
                ),
            ),
        ),
    )

    fun work(id: DemoWorkId): DemoWork = works.first { it.id == id }

    fun episode(id: DemoEpisodeId): DemoEpisode = works
        .flatMap { it.episodes }
        .first { it.id == id }
}
