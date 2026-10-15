package com.cantos.theatre.adapters.mock

import com.cantos.theatre.demo.DemoWorkIds
import com.cantos.theatre.demo.DemoEpisodeIds
import com.cantos.theatre.presentation.model.ShellEpisode
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellWork
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.presentation.model.SupportedUiSpeedPercents

/** Synthetic presentation fixtures, with reference titles only; no manuscript or media. */
object DemoFixtures {
    val initialEpisodeId = DemoEpisodeIds.SonHa03
    const val initialPositionSeconds = 768
    val speedPercents: List<Int> = SupportedUiSpeedPercents

    val works: List<ShellWork> = listOf(
        ShellWork(
            id = DemoWorkIds.SonHa,
            title = "Sơn Hà Ký",
            category = "Tiên hiệp · Phiêu lưu",
            description = "Theo tiếng chuông qua miền sơn thủy, một người trẻ tìm lại lời hẹn cũ.",
            episodes = listOf(
                ShellEpisode(
                    id = DemoEpisodeIds.SonHa01,
                    workId = DemoWorkIds.SonHa,
                    index = 1,
                    title = "Bến sông đầu mùa",
                    durationSeconds = 1560,
                ),
                ShellEpisode(
                    id = DemoEpisodeIds.SonHa02,
                    workId = DemoWorkIds.SonHa,
                    index = 2,
                    title = "Con đường qua núi",
                    durationSeconds = 1640,
                ),
                ShellEpisode(
                    id = DemoEpisodeIds.SonHa03,
                    workId = DemoWorkIds.SonHa,
                    index = 3,
                    title = "Người giữ tiếng chuông",
                    durationSeconds = 1722,
                ),
            ),
        ),
        ShellWork(
            id = DemoWorkIds.DenKhua,
            title = "Đèn Khuya",
            category = "Đời thường · Tâm tình",
            description = "Một ô cửa còn sáng và những cuộc trò chuyện dịu dàng giữa đêm yên.",
            episodes = listOf(
                ShellEpisode(
                    id = DemoEpisodeIds.DenKhua01,
                    workId = DemoWorkIds.DenKhua,
                    index = 1,
                    title = "Ánh đèn bên cửa sổ",
                    durationSeconds = 1260,
                ),
                ShellEpisode(
                    id = DemoEpisodeIds.DenKhua02,
                    workId = DemoWorkIds.DenKhua,
                    index = 2,
                    title = "Lời hẹn chưa gửi",
                    durationSeconds = 1380,
                ),
            ),
        ),
        ShellWork(
            id = DemoWorkIds.CanhBuom,
            title = "Cánh Bướm",
            category = "Kỳ ảo · Chữa lành",
            description = "Một cánh bướm dẫn lối qua khu vườn ký ức, nơi mỗi bước chân mở một lời chào.",
            episodes = listOf(
                ShellEpisode(
                    id = DemoEpisodeIds.CanhBuom01,
                    workId = DemoWorkIds.CanhBuom,
                    index = 1,
                    title = "Một mùa gió mới",
                    durationSeconds = 1140,
                ),
            ),
        ),
    )

    fun work(id: ShellWorkId): ShellWork = works.first { it.id == id }

    fun episode(id: ShellEpisodeId): ShellEpisode = works
        .flatMap { it.episodes }
        .first { it.id == id }
}
