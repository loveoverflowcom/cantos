//! App-private semantic HTML compositions, not new shared UI primitives.
use super::common::*;
use crate::{
    app::{context::TheatreContext, routes::Route},
    messages::Key,
};
use leptos::prelude::*;

/// A fact the mock cannot yet provide. It is text, never a control: nothing here pretends to work.
#[component]
fn StatusRow(ctx: TheatreContext, label: Key, help: Key, value: Key) -> impl IntoView {
    view! {
        <div class="theatre-row">
            <dt class="theatre-row-label">{move || ctx.text(label)}</dt>
            <dd class="theatre-row-value">{move || ctx.text(value)}</dd>
            <dd class="theatre-row-help theatre-muted">{move || ctx.text(help)}</dd>
        </div>
    }
}

#[component]
fn LinkRow(
    ctx: TheatreContext,
    route: Route,
    asset: &'static str,
    label: Key,
    help: Key,
) -> impl IntoView {
    view! {
        <li>
            <a class="theatre-row theatre-row-link" href=route.hash()>
                {icon(asset)}
                <span class="theatre-row-copy">
                    <span class="theatre-row-label">{move || ctx.text(label)}</span>
                    <span class="theatre-row-help theatre-muted">{move || ctx.text(help)}</span>
                </span>
            </a>
        </li>
    }
}

/// Identity first, then what the account can and cannot do today. State is the mock's own:
/// there is no sign-in, sync or conflict model, so none is drawn.
#[component]
pub(crate) fn Account(ctx: TheatreContext) -> impl IntoView {
    view! {
        <PageHeading ctx=ctx title=Key::Account/>
        <div class="theatre-account">
            <div class="theatre-panel theatre-account-identity">
                <span class="theatre-avatar" data-size="large" aria-hidden="true"><PersonGlyph/></span>
                <div class="theatre-account-name">
                    <h2>{move || ctx.text(Key::GuestListener)}</h2>
                    <p class="theatre-muted">{move || ctx.text(Key::GuestStatus)}</p>
                </div>
            </div>
            <div class="theatre-account-content">
                <div class="theatre-panel">
                    <section aria-labelledby="theatre-account-status">
                        <h2 id="theatre-account-status" class="theatre-group-heading">{move || ctx.text(Key::AccountStatusHeading)}</h2>
                        <dl class="theatre-rows">
                            <StatusRow ctx=ctx label=Key::SignIn help=Key::SignInHelp value=Key::NotIntegrated/>
                            <StatusRow ctx=ctx label=Key::Sync help=Key::SyncHelp value=Key::NotIntegrated/>
                            <StatusRow ctx=ctx label=Key::StorageLabel help=Key::StorageHelp value=Key::StorageValue/>
                        </dl>
                    </section>
                </div>
                <div class="theatre-panel">
                    <section aria-labelledby="theatre-account-links">
                        <h2 id="theatre-account-links" class="theatre-group-heading">{move || ctx.text(Key::AccountShortcutsHeading)}</h2>
                        <ul class="theatre-rows">
                            <LinkRow ctx=ctx route=Route::Settings asset="settings" label=Key::Settings help=Key::SettingsLinkHelp/>
                            <LinkRow ctx=ctx route=Route::Library asset="library_books" label=Key::Library help=Key::LibraryLinkHelp/>
                        </ul>
                    </section>
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guest_account_is_identity_then_truthful_status_then_existing_links() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let html = view! { <Account ctx=ctx/> }.to_html();
            assert_eq!(html.matches("<h1").count(), 1);
            assert!(html.contains("theatre-avatar") && html.contains("Người nghe khách"));
            assert!(html.contains("Chưa đăng nhập"));
            for status in [
                "Đăng nhập",
                "Đồng bộ",
                "Chưa tích hợp",
                "Chỉ trong phiên này",
            ] {
                assert!(html.contains(status), "{status}");
            }
            for href in ["href=\"#settings\"", "href=\"#library\""] {
                assert!(html.contains(href), "{href}");
            }
            assert!(html.contains("Cài đặt nghe") && html.contains("Của tôi"));
            // No control pretends to work, and no progress-conflict choice exists in this domain.
            assert!(!html.contains("<button") && !html.contains("disabled"));
            for conflict in ["xung đột", "Giữ bản", "conflict"] {
                assert!(!html.contains(conflict), "{conflict}");
            }
        });
    }

    #[test]
    fn guest_account_localizes_without_reusing_vietnamese_in_english() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            ctx.locale.set(crate::messages::Locale::English);
            let html = view! { <Account ctx=ctx/> }.to_html();
            for text in [
                "Guest listener",
                "Not integrated",
                "This session only",
                "Listening settings",
            ] {
                assert!(html.contains(text), "{text}");
            }
            assert!(!html.contains("Người nghe khách"));
        });
    }
}
