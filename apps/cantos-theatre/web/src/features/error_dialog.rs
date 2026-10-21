//! Web composition of the shared Dialog/Button APIs; runtime qualification is pending. One shell-owned error window.
use crate::{
    application::{
        error_writer::update_errors,
        errors::{ErrorAction, ErrorCenter, ErrorNotice},
    },
    messages::{self, Key, Locale},
};
use cantos_ui::{Button, ButtonVariant, Dialog};
use leptos::prelude::*;

#[component]
pub(crate) fn ErrorWindow(
    center: RwSignal<ErrorCenter>,
    locale: RwSignal<Locale>,
    on_action: Callback<ErrorAction>,
) -> impl IntoView {
    let browser_failure = RwSignal::new(false);
    let active = Memo::new(move |_| center.with(ErrorCenter::active));
    view! {
        {move || active.get().map(|notice| view! { <NoticeDialog notice center locale on_action on_failure=Callback::new(move |_| { browser_failure.set(true); update_errors(center, ErrorCenter::dismiss); })/> })}
        <Show when=move || browser_failure.get()>
            <p role="alert">{move || messages::text(Key::ErrorProtocol, locale.get())}</p>
        </Show>
    }
}
#[component]
fn NoticeDialog(
    notice: ErrorNotice,
    center: RwSignal<ErrorCenter>,
    locale: RwSignal<Locale>,
    on_action: Callback<ErrorAction>,
    on_failure: Callback<()>,
) -> impl IntoView {
    let primary = notice.primary();
    #[cfg(target_arch = "wasm32")]
    Effect::new(move |_| {
        // A startup error has no pressed opener. Give the shell a safe return target before
        // the shared Dialog captures it; never steal focus from a real control.
        if crate::platform::prepare_error_focus().is_err() {
            on_failure.run(());
        }
    });
    view! {
        {move || {
            let language = locale.get();
            view! {
                <Dialog id="theatre-error" title=messages::text(Key::ErrorDialogTitle, language)
                    description=messages::text(messages::error_key(notice.key.kind), language)
                    close_label=messages::text(Key::CloseError, language)
                    open=Signal::derive(move || center.with(|s| s.active() == Some(notice)))
                    on_close_request=Callback::new(move |_| { update_errors(center, ErrorCenter::dismiss); })
                    on_error=Callback::new(move |_| on_failure.run(()))
                    actions=ViewFn::from(move || view! {
                        <Show when=move || primary != ErrorAction::Close>
                            <Button id="theatre-error-primary" variant=ButtonVariant::Primary focus_on_press=true
                                on_press=Callback::new(move |_| on_action.run(primary))>
                                {messages::text(if primary == ErrorAction::SignIn { Key::SignIn } else { Key::PublicRetry }, language)}
                            </Button>
                        </Show>
                    })>
                    <span></span>
                </Dialog>
            }
        }}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::errors::{ErrorKind, ErrorOperation};
    #[test]
    fn standard_window_uses_shared_anatomy_localized_copy_and_explicit_close() {
        Owner::new().with(|| {
            for (language, expected, close) in [
                (Locale::Vietnamese, "Cấu hình máy chủ", "Đóng"),
                (Locale::English, "server configuration", "Close"),
            ] {
                let center = RwSignal::new(ErrorCenter::default());
                let locale = RwSignal::new(language);
                center.update(|s| {
                    s.report(
                        ErrorNotice::read(ErrorKind::Configuration, ErrorOperation::LoadCatalog),
                        false,
                    )
                });
                let html = view! { <ErrorWindow center locale on_action=Callback::new(|_| {})/> }
                    .to_html();
                assert!(
                    html.contains("<dialog") && html.contains(expected) && html.contains(close)
                );
                assert!(!html.contains("theatre-error-primary"));
            }
        });
    }
}
