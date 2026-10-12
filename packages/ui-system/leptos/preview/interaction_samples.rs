//! Native HTML counterparts for the bounded CMP interaction contracts. No service effects.
use cantos_ui::{
    Announcement, Button, ButtonVariant, ChoiceState, Container, ContainerWidth, DisabledReason,
    ErrorState, Grid, GridColumns, RangeField, Select, Space, Stack, StackDirection, Status,
    StatusTone, Surface, SurfaceTone,
};
use leptos::prelude::*;

#[component]
pub fn SelectionContractSamples(
    #[prop(default = "stable")] initial_mode: &'static str,
) -> impl IntoView {
    let mode = RwSignal::new(initial_mode.to_owned());
    let value = RwSignal::new("narrator".to_owned());
    let calls = RwSignal::new(0u32);
    let range = RwSignal::new("0".to_owned());
    let completions = RwSignal::new(0u32);
    let state = Signal::derive(move || match mode.get().as_str() {
        "empty" => ChoiceState::Disabled(
            DisabledReason::new("Chưa có nhân vật. Hãy thêm nhân vật để tiếp tục.")
                .expect("nonblank fixture"),
        ),
        "interrupted" => ChoiceState::Loading(
            DisabledReason::new("Đang nạp bản nháp; lựa chọn vẫn được giữ.")
                .expect("nonblank fixture"),
        ),
        _ => ChoiceState::Enabled,
    });
    view! {
        <Stack gap=Space::Sm>
            <div data-preview-scenario="empty-options missing-value reordered-options unsupported-option interrupted">
                <Stack gap=Space::Sm>
                    <h3>"Lựa chọn theo stable value"</h3>
                    <Select id="gallery-option-mode" label="Tình huống danh sách" value=mode
                        on_input=Callback::new(move |next: String| mode.set(next))>
                        <option value="stable">"Danh sách ban đầu"</option>
                        <option value="reordered">"Đảo thứ tự nhân vật"</option>
                        <option value="empty">"Chưa có nhân vật"</option>
                        <option value="missing">"Giá trị đã bị xóa khỏi danh sách"</option>
                        <option value="interrupted">"Tạm ngừng lựa chọn"</option>
                    </Select>
                    <Select id="gallery-stable-select" label="Nhân vật theo stable value"
                        value=Signal::derive(move || if mode.get() == "missing" { "removed".to_owned() } else { value.get() })
                        state=state on_input=Callback::new(move |next: String| { value.set(next); calls.update(|n| *n += 1); })>
                        {move || match mode.get().as_str() {
                            "empty" => view! { <option value="" disabled=true>"Chưa có nhân vật"</option> }.into_any(),
                            "reordered" => view! {
                                <option value="minh-phuong">"Nguyễn Thị Minh Phương"</option>
                                <option value="narrator">"Người dẫn chuyện"</option>
                                <option value="unavailable" disabled=true>"Người giữ đèn — thiếu quyền dùng giọng"</option>
                            }.into_any(),
                            _ => view! {
                                <option value="narrator">"Người dẫn chuyện"</option>
                                <option value="minh-phuong">"Nguyễn Thị Minh Phương"</option>
                                <option value="unavailable" disabled=true>"Người giữ đèn — thiếu quyền dùng giọng"</option>
                            }.into_any(),
                        }}
                    </Select>
                    <output id="gallery-stable-output">{move || format!("Stable value: {} · requested: {} · callback: {}", value.get(), if mode.get() == "missing" { "removed".to_owned() } else { value.get() }, calls.get())}</output>
                    <p>"Thay đổi danh sách không tự gửi callback. HTML select dùng popup của hệ điều hành; menu, Escape và focus khác CMP."</p>
                </Stack>
            </div>
            <div data-preview-scenario="stepped completion">
                <RangeField id="gallery-stepped-range" label="Mức âm lượng thử theo bước" value=range
                    min="0" max="100" step="25" value_text=Signal::derive(move || Some(format!("{} trên 100", range.get())))
                    on_input=Callback::new(move |next| range.set(next))
                    on:change=move |_| completions.update(|n| *n += 1)/>
                <output id="gallery-range-completion">{move || format!("Giá trị: {} trên 100 · hoàn tất: {}", range.get(), completions.get())}</output>
                <p>"CMP steps=3 là ba tick bên trong; HTML step=25 là khoảng giá trị. Dùng mũi tên, Home/End và quan sát input/change."</p>
            </div>
        </Stack>
    }
}

#[component]
pub fn LayoutFeedbackContractSamples() -> impl IntoView {
    let retries = RwSignal::new(0u32);
    view! {
        <Stack gap=Space::Lg>
            <div data-preview-scenario="gap-lg inset-none">
                <Surface tone=SurfaceTone::Reading inset=Space::None>
                    <Stack direction=StackDirection::Horizontal gap=Space::Lg>
                        <Button id="gallery-wrap-narrator" on_press=Callback::new(|_| {})>"Người dẫn chuyện"</Button>
                        <Button id="gallery-wrap-minh-phuong" on_press=Callback::new(|_| {})>"Nguyễn Thị Minh Phương · Ỗ, Ữ"</Button>
                        <p>"Vùng đọc có chân dùng shape.reading; các nhóm ngang xuống dòng ở kích thước nội dung."</p>
                    </Stack>
                </Surface>
            </div>
            <Container width=ContainerWidth::Reading>
                <Grid columns=GridColumns::Three gap=Space::Lg>
                    <Surface inset=Space::None>"Người dẫn chuyện · cảnh một"</Surface>
                    <Surface inset=Space::None>"Nguyễn Thị Minh Phương · cảnh hai"</Surface>
                    <Surface inset=Space::None>"Ỗ, Ữ · Ánh đèn cuối sân khấu"</Surface>
                </Grid>
            </Container>
            <div data-preview-scenario="recovery-slot announcement">
                <ErrorState title="Không thể nạp bản nháp" announcement=Announcement::Off>
                    <p>"Ghi chú vẫn còn. Chủ ứng dụng chọn cách khôi phục và thời điểm thông báo."</p>
                    <Stack direction=StackDirection::Horizontal gap=Space::Sm>
                        <Button id="gallery-recovery-retry" variant=ButtonVariant::Secondary
                            on_press=Callback::new(move |_| retries.update(|n| *n += 1))>"Thử lại"</Button>
                        <Button id="gallery-recovery-notes" variant=ButtonVariant::Text
                            on_press=Callback::new(move |_| retries.update(|n| *n += 1))>"Xem ghi chú bản nháp"</Button>
                    </Stack>
                </ErrorState>
                <Status tone=StatusTone::Info announcement=Announcement::Polite>
                    <output id="gallery-recovery-output">{move || format!("Hành động khôi phục: {}", retries.get())}</output>
                </Status>
            </div>
        </Stack>
    }
}
