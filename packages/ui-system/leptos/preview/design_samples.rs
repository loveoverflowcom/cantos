//! Preview-owned v3 workbench. Proposal metadata never creates a shared component API.
use super::{catalog, href, preview_catalog};
use cantos_ui::{
    Button, ButtonVariant, Grid, GridColumns, Link, Select, Space, Stack, TextField, TextFieldKind,
};
use leptos::prelude::*;

#[component]
pub fn DesignInventory(
    token_query: RwSignal<String>,
    on_sample: Callback<&'static str>,
) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let status = RwSignal::new("all".to_owned());
    let scope = RwSignal::new("all".to_owned());
    view! {
        <section id="gallery-design" data-preview-scenario="v3-inventory" aria-labelledby="gallery-design-heading">
            <Stack>
                <header class="cui-gallery-header">
                    <p class="cui-gallery-eyebrow">"Design v3 · component-first"</p>
                    <h2 id="gallery-design-heading">"Tìm đúng mẫu trước khi xây màn"</h2>
                    <p>"Tìm → dùng lại hoặc mở rộng → khai báo và thử mẫu còn thiếu → kiểm tra → admission → dùng trong sản phẩm."</p>
                    <p class="cui-gallery-note">"Thiết kế đề xuất có owner, consumer, anatomy và trạng thái dự kiến. Registration và qualification runtime được ghi riêng; một mẫu thiết kế không tự tạo public API."</p>
                </header>
                <Grid columns=GridColumns::Three>
                    <TextField id="gallery-design-query" label="Tìm trong design v3" kind=TextFieldKind::Search value=query
                        placeholder="Ví dụ: mini-player, Studio, focus, color.primary"
                        on_input=Callback::new(move |value| query.set(value))/>
                    <Select id="gallery-design-status" label="Trạng thái thiết kế" value=status on_input=Callback::new(move |value| status.set(value))>
                        <option value="all">"Tất cả thiết kế"</option>
                        <option value="existing">"API kế thừa"</option>
                        <option value="proposed">"Đề xuất — chưa admission"</option>
                    </Select>
                    <Select id="gallery-design-scope" label="Phạm vi owner" value=scope on_input=Callback::new(move |value| scope.set(value))>
                        <option value="all">"Tất cả phạm vi"</option>
                        <option value="shared-primitive">"Shared primitive"</option>
                        <option value="shared-composition">"Shared composition"</option>
                        <option value="app-owned">"App-owned"</option>
                        <option value="platform-specific">"Platform-specific"</option>
                    </Select>
                </Grid>
                <Button id="gallery-design-reset" variant=ButtonVariant::Text on_press=Callback::new(move |_| {
                    query.set(String::new()); status.set("all".to_owned()); scope.set("all".to_owned());
                })>"Xóa bộ lọc thiết kế"</Button>
                <p id="gallery-design-count" class="cui-gallery-note" role="status" aria-live="polite" aria-atomic="true">
                    {move || format!("{} / {} specimen khớp · {} public APIs · qualification cần evidence riêng",
                        preview_catalog::DESIGN_SPECIMENS.iter().filter(|entry| catalog::design_matches(entry, &status.get(), &scope.get(), &query.get())).count(),
                        preview_catalog::DESIGN_SPECIMENS.len(), preview_catalog::COMPONENTS.len())}
                </p>
                <p id="gallery-design-empty" hidden=move || preview_catalog::DESIGN_SPECIMENS.iter().any(|entry| catalog::design_matches(entry, &status.get(), &scope.get(), &query.get()))>
                    "Không có specimen khớp. Xóa bộ lọc để tìm lại."
                </p>
                <details class="cui-design-list" open=move || !query.get().is_empty() || status.get() != "all" || scope.get() != "all">
                <summary>"Mở inventory anatomy, states và tokens"</summary>
                <ul class="cui-design-entries">
                    {preview_catalog::DESIGN_SPECIMENS.iter().map(|entry| {
                        let registration = catalog::specimen_registration(entry);
                        view! {
                            <li class="cui-design-entry" data-design-specimen=entry.id data-design-status=entry.design_status
                                hidden=move || !catalog::design_matches(entry, &status.get(), &scope.get(), &query.get())>
                                <details>
                                    <summary><span class="cui-gallery-eyebrow">{entry.family}" · "{entry.design_status}</span><strong>{entry.name}</strong><code>{entry.id}</code></summary>
                                    <Stack gap=Space::Sm>
                                        <p>{entry.description}</p>
                                        <dl class="cui-catalog-meta">
                                            <div><dt>"Owner / phạm vi"</dt><dd>{entry.owner}" · "{entry.scope}</dd></div>
                                            <div><dt>"Consumer dự kiến"</dt><dd>{entry.consumer}</dd></div>
                                            <div><dt>"Anatomy"</dt><dd>{entry.anatomy.join(" → ")}</dd></div>
                                            <div><dt>"State matrix thiết kế"</dt><dd>{entry.states.join(" · ")}</dd></div>
                                            <div><dt>"API tái sử dụng"</dt><dd class="cui-design-token-links">{entry.component_ids.iter().filter_map(|id| {
                                                let api = preview_catalog::COMPONENTS.iter().find(|api| api.id == *id)?;
                                                let anchor = catalog::sample_anchor(api.family)?;
                                                Some(view! { <Link id=format!("gallery-design-{}-reuse-{}", entry.id, id) href=href(&format!("#{anchor}"))
                                                    on_press=Callback::new(move |_| on_sample.run(api.family))>{api.name}</Link> })
                                            }).collect_view()}</dd></div>
                                            <div><dt>"Lý do / admission"</dt><dd>{entry.reason}</dd></div>
                                            <div><dt>"Runtime Leptos / CMP"</dt><dd>{registration.map_or_else(
                                                || "planned · chưa có public API; chưa qualified".to_owned(),
                                                |api| format!("{} / {} · qualification cần evidence riêng", api.leptos.status, api.cmp.status))}</dd></div>
                                            {registration.map(|api| view! {
                                                <div><dt>"Baseline states được giữ"</dt><dd>{api.states.join(" · ")}</dd></div>
                                            })}
                                        </dl>
                                        <div class="cui-design-token-links">
                                            {entry.token_roles.iter().map(|role| view! {
                                                <Link id=format!("gallery-design-{}-token-{}", entry.id, role) href=href("#gallery-tokens")
                                                    on_press=Callback::new(move |_| token_query.set((*role).to_owned()))>{*role}</Link>
                                            }).collect_view()}
                                        </div>
                                        {registration.and_then(|api| catalog::sample_anchor(api.family).map(|anchor| (api.family, anchor))).map(|(family, anchor)| view! {
                                            <Link id=format!("gallery-design-{}-sample", entry.id) href=href(&format!("#{anchor}"))
                                                on_press=Callback::new(move |_| on_sample.run(family))>"Xem sample API hiện có bên dưới"</Link>
                                        })}
                                    </Stack>
                                </details>
                            </li>
                        }
                    }).collect_view()}
                </ul>
                </details>
                <details class="cui-design-provenance">
                    <summary>"Nguồn và giới hạn so sánh"</summary>
                    <p>"PNG trong handoff là offline static export. Đối chiếu browser cần reference từ HTML cùng viewport/state; ảnh không chứng minh interaction hoặc motion."</p>
                    <dl class="cui-catalog-meta">
                        <div><dt>"Design version"</dt><dd>{preview_catalog::DESIGN_VERSION}</dd></div>
                        <div><dt>"Manifest SHA-256"</dt><dd><code>{preview_catalog::DESIGN_SOURCE_SHA256}</code></dd></div>
                        <div><dt>"Baseline SHA-256"</dt><dd><code>{preview_catalog::DESIGN_BASELINE_SHA256}</code></dd></div>
                    </dl>
                    <a href=preview_catalog::DESIGN_SOURCE_URL>"Manifest design v3 trên Drive"</a>
                </details>
            </Stack>
        </section>
    }
}

