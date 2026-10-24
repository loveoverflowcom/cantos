package com.cantos.theatre.features.auth.presentation

import com.cantos.theatre.application.identity.NativeAccountFailure
import com.cantos.theatre.application.identity.AccountLoginFailure
import com.cantos.theatre.domain.identity.RegistrationFailure
import com.cantos.theatre.presentation.model.ShellLocale

internal class AuthCopy(private val locale: ShellLocale) {
    fun text(key: AuthText): String = if (locale == ShellLocale.English) key.en else key.vi

    fun registrationFailure(reason: RegistrationFailure): String = text(when (reason) {
        RegistrationFailure.InvalidEmail -> AuthText.RegisterInvalidEmail
        RegistrationFailure.Disabled -> AuthText.RegisterDisabled
        RegistrationFailure.Denied -> AuthText.RegisterDenied
        RegistrationFailure.Protocol -> AuthText.FailureProtocol
        RegistrationFailure.RateLimited -> AuthText.RegisterRateLimited
        RegistrationFailure.StartUnknownOutcome -> AuthText.RegisterSendUnknown
    })

    fun failure(reason: NativeAccountFailure, identity: AccountLoginFailure? = null): String {
        if (identity is AccountLoginFailure.RateLimited) {
            val seconds = identity.retryAfterSeconds
            return if (seconds == null) text(AuthText.RetryLater) else text(AuthText.RetryAfter).replace("{seconds}", seconds.toString())
        }
        if (identity != null) return text(when (identity) {
            AccountLoginFailure.InvalidCredentials -> AuthText.InvalidCredentials
            AccountLoginFailure.Protocol -> AuthText.FailureProtocol
            AccountLoginFailure.RetryLater, is AccountLoginFailure.RateLimited -> AuthText.RetryLater
        })
        return text(when (reason) {
            NativeAccountFailure.Storage -> AuthText.FailureStorage
            NativeAccountFailure.Expired -> AuthText.FailureExpired
            NativeAccountFailure.Api -> AuthText.FailureApi
            NativeAccountFailure.Protocol -> AuthText.FailureProtocol
            NativeAccountFailure.Exhausted -> AuthText.FailureExhausted
        })
    }
}

/**
 * Listener-facing resources for the optional account flow. The brand is only **Cantos**.
 * Nothing here is a name, count or entitlement: those need a real contract before display.
 */
internal enum class AuthText(val vi: String, val en: String) {
    Brand("Cantos", "Cantos"),
    Back("Quay lại", "Back"),
    ReturnHome("Về Khám phá", "Back to Discover"),
    ReturnAccount("Về Tài khoản", "Back to Account"),
    ReturnMembership("Về Membership", "Back to Membership"),
    ReturnLogin("Về Đăng nhập", "Back to sign-in"),
    ToggleTheme("Chuyển giao diện sáng tối", "Switch light and dark theme"),

