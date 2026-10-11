use cantos_ui::Dialog;
use leptos::prelude::*;

#[test]
fn dialog_title_description_and_close_control_keep_stable_relations_and_escape_text() {
    let html = view! {
        <Dialog id="discard-r13" title="Rời bản diễn <Sơn Hà Ký>?" close_label="Giữ bản sửa"
            description="Bản sửa <r13> chưa được lưu."
            open=false on_close_request=Callback::new(|_| {}) on_error=Callback::new(|_| {})>
            <p>"Nội dung & lựa chọn được giữ nguyên."</p>
        </Dialog>
    }
    .to_html();
    assert!(html.contains("<dialog"), "{html}");
    assert!(
        html.contains("aria-labelledby=\"discard-r13-title\""),
        "{html}"
    );
    assert!(html.contains("id=\"discard-r13-title\""), "{html}");
    assert!(
        html.contains("aria-describedby=\"discard-r13-description\""),
        "{html}"
    );
    assert!(html.contains("id=\"discard-r13-description\""), "{html}");
    assert!(html.contains("id=\"discard-r13-close\""), "{html}");
    assert!(html.contains("Rời bản diễn &lt;Sơn Hà Ký&gt;?"), "{html}");
    assert!(
        html.contains("Bản sửa &lt;r13&gt; chưa được lưu."),
        "{html}"
    );
    assert!(html.contains("type=\"button\""), "{html}");
    assert!(html.contains("autofocus"), "{html}");
    assert!(html.contains("Giữ bản sửa"), "{html}");
    assert!(!html.contains("<Sơn Hà Ký>"), "{html}");
}

#[test]
fn requesting_open_does_not_render_a_non_modal_open_attribute_or_invent_a_description() {
    let html = view! {
        <Dialog id="sample-modal" title="Đọc bản diễn Một lời hẹn" close_label="Đóng bản diễn"
            open=true on_close_request=Callback::new(|_| {}) on_error=Callback::new(|_| {})
            class="consumer-reading-dialog">
            <p>"Vùng đọc giữ nội dung tiếng Việt dài."</p>
        </Dialog>
    }
    .to_html();
    let opening_tag = html.split('>').next().unwrap();
    assert!(!opening_tag.contains(" open"), "{html}");
    assert!(!html.contains("aria-describedby="), "{html}");
    assert!(!html.contains("cui-dialog-description"), "{html}");
    assert!(html.contains("consumer-reading-dialog"), "{html}");
    assert!(
        html.contains("Vùng đọc giữ nội dung tiếng Việt dài."),
        "{html}"
    );
}

#[test]
fn confirmation_actions_have_a_separate_footer_from_scrolling_content() {
    let actions = ViewFn::from(|| {
        view! {
            <button type="button">"Lưu bản diễn & quay lại"</button>
        }
    });
    let html = view! {
        <Dialog id="save-modal" title="Lưu bản diễn" close_label="Tiếp tục sửa"
            open=false actions=actions
            on_close_request=Callback::new(|_| {}) on_error=Callback::new(|_| {})>
            <p>"Nội dung bản diễn."</p>
        </Dialog>
    }
    .to_html();
    assert!(html.contains("<div class=\"cui-dialog-body\">"), "{html}");
    assert!(
        html.contains("<footer class=\"cui-dialog-actions\">"),
        "{html}"
    );
    assert!(html.contains("Lưu bản diễn &amp; quay lại"), "{html}");
}
