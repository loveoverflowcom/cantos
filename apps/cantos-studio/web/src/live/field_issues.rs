//! Workspace validation feedback; server diagnostics remain opaque text.
use super::copy;
use crate::contract::FieldIssue;
use leptos::prelude::*;

#[component]
pub(super) fn FieldIssues(
    issues: Signal<Vec<FieldIssue>>,
    locale: Signal<crate::localization::Locale>,
) -> impl IntoView {
    view! {
        <ul>{move || issues.get().into_iter().map(|issue| {
            let key = copy::field_rule_key(&issue.rule);
            view! {
                <li><p>{move || copy::text(key, locale.get())}</p>
                    <code>{copy::diagnostic(&issue.path, &issue.rule)}</code></li>
            }
        }).collect_view()}</ul>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_field_has_localized_guidance_before_its_exact_diagnostic() {
        Owner::new().with(|| {
            let issues = Signal::stored(vec![FieldIssue {
                path: "details.title".into(),
                rule: "required".into(),
            }]);
            for (choice, guidance) in [
                (
                    crate::localization::Locale::Vietnamese,
                    "Điền trường bắt buộc này.",
                ),
                (
                    crate::localization::Locale::English,
                    "Fill in this required field.",
                ),
            ] {
                let html = view! { <FieldIssues issues locale=Signal::stored(choice)/> }.to_html();
                assert!(html.contains(guidance), "{html}");
                assert!(
                    html.contains("<code>details.title · required</code>"),
                    "{html}"
                );
                assert!(html.find(guidance).unwrap() < html.find("<code>").unwrap());
            }
        });
    }

    #[test]
    fn unknown_rule_has_safe_localized_guidance_and_escaped_unchanged_diagnostic() {
        Owner::new().with(|| {
            let issues = Signal::stored(vec![FieldIssue {
                path: "details.<title>{time}".into(),
                rule: "future_rule_<script>&{title}".into(),
            }]);
            for (choice, guidance) in [
                (crate::localization::Locale::Vietnamese, "Kiểm tra trường này và mã chẩn đoán."),
                (crate::localization::Locale::English, "Review this field and its diagnostic code."),
            ] {
                let html = view! { <FieldIssues issues locale=Signal::stored(choice)/> }.to_html();
                assert!(html.contains(guidance), "{html}");
                assert!(html.contains("<code>details.&lt;title&gt;{time} · future_rule_&lt;script&gt;&amp;{title}</code>"), "{html}");
                assert!(!html.contains("<script>"), "{html}");
                assert!(!html.contains("<title>"), "{html}");
            }
        });
    }

    #[test]
    fn locale_projection_changes_guidance_without_rewriting_response_issues() {
        Owner::new().with(|| {
            let issues = RwSignal::new(vec![FieldIssue {
                path: "details.language".into(),
                rule: "language_tag".into(),
            }]);
            let original = issues.get_untracked();
            let locale = RwSignal::new(crate::localization::Locale::Vietnamese);
            for (choice, guidance) in [
                (
                    crate::localization::Locale::Vietnamese,
                    "Nhập mã ngôn ngữ nội dung hợp lệ, ví dụ vi-VN hoặc zh-Hant.",
                ),
                (
                    crate::localization::Locale::English,
                    "Enter a valid content language tag, such as vi-VN or zh-Hant.",
                ),
                (
                    crate::localization::Locale::Vietnamese,
                    "Nhập mã ngôn ngữ nội dung hợp lệ, ví dụ vi-VN hoặc zh-Hant.",
                ),
            ] {
                locale.set(choice);
                let html =
                    view! { <FieldIssues issues=issues.into() locale=locale.into()/> }.to_html();
                assert!(html.contains(guidance), "{html}");
                assert!(
                    html.contains("<code>details.language · language_tag</code>"),
                    "{html}"
                );
                assert_eq!(issues.get_untracked(), original);
            }
            issues.set(vec![]);
            let html = view! { <FieldIssues issues=issues.into() locale=locale.into()/> }.to_html();
            assert!(!html.contains("<li>"), "{html}");
        });
    }
}
