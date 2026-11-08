//! Studio Web's native file-selection recipe, not an uploader or a shared UI export.
//! The caller owns validation, selected-file facts, reset and all subsequent effects.
use leptos::{ev::Event, prelude::*};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FilePickerTone {
    #[default]
    Primary,
    Tonal,
}

impl FilePickerTone {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Tonal => "tonal",
        }
    }
}

/// One stable input owns keyboard focus and the browser picker. Its associated label is the
/// visible target; there is no second button, synthetic click, remount or implicit upload.
#[component]
pub fn NativeFilePicker(
    #[prop(into)] id: String,
    #[prop(into)] accept: String,
    #[prop(into)] label: Signal<String>,
    #[prop(into)] help: Signal<String>,
    #[prop(into)] error: Signal<Option<String>>,
    #[prop(into)] status: Signal<String>,
    #[prop(into)] disabled: Signal<bool>,
    #[prop(into)] tone: Signal<FilePickerTone>,
    on_change: Callback<Event>,
) -> impl IntoView {
    let help_id = format!("{id}-help");
    let error_id = format!("{id}-error");
    let status_id = format!("{id}-status");
    let descriptions = format!("{help_id} {error_id} {status_id}");
    let input_id = id.clone();
    view! {
        <div class="studio-file-picker">
            <input id=input_id class="studio-file-picker-input" type="file" accept=accept
                disabled=move ||disabled.get() aria-describedby=descriptions
                aria-invalid=move ||if error.with(Option::is_some) {"true"} else {"false"}
                on:change=move |event| {
                    if !disabled.get_untracked() { on_change.run(event); }
                }/>
            <label class="studio-file-picker-label" for=id data-tone=move ||tone.get().as_str()>
                {move ||label.get()}
            </label>
            <p id=help_id class="cui-field-help">{move ||help.get()}</p>
            <p id=error_id class="cui-field-error" role="alert" hidden=move ||error.with(Option::is_none)>
                {move ||error.get()}
            </p>
            <p id=status_id class="studio-file-picker-status" role="status" aria-atomic="true">
                {move ||status.get()}
            </p>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instances_have_distinct_native_labels_and_descriptions_for_every_visual_state() {
        let owner = Owner::new();
        owner.with(|| {
            for disabled in [false, true] {
                for tone in [FilePickerTone::Primary, FilePickerTone::Tonal] {
                    for error in [None, Some("Tệp không hợp lệ".to_owned())] {
                        let html = ["first", "second"].into_iter().map(|id| view! {
                            <NativeFilePicker id=id accept="image/webp" label="Chọn tệp ảnh".to_owned()
                                help="WebP · 8 MiB".to_owned() status="Lời hẹn {time}.webp".to_owned()
                                error=Signal::stored(error.clone()) disabled=disabled tone=tone
                                on_change=Callback::new(|_|panic!("render must not select or upload"))/>
                        }.to_html()).collect::<String>();
                        assert_eq!(html.matches("type=\"file\"").count(), 2, "{html}");
                        assert!(!html.contains("<button"), "{html}");
                        for id in ["first", "second"] {
                            assert!(html.contains(&format!("for=\"{id}\"")), "{html}");
                            assert!(html.contains(&format!("aria-describedby=\"{id}-help {id}-error {id}-status\"")), "{html}");
                            for suffix in ["", "-help", "-error", "-status"] {
                                assert_eq!(html.matches(&format!("id=\"{id}{suffix}\"")).count(), 1, "{html}");
                            }
                        }
                        assert_eq!(html.matches(" disabled").count(), if disabled {2} else {0}, "{html}");
                        let invalid = if error.is_some() {"true"} else {"false"};
                        assert_eq!(html.matches(&format!("aria-invalid=\"{invalid}\"")).count(), 2, "{html}");
                        assert_eq!(html.matches("role=\"alert\" hidden").count(), if error.is_none() {2} else {0}, "{html}");
                        assert_eq!(html.matches(&format!("data-tone=\"{}\"", tone.as_str())).count(), 2, "{html}");
                        assert_eq!(html.matches("Lời hẹn {time}.webp").count(), 2, "{html}");
                    }
                }
            }
        });
    }
}
