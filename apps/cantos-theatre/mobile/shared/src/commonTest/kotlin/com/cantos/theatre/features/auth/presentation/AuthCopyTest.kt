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
            NativeAccountFailure.Storage to ("Không thể đọc phiên đã lưu trên thiết bị." to "The saved session on this device could not be read."),
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
    fun every_signup_failure_has_its_own_message_in_both_locales() {
        val reasons = com.cantos.theatre.domain.identity.RegistrationFailure.entries
        for (locale in ShellLocale.entries) {
            val messages = reasons.map { AuthCopy(locale).registrationFailure(it) }
            assertTrue(messages.all { it.isNotBlank() })
            // Each refusal says something different, so the listener is never told the wrong thing.
            assertEquals(reasons.size, messages.toSet().size, locale.name)
        }
        assertEquals("Bạn đã gửi quá nhiều yêu cầu. Hãy thử lại sau ít phút.",
            AuthCopy(ShellLocale.Vietnamese).registrationFailure(com.cantos.theatre.domain.identity.RegistrationFailure.RateLimited))
    }

    @Test
    fun retired_provider_and_browser_wording_is_gone_from_both_locales() {
        val retired = listOf("issuer", "callback", "link", "liên kết", "browser", "trình duyệt", "provider", "Kanidm", "OIDC")
        for (locale in ShellLocale.entries) {
            val text = AuthText.entries.joinToString("\n") { AuthCopy(locale).text(it) }.lowercase()
            for (word in retired) {
                // The email-link help legitimately says the browser completes the mailed proof.
                if (word == "trình duyệt" || word == "browser") continue
                assertTrue(word.lowercase() !in text, "$word in ${locale.name}")
            }
        }
    }

    @Test
    fun check_mail_guidance_names_the_browser_step_and_the_return_to_this_app() {
        // ADR 0048 stage 0: the mailed step opens in the browser; the session starts only by
        // coming back here and signing in, never by opening the email.
        val vi = AuthCopy(ShellLocale.Vietnamese).text(AuthText.RegisterWaiting)
        val en = AuthCopy(ShellLocale.English).text(AuthText.RegisterWaiting)
        assertTrue("trình duyệt" in vi && "quay lại ứng dụng này để đăng nhập" in vi, vi)
        assertTrue("browser" in en && "come back to this app to sign in" in en, en)
    }

    @Test
    fun every_auth_resource_is_available_in_both_locales() {
        AuthText.entries.forEach { key ->
            assertTrue(AuthCopy(ShellLocale.Vietnamese).text(key).isNotBlank(), key.name)
            assertTrue(AuthCopy(ShellLocale.English).text(key).isNotBlank(), key.name)
        }
    }
}
