//! Read-only specimens bind only generated, canonical CSS-variable names.

use super::preview_catalog::TokenRegistration;
use leptos::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleKind {
    Color,
    FontFamily,
    FontSize,
    FontWeight,
    LineHeight,
    Spacing,
    Shape,
    Size,
    Stroke,
    Shadow,
    Opacity,
    ValueOnly,
}

impl SampleKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::FontFamily => "font-family",
            Self::FontSize => "font-size",
            Self::FontWeight => "font-weight",
            Self::LineHeight => "line-height",
            Self::Spacing => "spacing",
            Self::Shape => "shape",
            Self::Size => "size",
            Self::Stroke => "stroke",
            Self::Shadow => "shadow",
            Self::Opacity => "opacity",
            Self::ValueOnly => "value-only",
        }
    }

    pub const fn caption(self) -> &'static str {
        match self {
            Self::Color => "Mẫu màu theo theme hiện tại",
            Self::FontFamily => "Mẫu font; fallback vẫn phụ thuộc font có trên máy",
            Self::FontSize => "Mẫu cỡ chữ theo vai trò",
            Self::FontWeight => "Mẫu độ đậm theo vai trò",
            Self::LineHeight => "Mẫu nhịp hai dòng văn bản",
            Self::Spacing => "Khoảng cách thực giữa hai dấu mẫu",
            Self::Shape => "Bo góc theo vai trò trên vùng mẫu tĩnh",
            Self::Size => "Kích thước thực của vùng mẫu tĩnh",
            Self::Stroke => "Độ dày thực của nét mẫu",
            Self::Shadow => "Bóng theo vai trò trên vùng mẫu tĩnh",
            Self::Opacity => "Lớp màu có độ trong theo vai trò",
            Self::ValueOnly => "Tham số chỉ đọc; không mô phỏng breakpoint hay motion",
        }
    }
}

pub fn sample_kind(token: &TokenRegistration) -> SampleKind {
    match token.kind {
        "color" => SampleKind::Color,
        "fontFamily" => SampleKind::FontFamily,
        "fontWeight" => SampleKind::FontWeight,
        "shadow" => SampleKind::Shadow,
        "dimension" if token.name.starts_with("space.") => SampleKind::Spacing,
        "dimension" if token.name.starts_with("shape.") => SampleKind::Shape,
        "dimension" if token.name.starts_with("type.") && token.name.ends_with(".size") => {
            SampleKind::FontSize
        }
        "dimension" if ["size.control", "size.icon", "size.iconCompact"].contains(&token.name) => {
            SampleKind::Size
        }
        "dimension" if ["size.border", "size.focusWidth"].contains(&token.name) => {
            SampleKind::Stroke
        }
        "number" if token.name.starts_with("type.") && token.name.ends_with(".lineHeight") => {
            SampleKind::LineHeight
        }
        "number" if token.name.starts_with("state.") && token.name.ends_with("Opacity") => {
            SampleKind::Opacity
        }
        _ => SampleKind::ValueOnly,
    }
}

pub fn sample_value(token: &TokenRegistration) -> String {
    format!("var({})", token.css_variable)
}

#[component]
pub fn TokenSample(token: &'static TokenRegistration) -> impl IntoView {
    let kind = sample_kind(token);
    view! {
        <figure class="cui-token-sample" data-token-sample=token.name data-preview-kind=kind.as_str()
            style=("--cui-token-sample-value", sample_value(token))>
            <figcaption class="cui-gallery-note">{kind.caption()}</figcaption>
            {match kind {
                SampleKind::FontFamily | SampleKind::FontSize | SampleKind::FontWeight | SampleKind::LineHeight => view! {
                    <p class="cui-token-specimen">"Ánh đèn cuối sân khấu"<br/>"Nguyễn Thị Ánh Nguyệt vẫn nhớ lời hẹn ấy."</p>
                }.into_any(),
                SampleKind::Spacing => view! {
                    <div class="cui-token-specimen" aria-hidden="true"><span class="cui-token-mark"></span><span class="cui-token-mark"></span></div>
                }.into_any(),
                SampleKind::ValueOnly => view! {
                    <p class="cui-gallery-note">"Giá trị và đơn vị nằm trong bảng bên dưới; không tạo animation hay mô phỏng kích thước màn hình."</p>
                }.into_any(),
                _ => view! {
                    <div class="cui-token-specimen" aria-hidden="true"></div>
                }.into_any(),
            }}
        </figure>
    }
}