/// Two future consumer compositions, assembled from declared APIs only.
/// These fixtures dispatch local presentation intents; no audio/auth/network port exists.
#[component]
pub fn PrototypeCompositions() -> impl IntoView {
    use super::reason;
    use cantos_ui::{
        Announcement, Badge, ButtonState, IconName, RangeField, Status, StatusTone, Surface,
        SurfaceTone,
    };
    let listening = RwSignal::new(false);
    let position = RwSignal::new("0".to_owned());
    let draft = RwSignal::new("Ánh đèn cuối sân khấu — lời hẹn chưa bao giờ nói ra".to_owned());
    let review = RwSignal::new("empty".to_owned());
    view! {
        <section id="gallery-compositions" data-preview-scenario="v3-compositions" aria-labelledby="gallery-compositions-heading">
            <Stack>
                <h2 id="gallery-compositions-heading">"Thử composition cho consumer kế tiếp"</h2>
                <p class="cui-gallery-note">"Fixture đề xuất · planned · chưa qualified. Tái sử dụng Button, RangeField, TextField, Select, Status, Badge, Surface, Grid và Stack; không xuất public API mới."</p>
                <Grid columns=GridColumns::Two>
                    <Surface tone=SurfaceTone::Reading>
                        <article data-prototype="episode-list mini-player" aria-labelledby="gallery-listening-prototype-heading">
                            <header><Badge>"Theatre Web · proposed"</Badge><h3 id="gallery-listening-prototype-heading">"Một lời hẹn dưới chân Thanh Vân Sơn"</h3></header>
                            <p>"Tập 12 · Nguyễn Thị Ánh Nguyệt và Người dẫn chuyện · Ỗ, Ẫ, ặ"</p>
                            <p>"Ngày mai, mình có diễn tiếp không? Gió lướt qua hiên cũ, nhưng lời hẹn vẫn còn nguyên."</p>
                            <RangeField id="gallery-prototype-position" label="Vị trí nghe minh họa" value=position min="0" max="100" step="1"
                                value_text=Signal::derive(move || Some(format!("{} trên 100 · không phát âm thanh", position.get())))
                                on_input=Callback::new(move |value| position.set(value))/>
                            <Button id="gallery-prototype-play" variant=ButtonVariant::Tonal
                                icon=IconName::MenuBook on_press=Callback::new(move |_| listening.update(|value| *value = !*value))>
                                {move || if listening.get() { "Tạm dừng minh họa tập 12" } else { "Phát minh họa tập 12" }}
                            </Button>
                            <Status tone=StatusTone::Info announcement=Announcement::Polite>
                                <p id="gallery-prototype-playback">{move || if listening.get() { "Intent phát đã nhận tại fixture; không có audio adapter." } else { "Đang dừng trong fixture; không có audio adapter." }}</p>
                            </Status>
                        </article>
                    </Surface>
                    <Surface tone=SurfaceTone::Container>
                        <article data-prototype="studio-content-composer studio-audio-transfer studio-named-review" aria-labelledby="gallery-studio-prototype-heading">
                            <header><Badge>"Studio Web · proposed"</Badge><h3 id="gallery-studio-prototype-heading">"Biên tập và kiểm tra bản thu riêng"</h3></header>
                            <TextField id="gallery-prototype-draft" label="Tiêu đề bản nháp trong fixture" value=draft
                                on_input=Callback::new(move |value| draft.set(value))/>
                            <Select id="gallery-prototype-review" label="Trạng thái review minh họa" value=review
                                on_input=Callback::new(move |value| review.set(value))>
                                <option value="empty">"Chưa nhận bản thu"</option>
                                <option value="loading">"Đang gửi — chưa biết kết quả"</option>
                                <option value="error">"Cần sửa và đối chiếu receipt"</option>
                                <option value="review">"Chờ reviewer nghe bản thu"</option>
                            </Select>
                            <Status tone=StatusTone::Info announcement=Announcement::Polite>
                                <p id="gallery-prototype-review-output">{move || match review.get().as_str() {
                                    "loading" => "Fixture: kết quả chưa rõ; đối chiếu receipt trước khi thử lại.",
                                    "error" => "Fixture: bản thu có lỗi; văn bản biên tập vẫn được giữ.",
                                    "review" => "Fixture: chưa có named listening; không thể phê duyệt.",
                                    _ => "Fixture: chưa có bản thu; không có dữ liệu hoặc quyền từ backend.",
                                }}</p>
                            </Status>
                            <Button id="gallery-prototype-publish" variant=ButtonVariant::Secondary
                                state=ButtonState::Disabled(reason("Fixture chưa có named review, quyền và bản release; không thể xuất bản."))
                                on_press=Callback::new(move |_| {})>"Xuất bản — mẫu bị khóa"</Button>
                            <p class="cui-gallery-note">"Các mô tả ở đây là fixture giao diện. Backend vẫn là owner của receipt, rights và publication eligibility."</p>
                        </article>
                    </Surface>
                </Grid>
                <p class="cui-gallery-note">"Scenario v3-compositions: VI dài · vị trí 0 · empty/loading/error/review · input giữ nguyên khi theme, density hoặc text scale đổi. Prototype không đủ admission để product import."</p>
            </Stack>
        </section>
    }
}
