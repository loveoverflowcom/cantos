package com.cantos.theatre.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.application.identity.NativeAccountFailure
import com.cantos.ui.CantosTheme
import com.cantos.ui.UiPreferences

/** Opt-in account boundary. It never imports or projects mock Library state. */
@Composable
fun NativeIdentityApp(controller: NativeAccountController) {
    val state by controller.state.collectAsState()
    CantosTheme(UiPreferences()) {
        Surface(modifier = Modifier.fillMaxSize()) {
            Column(modifier = Modifier.safeDrawingPadding().padding(24.dp).verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(16.dp)) {
                Text("Cantos", style = MaterialTheme.typography.headlineMedium, modifier = Modifier.semantics { heading() })
                when (val account = state) {
                    NativeAccountState.SignedOut -> {
                        Text("Đăng nhập để xem thư viện của bạn.")
                        Button(onClick = controller::beginLogin) { Text("Đăng nhập") }
                        TextButton(onClick = controller::restore) { Text("Khôi phục phiên đã lưu") }
                    }
                    NativeAccountState.Pending -> {
                        AccountStatus("Đang xác minh phiên đăng nhập…")
                        TextButton(onClick = controller::cancelLogin) { Text("Hủy") }
                    }
                    is NativeAccountState.SignedIn -> {
                        AccountStatus("Bạn đã đăng nhập.")
                        account.savedCount?.let { Text("Đã lưu: $it tác phẩm trên trang này.") }
                        Button(onClick = controller::refreshSaved) { Text("Xem số tác phẩm đã lưu") }
                        TextButton(onClick = controller::logout) { Text("Đăng xuất") }
                    }
                    NativeAccountState.CleanupRequired -> {
                        AccountStatus("Cần hoàn tất xóa phiên cũ trước khi đăng nhập lại.")
                        Button(onClick = controller::logout) { Text("Thử xóa phiên lại") }
                    }
                    is NativeAccountState.Failed -> {
                        AccountStatus(when (account.reason) {
                            NativeAccountFailure.Configuration -> "Đăng nhập chưa sẵn sàng. Vui lòng thử lại sau."
                            NativeAccountFailure.Entropy -> "Không thể bắt đầu đăng nhập an toàn. Vui lòng thử lại."
                            NativeAccountFailure.Login -> "Đăng nhập đã hủy hoặc hết hạn. Vui lòng thử lại."
                            NativeAccountFailure.Storage -> "Không thể đọc phiên đã lưu trên thiết bị."
                            NativeAccountFailure.Cleanup -> "Cần xóa phiên cũ trước khi đăng nhập lại."
                            NativeAccountFailure.Expired -> "Phiên đã hết hạn. Vui lòng đăng nhập lại."
                            NativeAccountFailure.Api -> "Không thể xác minh phiên qua mạng. Vui lòng thử lại."
                            NativeAccountFailure.Protocol -> "Không thể xác minh phản hồi đăng nhập an toàn."
                            NativeAccountFailure.Exhausted -> "Đăng nhập tạm thời không khả dụng. Vui lòng mở lại ứng dụng."
                        })
                        Button(onClick = controller::beginLogin) { Text("Đăng nhập lại") }
                        TextButton(onClick = controller::logout) { Text("Xóa phiên đã lưu") }
                    }
                }
            }
        }
    }
}

@Composable
private fun AccountStatus(message: String) {
    Text(message, modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite })
}
