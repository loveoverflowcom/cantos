//! Preview-owned v3 workbench. Proposal metadata never creates a shared component API.
use super::messages::tool;
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
                    <h2 lang=tool::LANGUAGE_TAG id="gallery-design-heading">{tool::DESIGN_TITLE}</h2>
                    <p lang=tool::LANGUAGE_TAG>{tool::DESIGN_FLOW}</p>
                    <p lang=tool::LANGUAGE_TAG class="cui-gallery-note">{tool::DESIGN_HELP}</p>
                </header>
                <Grid columns=GridColumns::Three>
                    <TextField id="gallery-design-query" label=tool::DESIGN_SEARCH kind=TextFieldKind::Search value=query
                        placeholder=tool::DESIGN_PLACEHOLDER
                        on_input=Callback::new(move |value| query.set(value))/>
                    <Select id="gallery-design-status" label=tool::DESIGN_STATUS value=status on_input=Callback::new(move |value| status.set(value))>
                        <option value="all">{tool::DESIGN_ALL}</option>
                        <option value="existing">{tool::DESIGN_EXISTING}</option>
                        <option value="proposed">{tool::DESIGN_PROPOSED}</option>
                    </Select>
                    <Select id="gallery-design-scope" label=tool::DESIGN_SCOPE value=scope on_input=Callback::new(move |value| scope.set(value))>
                        <option value="all">{tool::SCOPE_ALL}</option>
                        <option value="shared-primitive">"Shared primitive"</option>
                        <option value="shared-composition">"Shared composition"</option>
                        <option value="app-owned">"App-owned"</option>
                        <option value="platform-specific">"Platform-specific"</option>
                    </Select>
                </Grid>
                <Button id="gallery-design-reset" variant=ButtonVariant::Text on_press=Callback::new(move |_| {
                    query.set(String::new()); status.set("all".to_owned()); scope.set("all".to_owned());
                })>{tool::DESIGN_RESET}</Button>
                <p id="gallery-design-count" class="cui-gallery-note" role="status" aria-live="polite" aria-atomic="true">
                    {move || tool::design_count(
                        preview_catalog::DESIGN_SPECIMENS.iter().filter(|entry| catalog::design_matches(entry, &status.get(), &scope.get(), &query.get())).count(),
                        preview_catalog::DESIGN_SPECIMENS.len(), preview_catalog::COMPONENTS.len())}
                </p>
                <p id="gallery-design-empty" hidden=move || preview_catalog::DESIGN_SPECIMENS.iter().any(|entry| catalog::design_matches(entry, &status.get(), &scope.get(), &query.get()))>
                    {tool::DESIGN_EMPTY}
                </p>
                <details class="cui-design-list" open=move || !query.get().is_empty() || status.get() != "all" || scope.get() != "all">
                <summary>{tool::DESIGN_DETAILS}</summary>
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
                                            <div><dt>{tool::DESIGN_OWNER}</dt><dd>{entry.owner}" · "{entry.scope}</dd></div>
                                            <div><dt>{tool::DESIGN_CONSUMER}</dt><dd>{entry.consumer}</dd></div>
                                            <div><dt>"Anatomy"</dt><dd>{entry.anatomy.join(" → ")}</dd></div>
                                            <div><dt>{tool::DESIGN_MATRIX}</dt><dd>{entry.states.join(" · ")}</dd></div>
                                            <div><dt>{tool::DESIGN_REUSE}</dt><dd class="cui-design-token-links">{entry.component_ids.iter().filter_map(|id| {
                                                let api = preview_catalog::COMPONENTS.iter().find(|api| api.id == *id)?;
                                                let anchor = catalog::sample_anchor(api.family)?;
                                                Some(view! { <Link id=format!("gallery-design-{}-reuse-{}", entry.id, id) href=href(&format!("#{anchor}"))
                                                    on_press=Callback::new(move |_| on_sample.run(api.family))>{api.name}</Link> })
                                            }).collect_view()}</dd></div>
                                            <div><dt>{tool::DESIGN_REASON}</dt><dd>{entry.reason}</dd></div>
                                            <div><dt>"Runtime Leptos / CMP"</dt><dd>{registration.map_or_else(
                                                || tool::DESIGN_PENDING.to_owned(),
                                                |api| tool::renderer_status( api.leptos.status, api.cmp.status))}</dd></div>
                                            {registration.map(|api| view! {
                                                <div><dt>{tool::DESIGN_BASELINE}</dt><dd>{api.states.join(" · ")}</dd></div>
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
                                                on_press=Callback::new(move |_| on_sample.run(family))>{tool::DESIGN_SAMPLE}</Link>
                                        })}
                                    </Stack>
                                </details>
                            </li>
                        }
                    }).collect_view()}
                </ul>
                </details>
                <details class="cui-design-provenance">
                    <summary>{tool::DESIGN_LIMITS}</summary>
                    <p lang=tool::LANGUAGE_TAG>{tool::DESIGN_SOURCE_HELP}</p>
                    <dl class="cui-catalog-meta">
                        <div><dt>"Design version"</dt><dd>{preview_catalog::DESIGN_VERSION}</dd></div>
                        <div><dt>"Manifest SHA-256"</dt><dd><code>{preview_catalog::DESIGN_SOURCE_SHA256}</code></dd></div>
                        <div><dt>"Baseline SHA-256"</dt><dd><code>{preview_catalog::DESIGN_BASELINE_SHA256}</code></dd></div>
                    </dl>
                    <a href=preview_catalog::DESIGN_SOURCE_URL>{tool::DESIGN_MANIFEST}</a>
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
        <section lang="vi-VN" data-preview-fixture="vietnamese" id="gallery-compositions" data-preview-scenario="v3-compositions" aria-labelledby="gallery-compositions-heading">
            <Stack>
                <h2 lang=tool::LANGUAGE_TAG id="gallery-compositions-heading">{tool::PROTOTYPE_TITLE}</h2>
                <p lang=tool::LANGUAGE_TAG class="cui-gallery-note">{tool::PROTOTYPE_HELP}</p>
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
                <p lang=tool::LANGUAGE_TAG class="cui-gallery-note">{tool::PROTOTYPE_LIMITS}</p>
            </Stack>
        </section>
    }
}
