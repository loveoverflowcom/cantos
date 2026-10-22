import Foundation
import Security
import CantosTheatre
import Darwin

enum TheatreCredentialError: Error { case unavailable, denied, corrupt }

/// Native session credential only. No UserDefaults/file fallback.
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
        // Read the one complete item back before the shared owner admits a rotated pair.
        guard try read() == value else { throw TheatreCredentialError.corrupt }
    }
    func clear() throws {
        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else { throw TheatreCredentialError.unavailable }
        guard try read() == nil else { throw TheatreCredentialError.unavailable }
    }
}

/// Nonsecret intent, written before the common owner's first logout effect.
final class TheatreLogoutBarrier {
    private let directory: URL
    private let marker: URL
    init() throws {
        directory = try FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
            .appendingPathComponent("CantosIdentity", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        var protectedDirectory = directory
        var values = URLResourceValues(); values.isExcludedFromBackup = true
        try protectedDirectory.setResourceValues(values)
        marker = directory.appendingPathComponent("logout-pending-v1")
    }
    func read() -> String {
        if FileManager.default.fileExists(atPath: marker.path) { return "pending" }
        return FileManager.default.isWritableFile(atPath: directory.path) ? "clear" : "unavailable"
    }
    private func syncDirectory() throws {
        let fd = Darwin.open(directory.path, O_RDONLY)
        guard fd >= 0 else { throw TheatreCredentialError.unavailable }
        defer { Darwin.close(fd) }
        guard Darwin.fsync(fd) == 0 else { throw TheatreCredentialError.unavailable }
    }
    func mark() throws {
        try Data([1]).write(to: marker, options: .atomic)
        let file = try FileHandle(forWritingTo: marker)
        defer { try? file.close() }
        try file.synchronize()
        try syncDirectory()
    }
    func clear() throws {
        if FileManager.default.fileExists(atPath: marker.path) { try FileManager.default.removeItem(at: marker) }
        try syncDirectory()
    }
}

/// Bounded URLSession delegate: no cookies/cache/redirects or customized TLS trust.
private final class TheatreHttpCall: NSObject, URLSessionDataDelegate, NativeRequestHandle {
    private var session: URLSession?
    private var task: URLSessionDataTask?
    private var bytes = Data()
    private var response: HTTPURLResponse?
    private var protocolFailure = false
    private var cancelled = false
    private let lock = NSLock()
    private let completion: (NativeHttpResponse) -> KotlinUnit
    init(request: NativeHttpRequest, apiOrigin: String, completion: @escaping (NativeHttpResponse) -> KotlinUnit) {
        self.completion = completion
        super.init()
        guard let url = URL(string: request.url), let origin = URL(string: apiOrigin),
              url.scheme == "https", url.host == origin.host, url.port == origin.port,
              url.user == nil, url.password == nil, url.fragment == nil,
              ["GET", "POST", "PUT", "DELETE"].contains(request.method) else {
            _ = completion(NativeHttpResponse(status: 0, contentType: nil, body: nil, failure: "configuration", actualSession: nil, retryAfter: nil)); return
        }
        let config = URLSessionConfiguration.ephemeral
        config.httpCookieStorage = nil; config.httpShouldSetCookies = false; config.urlCache = nil
        config.requestCachePolicy = .reloadIgnoringLocalCacheData
        config.timeoutIntervalForRequest = 5; config.timeoutIntervalForResource = 15
        let ownedSession = URLSession(configuration: config, delegate: self, delegateQueue: nil)
        session = ownedSession
        var nativeRequest = URLRequest(url: url)
        nativeRequest.httpMethod = request.method
        for (name, value) in request.headers { nativeRequest.setValue(value, forHTTPHeaderField: name) }
        if let body = request.body {
            nativeRequest.httpBody = Data(body.utf8)
        }
        task = ownedSession.dataTask(with: nativeRequest); task?.resume()
    }
    func cancel() {
        lock.lock(); cancelled = true; let activeTask = task; let activeSession = session; lock.unlock()
        activeTask?.cancel(); activeSession?.invalidateAndCancel()
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) { completionHandler(nil) }
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse, completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        self.response = response as? HTTPURLResponse
        guard response.expectedContentLength <= 65536 else { protocolFailure = true; completionHandler(.cancel); return }
        completionHandler(.allow)
    }
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        if bytes.count + data.count > 65536 { protocolFailure = true; dataTask.cancel(); return }
        bytes.append(data)
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        lock.lock(); let wasCancelled = cancelled; self.session = nil; self.task = nil; lock.unlock()
        defer { session.finishTasksAndInvalidate() }
        guard !wasCancelled else { return }
        let body = String(data: bytes, encoding: .utf8)
        let failure = protocolFailure || body == nil ? "protocol" : (error == nil ? nil : "transport")
        let result = NativeHttpResponse(status: Int32(response?.statusCode ?? 0), contentType: response?.value(forHTTPHeaderField: "Content-Type"), body: body, failure: failure, actualSession: response?.value(forHTTPHeaderField: "X-Cantos-Theatre-Session"), retryAfter: response?.value(forHTTPHeaderField: "Retry-After"))
        DispatchQueue.main.async { _ = self.completion(result) }
    }
}

/// Swift boundary is injected into one shared CMP owner; foreign errors become closed outcomes.
/// Identity v1 is explicit email/password over this HTTPS transport: there is no browser,
/// callback or URL-scheme authority, and Keychain holds the only credential copy.
@MainActor final class TheatreNativePlatform: NSObject, IosIdentityPlatform {
    private let apiOrigin: String
    private let credentials = TheatreCredentialStore()
    private lazy var barrier = try? TheatreLogoutBarrier()
    init(apiOrigin: String) { self.apiOrigin = apiOrigin; super.init() }
    private func failure(_ error: Error) -> String {
        switch error {
        case TheatreCredentialError.denied: return "denied"
        case TheatreCredentialError.corrupt: return "corrupt"
        default: return "unavailable"
        }
    }
    func readCredential() -> IosStoreRead {
        do { return IosStoreRead(value: try credentials.read(), failure: nil) }
        catch { return IosStoreRead(value: nil, failure: failure(error)) }
    }
    func writeCredential(value: String) -> String? { do { try credentials.write(value); return nil } catch { return failure(error) } }
    func clearCredential() -> String? { do { try credentials.clear(); return nil } catch { return failure(error) } }
    func readLogoutBarrier() -> String { barrier?.read() ?? "unavailable" }
    func markLogoutBarrier() -> String? { guard let barrier else { return "unavailable" }; do { try barrier.mark(); return nil } catch { return failure(error) } }
    func clearLogoutBarrier() -> String? { guard let barrier else { return "unavailable" }; do { try barrier.clear(); return nil } catch { return failure(error) } }
    func send(request: NativeHttpRequest, completion: @escaping (NativeHttpResponse) -> KotlinUnit) -> NativeRequestHandle {
        TheatreHttpCall(request: request, apiOrigin: apiOrigin, completion: completion)
    }
}
