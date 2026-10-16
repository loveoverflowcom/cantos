package com.cantos.theatre.features.auth.presentation

import com.cantos.theatre.application.identity.NativeAccountFailure
import com.cantos.theatre.presentation.model.ShellLocale
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class AuthCopyTest {
    @Test
    fun every_account_failure_preserves_its_specific_correction_in_both_locales() {
        val expected = mapOf(
            NativeAccountFailure.Configuration to ("Đăng nhập chưa sẵn sàng. Vui lòng thử lại sau." to "Sign-in is not ready. Please try again later."),
            NativeAccountFailure.Entropy to ("Không thể bắt đầu đăng nhập an toàn. Vui lòng thử lại." to "Secure sign-in could not start. Please try again."),
            NativeAccountFailure.Login to ("Đăng nhập đã hủy hoặc hết hạn. Vui lòng thử lại." to "Sign-in was cancelled or expired. Please try again."),
            NativeAccountFailure.Storage to ("Không thể đọc phiên đã lưu trên thiết bị." to "The saved session on this device could not be read."),
            NativeAccountFailure.Cleanup to ("Cần xóa phiên cũ trước khi đăng nhập lại." to "The previous session must be removed before you sign in again."),
            NativeAccountFailure.Expired to ("Phiên đã hết hạn. Vui lòng đăng nhập lại." to "The session expired. Please sign in again."),
            NativeAccountFailure.Api to ("Không thể xác minh phiên qua mạng. Vui lòng thử lại." to "The session could not be verified over the network. Please try again."),
            NativeAccountFailure.Protocol to ("Không thể xác minh phản hồi đăng nhập an toàn." to "The secure sign-in response could not be verified."),
            NativeAccountFailure.Exhausted to ("Đăng nhập tạm thời không khả dụng. Vui lòng mở lại ứng dụng." to "Sign-in is temporarily unavailable. Please reopen the app."),
        )
        assertEquals(NativeAccountFailure.entries.toSet(), expected.keys)
        expected.forEach { (reason, text) ->
            assertEquals(text.first, AuthCopy(ShellLocale.Vietnamese).failure(reason), reason.name)
            assertEquals(text.second, AuthCopy(ShellLocale.English).failure(reason), reason.name)
        }
    }

    @Test
    fun every_auth_resource_is_available_in_both_locales() {
        AuthText.entries.forEach { key ->
            assertTrue(AuthCopy(ShellLocale.Vietnamese).text(key).isNotBlank(), key.name)
            assertTrue(AuthCopy(ShellLocale.English).text(key).isNotBlank(), key.name)
        }
    }
}
