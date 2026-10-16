package com.cantos.theatre.features.membership.presentation

import com.cantos.theatre.domain.membership.MembershipFieldError
import com.cantos.theatre.domain.membership.PlansStatus
import com.cantos.theatre.presentation.model.ShellLocale
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class MembershipCopyTest {
    @Test
    fun every_membership_field_error_preserves_its_specific_correction_in_both_locales() {
        val expected = mapOf(
            MembershipFieldError.NameRequired to ("Nhập tên bạn muốn sử dụng." to "Enter the name you want to use."),
            MembershipFieldError.NameTooLong to ("Tên tối đa 120 ký tự Unicode." to "Use no more than 120 Unicode characters for your name."),
            MembershipFieldError.NameInvalid to ("Tên không được chứa ký tự điều khiển." to "Your name must not contain control characters."),
            MembershipFieldError.EmailRequired to ("Nhập địa chỉ email." to "Enter your email address."),
            MembershipFieldError.EmailTooLong to ("Email tối đa 254 ký tự Unicode." to "Use no more than 254 Unicode characters for email."),
            MembershipFieldError.EmailInvalid to ("Nhập email hợp lệ với một dấu @ và không có khoảng trắng." to "Enter an email with one @ and no whitespace."),
        )
        assertEquals(MembershipFieldError.entries.toSet(), expected.keys)
        expected.forEach { (reason, text) ->
            assertEquals(text.first, MembershipCopy(ShellLocale.Vietnamese).error(reason), reason.name)
            assertEquals(text.second, MembershipCopy(ShellLocale.English).error(reason), reason.name)
        }
    }

    @Test
    fun plan_status_copy_distinguishes_unpublished_loading_empty_unavailable_and_failed() {
        val expected = mapOf(
            PlansStatus.Unpublished to ("Chưa công bố các gói Membership." to "Membership plans have not been announced."),
            PlansStatus.Loading to ("Fixture: đang tải thông tin gói…" to "Fixture: loading plan information…"),
            PlansStatus.Empty to ("Fixture: chưa có gói để hiển thị." to "Fixture: no plans to display."),
            PlansStatus.Unavailable to ("Thông tin các gói hiện chưa khả dụng." to "Plan information is currently unavailable."),
            PlansStatus.Failed to ("Fixture: chưa tải được thông tin gói. Xác nhận đăng ký vẫn được giữ." to "Fixture: plan information could not load. Your receipt is retained."),
        )
        assertEquals(PlansStatus.entries.toSet(), expected.keys)
        expected.forEach { (status, text) ->
            assertEquals(text.first, MembershipCopy(ShellLocale.Vietnamese).plans(status), status.name)
            assertEquals(text.second, MembershipCopy(ShellLocale.English).plans(status), status.name)
        }
    }

    @Test
    fun every_membership_resource_is_available_in_both_locales() {
        MembershipText.entries.forEach { key ->
            assertTrue(MembershipCopy(ShellLocale.Vietnamese).text(key).isNotBlank(), key.name)
            assertTrue(MembershipCopy(ShellLocale.English).text(key).isNotBlank(), key.name)
        }
    }
}
