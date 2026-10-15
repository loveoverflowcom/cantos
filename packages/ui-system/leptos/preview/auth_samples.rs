//! Auth/account design-v2 specimens only. No identity, navigation or payment effects.
use cantos_ui::{
    AccessibleName, BackButton, Button, ButtonState, ButtonVariant, DisabledReason, FieldState,
    IconName, Select, Space, Stack, Status, Surface, SurfaceTone, SurfaceVariant, TextField,
    TextFieldKind, TextFieldVariant,
};
use leptos::prelude::*;

fn reason(text: &str) -> DisabledReason {
    DisabledReason::new(text).expect("nonblank diagnostic reason")
}

#[component]
pub fn AuthActionSamples(
    #[prop(default = "enabled")] initial_state: &'static str,
) -> impl IntoView {
    let mode = RwSignal::new(initial_state.to_owned());
    let presses = RwSignal::new(0u32);
    let state = Signal::derive(move || match mode.get().as_str() {
        "loading" => ButtonState::Loading(reason(
            "Đang chờ trong mẫu; dữ liệu và nhãn vẫn được giữ. Chọn Sẵn sàng để thử lại.",
        )),
        "disabled" => ButtonState::Disabled(reason(
            "Hành động mẫu bị khóa. Chọn Sẵn sàng trước khi tiếp tục.",
        )),
        _ => ButtonState::Enabled,
    });
    let press = Callback::new(move |_| presses.update(|count| *count += 1));
    view! {
        <Surface tone=SurfaceTone::Container variant=SurfaceVariant::Auth inset=Space::Lg>
            <Stack gap=Space::Sm class="cui-auth-specimen">
                <h3>"Hành động auth/account · Design v2"</h3>
                <p>"Nhấn giữ để quan sát shape; dùng Tab để quan sát focus. Đây là intent mẫu, không đăng nhập hoặc tạo tài khoản."</p>
                <Select id="auth-action-state" label="Trạng thái hành động auth mẫu" value=mode
                    on_input=Callback::new(move |value| mode.set(value))>
                    <option value="enabled">"Sẵn sàng"</option>
                    <option value="loading">"Đang chờ"</option>
                    <option value="disabled">"Bị khóa"</option>
                </Select>
                <div data-preview-scenario="auth-primary auth-focus auth-pressed auth-long-label auth-loading auth-disabled">
                    <Button id="auth-primary" variant=ButtonVariant::AuthPrimary icon=IconName::ArrowForward
                        state=state on_press=press>
                        "Đăng nhập với Cantos ID để giữ những câu chuyện và dấu trang của Nguyễn Thị Ánh Nguyệt"
                    </Button>
                </div>
                <div data-preview-scenario="auth-secondary">
                    <Button id="auth-secondary" variant=ButtonVariant::AuthSecondary state=state on_press=press>
                        "Tạo tài khoản Cantos"
                    </Button>
                </div>
                <div data-preview-scenario="auth-outlined">
                    <Button id="auth-outlined" variant=ButtonVariant::AuthOutlined state=state on_press=press>
                        "Thử lại hành động mẫu, giữ nguyên thông tin đã nhập"
                    </Button>
                </div>
                <p id="auth-action-output" class="cui-gallery-output">{move || format!("Số intent được nhận: {}", presses.get())}</p>
                <Button id="auth-regular-control" on_press=press>"Hành động thường không đổi hình/kích thước"</Button>
            </Stack>
        </Surface>
    }
}

#[component]
pub fn AuthFieldSamples(
    #[prop(default = "editable")] initial_state: &'static str,
    #[prop(optional)] initial_error: bool,
) -> impl IntoView {
    let mode = RwSignal::new(initial_state.to_owned());
    let invalid = RwSignal::new(initial_error);
    let text = RwSignal::new("nguyen-thi-anh-nguyet@example.test".to_owned());
    let edits = RwSignal::new(0u32);
    let state = Signal::derive(move || match mode.get().as_str() {
        "readonly" => FieldState::ReadOnly,
        "disabled" => FieldState::Disabled(reason("Trường mẫu bị khóa; giá trị được giữ nguyên.")),
        "loading" => {
            FieldState::Loading(reason("Đang chờ trong mẫu; không gửi email hoặc mật khẩu."))
        }
        _ => FieldState::Editable,
    });
    let error = Signal::derive(move || {
        invalid
            .get()
            .then(|| "Ví dụ lỗi: kiểm tra địa chỉ email; giá trị đã nhập không bị xóa.".to_owned())
    });
    view! {
        <Surface tone=SurfaceTone::Container variant=SurfaceVariant::Auth>
            <Stack gap=Space::Sm>
                <h3>"Trường tonal auth · nhãn luôn hiện"</h3>
                <Select id="auth-field-state" label="Trạng thái trường auth mẫu" value=mode
                    on_input=Callback::new(move |value| mode.set(value))>
                    <option value="editable">"Có thể nhập"</option>
                    <option value="readonly">"Chỉ đọc, có thể chọn/sao chép"</option>
                    <option value="disabled">"Bị khóa"</option>
                    <option value="loading">"Đang chờ"</option>
                </Select>
                <Button id="auth-field-toggle-error" variant=ButtonVariant::Text
                    on_press=Callback::new(move |_| invalid.update(|value| *value = !*value))>"Bật/tắt lỗi mẫu"</Button>
                <div data-preview-scenario="auth-tonal auth-error auth-field-focus auth-field-disabled auth-field-loading auth-field-readonly">
                    <TextField id="auth-email" variant=TextFieldVariant::AuthTonal kind=TextFieldKind::Email
                        label="Email của Nguyễn Thị Ánh Nguyệt để tiếp tục câu chuyện đang nghe"
                        value=text state=state error=error autocomplete="email"
                        help="Nhãn, giá trị và lỗi được giữ khi đổi trạng thái. Không có yêu cầu mạng."
                        on_input=Callback::new(move |value| {
                            text.set(value);
                            edits.update(|count| *count += 1);
                        })/>
                </div>
                <p id="auth-field-output" class="cui-gallery-output">{move || format!("Số lần nhận input: {} · Giá trị: {}", edits.get(), text.get())}</p>
                <TextField id="auth-regular-field" label="Trường thường không đổi geometry" value="Sơn Hà Ký"
                    on_input=Callback::new(|_: String| {})/>
            </Stack>
        </Surface>
    }
}