    LoginEyebrow("CANTOS · MỘT LẦN TRỞ LẠI", "CANTOS · A PLACE TO RETURN"),
    LoginHeading("Câu chuyện đang đợi bạn.", "Your story is waiting."),
    LoginHelp(
        "Đăng nhập để trở lại thư viện của bạn. Nội dung công khai vẫn có thể nghe ngay mà không cần đăng nhập.",
        "Sign in to return to your library. Public stories are still available without signing in.",
    ),
    Password("Mật khẩu", "Password"),
    PasswordRequired("Nhập mật khẩu.", "Enter your password."),
    ShowPassword("Hiện mật khẩu", "Show password"),
    HidePassword("Ẩn mật khẩu", "Hide password"),
    InvalidCredentials("Không thể đăng nhập bằng thông tin này.", "Unable to sign in with these credentials."),
    RetryLater("Đăng nhập tạm thời chưa khả dụng. Vui lòng thử lại sau.", "Sign-in is temporarily unavailable. Please try again later."),
    RetryAfter("Vui lòng đợi {seconds} giây trước khi thử lại.", "Please wait {seconds} seconds before trying again."),
    CantosIdLogin("Đăng nhập", "Sign in"),
    CreateAccount("Tạo tài khoản Cantos", "Create a Cantos account"),
    PasswordHelp(
        "Dùng email và mật khẩu đã thiết lập sau khi xác minh hộp thư.",
        "Use the email and password you set up after verifying your mailbox.",
    ),
    BrandHeading("Nhắm mắt lại. Mở một thế giới.", "Close your eyes. Open a world."),
    BrandHelp(
        "Có những câu chuyện chỉ cần lắng nghe. Có những miền tưởng tượng luôn chờ bạn.",
        "Some stories only ask you to listen. Worlds of imagination are waiting for you.",
    ),
    BrandGenres("Truyện · Podcast · Kịch truyền thanh", "Stories · Podcasts · Audio drama"),
    RestoreSession("Khôi phục phiên đã lưu", "Restore the saved session"),
    NothingSaved("Thiết bị này chưa lưu phiên nào để khôi phục.", "This device has no saved session to restore."),
    CancelLogin("Hủy đăng nhập", "Cancel sign-in"),
    PendingReason("Đang đăng nhập. Không gửi yêu cầu lần hai.", "Signing in. A second request is not sent."),
    PendingStatus("Đang xác minh phiên đăng nhập…", "Verifying the sign-in session…"),
    UnavailableReason(
        "Đăng nhập chưa khả dụng trên bản này. Bạn vẫn nghe được nội dung công khai.",
        "Sign-in is not available in this build. Public stories still play.",
    ),
    CleanupRequired(
        "Cần hoàn tất xóa phiên cũ trước khi đăng nhập lại.",
        "The previous session must be removed before you sign in again.",
    ),
    CleanupReason(
        "Đăng nhập tạm dừng cho đến khi phiên cũ được xóa.",
        "Sign-in is paused until the previous session is removed.",
    ),
    RetryCleanup("Thử xóa phiên lại", "Retry removing the session"),

    FailureStorage("Không thể đọc phiên đã lưu trên thiết bị.", "The saved session on this device could not be read."),
    FailureExpired("Phiên đã hết hạn. Vui lòng đăng nhập lại.", "The session expired. Please sign in again."),
    FailureApi("Không thể xác minh phiên qua mạng. Vui lòng thử lại.", "The session could not be verified over the network. Please try again."),
    FailureProtocol("Không thể xác minh phản hồi đăng nhập an toàn.", "The secure sign-in response could not be verified."),
    FailureExhausted("Đăng nhập tạm thời không khả dụng. Vui lòng mở lại ứng dụng.", "Sign-in is temporarily unavailable. Please reopen the app."),

