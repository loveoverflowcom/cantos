// Generated proposal fixture; DO NOT EDIT. catalog sha256: 66acf31636e42d69184401c5a3d8bfefc82e5848dc34cc705bfa36e539fbea32
package cantos.localization.fixture

enum class Locale { L0, L1, L2 }

sealed interface Message {
    data object CommonCancel : Message
    data object CommonErrorUnavailable : Message
    data class StudioPublicationState(val arg_state: StudioPublicationStateState) : Message
    data class TheatreLibraryEpisodeCount(val arg_count: ULong) : Message
    data class TheatrePlayerBookmarkAdd(val arg_time: String, val arg_title: String) : Message
}

enum class StudioPublicationStateState { Draft, Ready }

fun tag(locale: Locale): String = when (locale) {
    Locale.L0 -> "vi-VN"
    Locale.L1 -> "en"
    Locale.L2 -> "ja"
}

fun render(locale: Locale, message: Message): String = when (message) {
    Message.CommonCancel -> when (locale) {
        Locale.L0 -> "Hủy"
        Locale.L1 -> "Cancel"
        Locale.L2 -> "キャンセル"
    }
    Message.CommonErrorUnavailable -> when (locale) {
        Locale.L0 -> "Dịch vụ đang tạm thời gián đoạn. Hãy thử lại."
        Locale.L1 -> "The service is temporarily unavailable. Try again."
        Locale.L2 -> "一時的に利用できません。もう一度お試しください。"
    }
    is Message.StudioPublicationState -> when (locale) {
        Locale.L0 -> when (message.arg_state) {
                StudioPublicationStateState.Draft -> "Bản nháp"
                StudioPublicationStateState.Ready -> "Sẵn sàng"
            }
        Locale.L1 -> when (message.arg_state) {
                StudioPublicationStateState.Draft -> "Draft"
                StudioPublicationStateState.Ready -> "Ready"
            }
        Locale.L2 -> when (message.arg_state) {
                StudioPublicationStateState.Draft -> "下書き"
                StudioPublicationStateState.Ready -> "準備完了"
            }
    }
    is Message.TheatreLibraryEpisodeCount -> when (locale) {
        Locale.L0 -> message.arg_count.toString() + " tập"
        Locale.L1 -> if (message.arg_count == 1uL) message.arg_count.toString() + " episode" else message.arg_count.toString() + " episodes"
        Locale.L2 -> message.arg_count.toString() + " 話"
    }
    is Message.TheatrePlayerBookmarkAdd -> when (locale) {
        Locale.L0 -> "Thêm dấu trang cho " + message.arg_title.toString() + " lúc " + message.arg_time.toString()
        Locale.L1 -> "At " + message.arg_time.toString() + ", add bookmark for " + message.arg_title.toString()
        Locale.L2 -> message.arg_title.toString() + " の " + message.arg_time.toString() + " にブックマークを追加"
    }
}
