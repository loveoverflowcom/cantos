package com.cantos.theatre.demo

import com.cantos.theatre.presentation.model.*
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.presentation.model.ShellEpisodeId

object DemoWorkIds {
    val SonHa = ShellWorkId("SonHa")
    val DenKhua = ShellWorkId("DenKhua")
    val CanhBuom = ShellWorkId("CanhBuom")
    val entries = listOf(SonHa, DenKhua, CanhBuom)
}
object DemoEpisodeIds {
    val SonHa01 = ShellEpisodeId("SonHa01", DemoWorkIds.SonHa)
    val SonHa02 = ShellEpisodeId("SonHa02", DemoWorkIds.SonHa)
    val SonHa03 = ShellEpisodeId("SonHa03", DemoWorkIds.SonHa)
    val DenKhua01 = ShellEpisodeId("DenKhua01", DemoWorkIds.DenKhua)
    val DenKhua02 = ShellEpisodeId("DenKhua02", DemoWorkIds.DenKhua)
    val CanhBuom01 = ShellEpisodeId("CanhBuom01", DemoWorkIds.CanhBuom)
    val entries = listOf(SonHa01, SonHa02, SonHa03, DenKhua01, DenKhua02, CanhBuom01)
}