    RegisterEyebrow("CANTOS ID · TÀI KHOẢN CỦA BẠN", "CANTOS ID · YOUR ACCOUNT"),
    RegisterHeading("Một nơi để trở lại.", "A place to return to."),
    RegisterHelp(
        "Xác minh email trước, rồi đặt thông tin đăng nhập trong trình duyệt an toàn. Cantos không nhận mật khẩu tại đây.",
        "Verify your email, then set up credentials in the secure browser. Cantos does not collect your password here.",
    ),
    SignupUnavailable(
        "Đăng ký chưa sẵn sàng trên bản này. Bạn vẫn có thể đăng nhập hoặc nghe nội dung công khai.",
        "Registration is not ready in this build. You can still sign in or play public stories.",
    ),
    RegisterDisabled(
        "Máy chủ chưa bật đăng ký cho bản này. Bạn vẫn có thể đăng nhập hoặc nghe nội dung công khai.",
        "Registration is not enabled by the server for this build. You can still sign in or play public stories.",
    ),
    RegisterEmail("Địa chỉ email", "Email address"),
    RegisterSend("Gửi yêu cầu xác minh", "Request email verification"),
    RegisterSending("Đang gửi yêu cầu…", "Submitting the request…"),
    RegisterWaiting("Yêu cầu đã được tiếp nhận. Mở email xác minh trong hộp thư: Cantos sẽ mở trong trình duyệt để bạn xác minh và đặt mật khẩu. Xong bước đó, hãy quay lại ứng dụng này để đăng nhập. Phản hồi này chưa xác nhận email đã gửi hoặc tài khoản đã tạo.", "The request was accepted. Open the verification email: Cantos opens in your browser so you can verify and set your password. When that is done, come back to this app to sign in. This does not confirm delivery or account creation."),
    RegisterCancel("Hủy đăng ký", "Cancel registration"),
    RegisterCancelled("Đã dừng đăng ký tại đây. Hủy không thu hồi thao tác máy chủ đã hoàn tất.", "Registration stopped here. Cancellation cannot undo work already completed by the server."),
    RegisterRestart("Bắt đầu yêu cầu mới", "Start a new request"),
    RegisterRetry("Kiểm tra lại", "Check again"),
    RegisterInvalidEmail("Nhập địa chỉ email hợp lệ.", "Enter a valid email address."),
    RegisterDenied("Máy chủ từ chối yêu cầu đăng ký trên bản này.", "The server refused registration from this build."),
    RegisterRateLimited("Bạn đã gửi quá nhiều yêu cầu. Hãy thử lại sau ít phút.", "Too many requests were sent. Please try again in a few minutes."),
    RegisterSendUnknown("Chưa biết máy chủ đã nhận yêu cầu hay chưa. Kiểm tra hộp thư trước khi bắt đầu yêu cầu mới.", "The submission outcome is unknown. Check your inbox before starting a new request."),
    RegisterLater("Để sau, về Khám phá", "Later, back to Discover"),

    GuestBadge("Người nghe khách", "Guest listener"),
    GuestHeading("Chào người nghe.", "Welcome, listener."),
    GuestHelp(
        "Bạn vẫn có thể khám phá và nghe nội dung công khai. Đăng nhập khi muốn mở thư viện của riêng mình.",
        "You can still explore and play public stories. Sign in when you want to open your own library.",
    ),
    SignIn("Đăng nhập", "Sign in"),
    CreateAccountShort("Tạo tài khoản", "Create an account"),
    SignedBadge("Đã đăng nhập", "Signed in"),
    SignedHeading("Chào mừng bạn trở lại.", "Welcome back."),
    SignedHelp(
        "Giữ những câu chuyện đã chọn, tìm lại những đoạn kể còn ngân.",
        "Return to the stories you saved and the passages that stay with you.",
    ),
    OpenLibrary("Mở thư viện", "Open my library"),
    LibraryNote(
        "Thư viện được lưu riêng theo tài khoản. Các thay đổi đang chờ sẽ đồng bộ khi kết nối khả dụng.",
        "Your library is stored separately for each account. Pending changes sync when a connection is available.",
    ),
    MembershipBadge("Sắp ra mắt", "Coming soon"),
    MembershipName("Cantos Membership", "Cantos Membership"),
    MembershipHeading("Một người nghe. Một người đồng hành.", "A listener. A companion."),
    MembershipHelp("Tìm hiểu chương trình hội viên khi bạn sẵn sàng.", "Learn about membership whenever you are ready."),
    MembershipSignedHelp(
        "Thông tin hội viên hiện chưa khả dụng. Đăng nhập không tự kích hoạt Membership.",
        "Membership details are currently unavailable. Signing in does not activate Membership.",
    ),
    MembershipLearn("Tìm hiểu Membership", "Explore Membership"),
    Appearance("Giao diện", "Appearance"),
    AppearanceLight("Đang dùng giao diện sáng", "Using the light appearance"),
    AppearanceDark("Đang dùng giao diện tối", "Using the dark appearance"),
    ListeningSettings("Cài đặt nghe", "Listening settings"),
    ListeningSettingsHint("Tốc độ, hẹn giờ và ngôn ngữ", "Speed, timer and language"),
    Logout("Đăng xuất", "Sign out"),
    LogoutHint("Trở về trạng thái người nghe khách", "Return to the guest listener state"),
}