#[component]
pub fn AuthSurfaceSamples() -> impl IntoView {
    view! {
        <Stack gap=Space::Sm>
            <div data-preview-scenario="auth-container">
                <Surface tone=SurfaceTone::Container variant=SurfaceVariant::Auth inset=Space::Lg>
                    <h3 class="cui-auth-heading">"Auth · vùng nội dung yên tĩnh"</h3>
                    <p>"Nhắm mắt lại. Mở một thế giới. Surface tonal và khoảng trống phân nhóm thay cho viền trang trí."</p>
                </Surface>
            </div>
            <div data-preview-scenario="profile-container">
                <Surface tone=SurfaceTone::PrimaryContainer variant=SurfaceVariant::Profile inset=Space::Lg>
                    <h3>"Chào, Nguyễn Thị Ánh Nguyệt."</h3>
                    <p>"Giữ những câu chuyện đã chọn, tìm lại những đoạn kể còn ngân."</p>
                    <p>"Thông tin phụ cũng dùng foreground của primary-container, không dùng muted ink thiếu tương phản."</p>
                </Surface>
            </div>
            <div data-preview-scenario="membership-container">
                <Surface tone=SurfaceTone::Container variant=SurfaceVariant::Membership inset=Space::Lg>
                    <h3 class="cui-membership-heading">"Một người nghe. Một người đồng hành."</h3>
                    <p>"Cantos Membership · ví dụ nội dung, không cấp quyền hội viên hoặc thanh toán."</p>
                </Surface>
            </div>
            <div data-preview-scenario="account-group">
                <Surface tone=SurfaceTone::Plain variant=SurfaceVariant::AccountGroup>
                    <h3>"Nhóm tài khoản"</h3>
                    <p>"Giao diện, cài đặt nghe và trạng thái khách được ghép bởi host. Không tự thêm hành động đăng xuất."</p>
                </Surface>
            </div>
        </Stack>
    }
}

#[component]
pub fn BackButtonSamples(
    #[prop(default = "enabled")] initial_state: &'static str,
) -> impl IntoView {
    let mode = RwSignal::new(initial_state.to_owned());
    let returns = RwSignal::new(0u32);
    let name = RwSignal::new(
        AccessibleName::new("Về Trang chủ, giữ vị trí câu chuyện đang xem")
            .expect("nonblank sample name"),
    );
    let state = Signal::derive(move || match mode.get().as_str() {
        "disabled" => {
            ButtonState::Disabled(reason("Back bị khóa trong mẫu; chọn Sẵn sàng để tiếp tục."))
        }
        "loading" => {
            ButtonState::Loading(reason("Đang chờ host mẫu; Back không phát intent lần hai."))
        }
        _ => ButtonState::Enabled,
    });
    let on_back = Callback::new(move |_| returns.update(|count| *count += 1));
    view! {
        <Surface tone=SurfaceTone::Container variant=SurfaceVariant::AccountGroup>
            <Stack gap=Space::Sm>
                <h3>"Back theo ngữ cảnh · host sở hữu nơi trở về"</h3>
                <Select id="auth-back-state" label="Trạng thái Back mẫu" value=mode
                    on_input=Callback::new(move |value| mode.set(value))>
                    <option value="enabled">"Sẵn sàng"</option>
                    <option value="loading">"Đang chờ"</option>
                    <option value="disabled">"Bị khóa"</option>
                </Select>
                <div data-preview-scenario="back-visible-label back-focus back-pressed back-loading back-disabled">
                    <BackButton id="auth-back-visible" name=name state=state on_back=on_back/>
                </div>
                <div data-preview-scenario="back-icon-only">
                    <BackButton id="auth-back-icon" name=name icon_only=true state=state on_back=on_back/>
                </div>
                <Button id="auth-back-change-context" variant=ButtonVariant::Text
                    on_press=Callback::new(move |_| name.set(AccessibleName::new("Về Tài khoản, giữ nguyên ý định đăng ký Membership").expect("nonblank alternate name")))>
                    "Đổi nơi quay về trong mẫu"
                </Button>
                <Status><p id="auth-back-output">{move || format!("Host nhận {} intent Back; không gọi history, đăng xuất hoặc đăng nhập.", returns.get())}</p></Status>
            </Stack>
        </Surface>
    }
}
