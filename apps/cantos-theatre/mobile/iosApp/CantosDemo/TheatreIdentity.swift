import Foundation
import Security
import CryptoKit
import AuthenticationServices
import UIKit

enum TheatreCredentialError: Error { case unavailable, denied, corrupt }

/// Adapter source, intentionally unmounted in the mock host. No UserDefaults/file fallback.
final class TheatreCredentialStore {
    private let service = "com.cantos.theatre.identity.v1"
    private let account = "cantos-theatre-native"
    private var query: [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service,
         kSecAttrAccount as String: account, kSecAttrSynchronizable as String: false]
    }
    func read() throws -> String? {
        var request = query
        request[kSecReturnData as String] = true
        request[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(request as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess else { throw status == errSecInteractionNotAllowed ? TheatreCredentialError.denied : .unavailable }
        guard let bytes = result as? Data, bytes.count <= 32768, let value = String(data: bytes, encoding: .utf8) else { throw TheatreCredentialError.corrupt }
        return value
    }
    func write(_ value: String) throws {
        let bytes = Data(value.utf8)
        guard !bytes.isEmpty && bytes.count <= 32768 else { throw TheatreCredentialError.corrupt }
        let values: [String: Any] = [kSecValueData as String: bytes,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly]
        let updated = SecItemUpdate(query as CFDictionary, values as CFDictionary)
        if updated == errSecItemNotFound {
            var request = query
            values.forEach { request[$0.key] = $0.value }
            guard SecItemAdd(request as CFDictionary, nil) == errSecSuccess else { throw TheatreCredentialError.unavailable }
        } else if updated != errSecSuccess { throw TheatreCredentialError.unavailable }
    }
    func clear() throws {
        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else { throw TheatreCredentialError.unavailable }
    }
}

enum TheatreIdentityCrypto {
    static func secret() throws -> String {
        var bytes = [UInt8](repeating: 0, count: 32)
        guard SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess else { throw TheatreCredentialError.unavailable }
        return base64url(Data(bytes))
    }
    static func challenge(_ verifier: String) -> String { base64url(Data(SHA256.hash(data: Data(verifier.utf8)))) }
    private static func base64url(_ value: Data) -> String { value.base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "") }
}

/// System authentication browser, retained by one owner. Cancel invalidates late callbacks.
@MainActor final class TheatreSystemBrowser: NSObject, ASWebAuthenticationPresentationContextProviding {
    private var session: ASWebAuthenticationSession?
    private var generation: UInt64 = 0
    private var exhausted = false
    private let anchor: ASPresentationAnchor
    init(anchor: ASPresentationAnchor) { self.anchor = anchor }
    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor { anchor }
    func cancel() {
        if generation == UInt64.max { exhausted = true } else { generation += 1 }
        session?.cancel(); session = nil
    }
    func open(_ url: URL, completion: @escaping (URL?) -> Void) -> Bool {
        cancel()
        guard url.scheme == "https", !exhausted, generation != UInt64.max else { return false }
        let issued = generation
        let next = ASWebAuthenticationSession(url: url, callbackURLScheme: "com.cantos.theatre") { [weak self] callback, _ in
            Task { @MainActor in
                guard let self, self.generation == issued else { return }
                self.session = nil
                completion(callback)
            }
        }
        next.presentationContextProvider = self
        session = next
        if !next.start() { session = nil; return false }
        return true
    }
}

struct TheatreNativeCallback {
    let state: String
    let code: String
    let hasError: Bool
    static func parse(_ url: URL) -> Self? {
        guard let parts = URLComponents(url: url, resolvingAgainstBaseURL: false), parts.fragment == nil,
              url.absoluteString.components(separatedBy: "?").first == "com.cantos.theatre:/oauth2/callback" else { return nil }
        var pairs: [String: String] = [:]
        for item in parts.queryItems ?? [] {
            guard ["state", "code", "error", "error_description"].contains(item.name), pairs[item.name] == nil, let value = item.value else { return nil }
            pairs[item.name] = value
        }
        guard let state = pairs["state"] else { return nil }
        return Self(state: state, code: pairs["code"] ?? "", hasError: pairs["error"] != nil)
    }
}
