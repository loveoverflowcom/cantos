package com.cantos.theatre.app

import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellText

/**
 * Wording that differs from the shared defaults only because the data is real: the API gives a
 * display order rather than episode numbers, no total for a partial list, and a Library that
 * records listening history instead of a session sample.
 */
val TheatreShellCopyOverrides: Map<CopyKey, ShellText> = mapOf(
    CopyKey.EpisodesCount to ShellText("{count} tập", "{count} episodes", "{count} episode"),
    CopyKey.LibrarySession to ShellText(
        "Các tác phẩm đã lưu, điểm nghe, dấu trang và lịch sử nghe của bạn.",
        "Your saved works, listening positions, bookmarks and listening history.",
    ),
    CopyKey.HistoryHelp to ShellText(
        "Các câu chuyện đã nghe, gần nhất ở trên.",
        "Stories you have listened to, most recent first.",
    ),
)
