// Generated proposal fixture; DO NOT EDIT. catalog sha256: 66acf31636e42d69184401c5a3d8bfefc82e5848dc34cc705bfa36e539fbea32
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale { L0, L1, L2 }

#[derive(Clone, Debug)]
pub enum Message {
    CommonCancel,
    CommonErrorUnavailable,
    StudioPublicationState { arg_state: StudioPublicationStateState },
    TheatreLibraryEpisodeCount { arg_count: u64 },
    TheatrePlayerBookmarkAdd { arg_time: String, arg_title: String },
}

#[derive(Clone, Copy, Debug)]
pub enum StudioPublicationStateState { Draft, Ready }

pub fn tag(locale: Locale) -> &'static str {
    match locale {
        Locale::L0 => "vi-VN",
        Locale::L1 => "en",
        Locale::L2 => "ja",
    }
}

pub fn render(locale: Locale, message: &Message) -> String {
    match message {
        Message::CommonCancel => match locale {
            Locale::L0 => ["Hủy".to_string()].concat(),
            Locale::L1 => ["Cancel".to_string()].concat(),
            Locale::L2 => ["キャンセル".to_string()].concat(),
        },
        Message::CommonErrorUnavailable => match locale {
            Locale::L0 => ["Dịch vụ đang tạm thời gián đoạn. Hãy thử lại.".to_string()].concat(),
            Locale::L1 => ["The service is temporarily unavailable. Try again.".to_string()].concat(),
            Locale::L2 => ["一時的に利用できません。もう一度お試しください。".to_string()].concat(),
        },
        Message::StudioPublicationState { arg_state } => match locale {
            Locale::L0 => match arg_state {
                StudioPublicationStateState::Draft => ["Bản nháp".to_string()].concat(),
                StudioPublicationStateState::Ready => ["Sẵn sàng".to_string()].concat(),
            },
            Locale::L1 => match arg_state {
                StudioPublicationStateState::Draft => ["Draft".to_string()].concat(),
                StudioPublicationStateState::Ready => ["Ready".to_string()].concat(),
            },
            Locale::L2 => match arg_state {
                StudioPublicationStateState::Draft => ["下書き".to_string()].concat(),
                StudioPublicationStateState::Ready => ["準備完了".to_string()].concat(),
            },
        },
        Message::TheatreLibraryEpisodeCount { arg_count } => match locale {
            Locale::L0 => [arg_count.to_string(), " tập".to_string()].concat(),
            Locale::L1 => if *arg_count == 1 { [arg_count.to_string(), " episode".to_string()].concat() } else { [arg_count.to_string(), " episodes".to_string()].concat() },
            Locale::L2 => [arg_count.to_string(), " 話".to_string()].concat(),
        },
        Message::TheatrePlayerBookmarkAdd { arg_time, arg_title } => match locale {
            Locale::L0 => ["Thêm dấu trang cho ".to_string(), arg_title.to_string(), " lúc ".to_string(), arg_time.to_string()].concat(),
            Locale::L1 => ["At ".to_string(), arg_time.to_string(), ", add bookmark for ".to_string(), arg_title.to_string()].concat(),
            Locale::L2 => [arg_title.to_string(), " の ".to_string(), arg_time.to_string(), " にブックマークを追加".to_string()].concat(),
        },
    }
}
