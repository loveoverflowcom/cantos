package com.cantos.theatre.features.auth.presentation

import com.cantos.theatre.application.identity.NativeAccountFailure
import com.cantos.theatre.domain.identity.RegistrationFailure
import com.cantos.theatre.presentation.model.ShellLocale

internal class AuthCopy(private val locale: ShellLocale) {
    fun text(key: AuthText): String = if (locale == ShellLocale.English) key.en else key.vi

    fun registrationFailure(reason: RegistrationFailure): String = text(when (reason) {
        RegistrationFailure.InvalidEmail -> AuthText.RegisterInvalidEmail
        RegistrationFailure.InvalidAction -> AuthText.RegisterInvalidAction
        RegistrationFailure.Disabled -> AuthText.RegisterDisabled
        RegistrationFailure.Unavailable -> AuthText.RegisterNetwork
        RegistrationFailure.Denied -> AuthText.RegisterDenied
        RegistrationFailure.Protocol -> AuthText.FailureProtocol
        RegistrationFailure.ProofUnavailable -> AuthText.RegisterProofUnavailable
        RegistrationFailure.StartUnknownOutcome -> AuthText.RegisterSendUnknown
        RegistrationFailure.UnknownOutcome -> AuthText.RegisterUnknown
        RegistrationFailure.Browser -> AuthText.RegisterBrowserFailed
    })

    fun failure(reason: NativeAccountFailure): String = text(when (reason) {
        NativeAccountFailure.Configuration -> AuthText.FailureConfiguration
        NativeAccountFailure.Entropy -> AuthText.FailureEntropy
        NativeAccountFailure.Login -> AuthText.FailureLogin
        NativeAccountFailure.Storage -> AuthText.FailureStorage
        NativeAccountFailure.Cleanup -> AuthText.FailureCleanup
        NativeAccountFailure.Expired -> AuthText.FailureExpired
        NativeAccountFailure.Api -> AuthText.FailureApi
        NativeAccountFailure.Protocol -> AuthText.FailureProtocol
        NativeAccountFailure.Exhausted -> AuthText.FailureExhausted
    })
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
    CantosIdLogin("Đăng nhập với Cantos ID", "Sign in with Cantos ID"),
    CreateAccount("Tạo tài khoản Cantos", "Create a Cantos account"),
    ProviderHelp(
        "Mở bước xác thực an toàn trong trình duyệt hệ thống, rồi quay lại Cantos. Bạn có thể quay lại mà chưa đăng nhập.",
        "Open secure authentication in the system browser, then return to Cantos. You can go back without signing in.",
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
    PendingReason("Đang mở Cantos ID. Không gửi yêu cầu lần hai.", "Opening Cantos ID. A second request is not sent."),
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

    FailureConfiguration("Đăng nhập chưa sẵn sàng. Vui lòng thử lại sau.", "Sign-in is not ready. Please try again later."),
    FailureEntropy("Không thể bắt đầu đăng nhập an toàn. Vui lòng thử lại.", "Secure sign-in could not start. Please try again."),
    FailureLogin("Đăng nhập đã hủy hoặc hết hạn. Vui lòng thử lại.", "Sign-in was cancelled or expired. Please try again."),
    FailureStorage("Không thể đọc phiên đã lưu trên thiết bị.", "The saved session on this device could not be read."),
    FailureCleanup("Cần xóa phiên cũ trước khi đăng nhập lại.", "The previous session must be removed before you sign in again."),
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
    RegisterChecking("Đang kiểm tra đăng ký…", "Checking registration availability…"),
    RegisterWaiting("Yêu cầu đã được tiếp nhận. Kiểm tra hộp thư; phản hồi này chưa xác nhận email đã gửi hoặc tài khoản đã tạo.", "The request was accepted. Check your inbox; this does not confirm delivery or account creation."),
    RegisterLink("Dán liên kết xác minh từ email", "Paste the verification link from your email"),
    RegisterImport("Kiểm tra liên kết", "Check the link"),
    RegisterConfirm("Xác minh email", "Verify email"),
    RegisterConfirmHelp("Liên kết đã sẵn sàng. Chỉ nút Xác minh email mới gửi mã tới máy chủ.", "The link is ready. Only Verify email sends the code to the server."),
    RegisterVerifying("Đang xác minh email…", "Verifying email…"),
    RegisterSetupReady("Email đã được xác minh. Mở bước đặt thông tin đăng nhập một lần trong trình duyệt; bạn chưa đăng nhập.", "Email verified. Open the one-time credential setup in the browser; you are not signed in yet."),
    RegisterOpenSetup("Đặt thông tin đăng nhập", "Set up credentials"),
    RegisterCredentialSetup("Hoàn tất trong trình duyệt, rồi trở lại Đăng nhập. Chỉ phiên được máy chủ xác nhận mới mở thư viện của bạn.", "Finish in the browser, then return to sign-in. Your library opens only after the server confirms your session."),
    RegisterCancel("Hủy đăng ký", "Cancel registration"),
    RegisterCancelling("Đang yêu cầu hủy…", "Requesting cancellation…"),
    RegisterCancelled("Đã dừng đăng ký tại đây. Hủy không thu hồi thao tác máy chủ đã hoàn tất.", "Registration stopped here. Cancellation cannot undo work already completed by the server."),
    RegisterRestart("Bắt đầu yêu cầu mới", "Start a new request"),
    RegisterRetry("Kiểm tra lại", "Check again"),
    RegisterInvalidEmail("Nhập địa chỉ email hợp lệ.", "Enter a valid email address."),
    RegisterInvalidAction("Liên kết không hợp lệ hoặc không thuộc máy chủ này. Dán nguyên liên kết từ email.", "The link is invalid or belongs to another server. Paste the complete link from your email."),
    RegisterNetwork("Không kết nối được dịch vụ đăng ký. Kiểm tra kết nối rồi thử lại.", "Registration could not be reached. Check your connection and try again."),
    RegisterDenied("Máy chủ từ chối yêu cầu đăng ký trên bản này.", "The server refused registration from this build."),
    RegisterProofUnavailable("Mã đã hết hạn, đã dùng hoặc không thể xác minh. Không gửi lại mã này; bắt đầu yêu cầu mới hoặc đăng nhập.", "The code is expired, already used or could not be verified. Do not resend this code; start a new request or sign in."),
    RegisterSendUnknown("Chưa biết máy chủ đã nhận yêu cầu hay chưa. Kiểm tra hộp thư trước khi bắt đầu yêu cầu mới.", "The submission outcome is unknown. Check your inbox before starting a new request."),
    RegisterUnknown("Chưa xác định được kết quả xác minh hoặc bàn giao. Không thử lại mã hay liên kết đặt thông tin; cần đối soát hoặc đăng nhập nếu đã hoàn tất.", "Verification or setup delivery has an unknown outcome. Do not retry the code or setup link; reconcile it, or sign in if setup finished."),
    RegisterBrowserFailed("Không mở được trình duyệt. Liên kết một lần đã được bỏ khỏi ứng dụng; cần đối soát bàn giao trước khi thử lại.", "The browser could not open. The one-time link was removed from the app; reconcile setup delivery before trying again."),
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
        "Thư viện của bản này vẫn chỉ lưu trong phiên, chưa đồng bộ với tài khoản.",
        "This build's library is still session-only and is not synced with your account.",
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
