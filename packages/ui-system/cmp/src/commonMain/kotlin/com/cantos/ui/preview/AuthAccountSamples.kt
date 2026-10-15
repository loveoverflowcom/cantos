package com.cantos.ui.preview

import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.PressInteraction
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import com.cantos.ui.CantosTypography
import com.cantos.ui.AccessibleName
import com.cantos.ui.BackButton
import com.cantos.ui.Button
import com.cantos.ui.ButtonState
import com.cantos.ui.ButtonVariant
import com.cantos.ui.DisabledReason
import com.cantos.ui.FieldError
import com.cantos.ui.FieldHelp
import com.cantos.ui.FieldState
import com.cantos.ui.Surface
import com.cantos.ui.SurfaceTone
import com.cantos.ui.SurfaceVariant
import com.cantos.ui.TextField
import com.cantos.ui.TextFieldVariant
import com.cantos.ui.tokenDp
import kotlinx.coroutines.awaitCancellation

/** A real interaction source drives the specimen; disposal cancels its retained press. */
@Composable
internal fun rememberSamplePress(pressed: Boolean): MutableInteractionSource {
    val source = remember { MutableInteractionSource() }
    LaunchedEffect(source, pressed) {
        if (pressed) {
            // Let the actual control subscribe before publishing this retained specimen press.
            withFrameNanos { }
            val press = PressInteraction.Press(Offset.Zero)
            source.emit(press)
            try {
                awaitCancellation()
            } finally {
                source.tryEmit(PressInteraction.Cancel(press))
            }
        }
    }
    return source
}

@Composable
internal fun AuthActionSample(state: String, onPress: () -> Unit, modifier: Modifier = Modifier) {
    val variant = when (state) {
        "auth-secondary" -> ButtonVariant.AuthSecondary
        "auth-outlined" -> ButtonVariant.AuthOutlined
        else -> ButtonVariant.AuthPrimary
    }
    val availability = when (state) {
        "auth-loading" -> ButtonState.Loading(DisabledReason.of("Đang mở Cantos ID. Không gửi yêu cầu lần hai."))
        "auth-disabled" -> ButtonState.Disabled(DisabledReason.of("Kết nối đang tạm ngừng. Bạn vẫn có thể quay lại."))
        else -> ButtonState.Enabled
    }
    val label = when (state) {
        "auth-secondary" -> "Tạo tài khoản Cantos"
        "auth-outlined" -> "Để sau, tiếp tục nghe nội dung công khai"
        "auth-long-label" -> "Đăng nhập với Cantos ID để giữ tủ sách của Nguyễn Thị Minh Phương · Ỗ, Ữ"
        else -> "Đăng nhập với Cantos ID"
    }
    Button(
        name = AccessibleName.of(label),
        onPress = onPress,
        state = availability,
        variant = variant,
        controlModifier = modifier.fillMaxWidth(),
        interactionSource = rememberSamplePress(state == "auth-pressed"),
        icon = if (variant == ButtonVariant.AuthPrimary) "arrow_forward" else null,
    ) {
        Text(label)
    }
    if (state == "auth-error") FieldError("Không mở được Cantos ID. Dữ liệu trong phiên vẫn được giữ lại.")
}

/** Package-only anatomy, not an adopted auth screen, route, account or password form. */
@Composable
internal fun AuthAccountSpecimen(
    state: String = "idle",
    modifier: Modifier = Modifier,
    onEvent: (String) -> Unit = {},
) {
    var title by remember { mutableStateOf("Nguyễn Thị Minh Phương · Ỗ, Ữ") }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.lg"))) {
        BackButton(AccessibleName.of("Quay lại Tài khoản"), { onEvent("back:account") })
        Surface(tone = SurfaceTone.Surface, variant = SurfaceVariant.Auth, inset = com.cantos.ui.Space.Lg) {
            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                Text("Câu chuyện đang đợi bạn.", style = CantosTypography.authHeading)
                Text("Nội dung công khai vẫn có thể nghe ngay. Đây là mẫu UI, không mở xác thực hay gửi thông tin.")
                AuthActionSample(if (state == "idle") "auth-primary" else state, { onEvent("auth:primary") })
                AuthActionSample("auth-secondary", { onEvent("auth:secondary") })
                TextField(
                    "Tên hiển thị minh họa · không phải thông tin đăng nhập", title, { title = it },
                    variant = TextFieldVariant.AuthTonal,
                    state = when (state) {
                        "auth-disabled" -> FieldState.Disabled(DisabledReason.of("Chưa thể chỉnh sửa thông tin minh họa."))
                        "auth-loading" -> FieldState.Loading(DisabledReason.of("Đang nạp thông tin minh họa, nội dung vẫn được giữ."))
                        else -> FieldState.Editable
                    },
                    help = "Giữ nguyên dấu tiếng Việt và nội dung đã nhập.",
                    error = if (state == "auth-error") "Tên hiển thị cần được kiểm tra, nội dung chưa bị xóa." else null,
                )
            }
        }
        Surface(tone = SurfaceTone.PrimaryContainer, variant = SurfaceVariant.Profile, inset = com.cantos.ui.Space.Lg) {
            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                Text("Chào, Nguyễn Thị Minh Phương.", style = MaterialTheme.typography.headlineSmall)
                Text("Giữ những câu chuyện đã chọn, tìm lại những đoạn kể còn ngân. · Ỗ, Ữ")
                FieldHelp("Thông tin minh họa trên nền primary-container dùng đúng cặp chữ, không dùng muted thiếu tương phản.")
                Button("Mở tủ sách", { onEvent("profile:library") }, variant = ButtonVariant.AuthPrimary,
                    controlModifier = Modifier.fillMaxWidth())
                Button("Đồng bộ tủ sách minh họa", {}, variant = ButtonVariant.AuthSecondary,
                    state = ButtonState.Disabled(DisabledReason.of("Chưa kết nối tài khoản, không gửi yêu cầu đồng bộ.")))
            }
        }
        Surface(tone = SurfaceTone.Container, variant = SurfaceVariant.Membership, inset = com.cantos.ui.Space.Lg) {
            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                Text("Cantos Membership", style = MaterialTheme.typography.titleLarge)
                Text("Một người nghe. Một người đồng hành.", style = CantosTypography.membershipTitle)
                FieldHelp("Mẫu giao diện, không tự cấp quyền hội viên hay phát sinh thanh toán.")
                Button("Tìm hiểu Membership", { onEvent("membership:learn") }, variant = ButtonVariant.AuthSecondary)
            }
        }
        Surface(tone = SurfaceTone.Surface, variant = SurfaceVariant.AccountGroup) {
            Text("Giao diện và cài đặt nghe thuộc một nhóm yên tĩnh. Không có viền trang trí.")
        }
    }
}
