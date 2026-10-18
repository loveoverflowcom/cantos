import Foundation
import CantosTheatre

/// Anonymous catalog reads only. HTTPS/trust, cookies, cache and redirects stay explicit.
final class CatalogPlatform: NSObject, IosCatalogPlatform {
    private let apiOrigin: String
    init(apiOrigin: String) { self.apiOrigin = apiOrigin; super.init() }
    func sendCatalog(request: CatalogHttpRequest, completion: @escaping (IosCatalogReply) -> KotlinUnit) -> NativeRequestHandle {
        CatalogCall(request: request, apiOrigin: apiOrigin, completion: completion)
    }
}

private final class CatalogCall: NSObject, URLSessionDataDelegate, NativeRequestHandle {
    private var session: URLSession?
    private var task: URLSessionDataTask?
    private var bytes = Data()
    private var response: HTTPURLResponse?
    private var oversized = false
    private var cancelled = false
    private let lock = NSLock()
    private let bound: Int
    private let completion: (IosCatalogReply) -> KotlinUnit

    init(request: CatalogHttpRequest, apiOrigin: String, completion: @escaping (IosCatalogReply) -> KotlinUnit) {
        self.completion = completion
        bound = Int(request.maxBodyBytes)
        super.init()
        guard let url = URL(string: request.url), let origin = URL(string: apiOrigin),
              url.scheme == "https", origin.scheme == "https",
              url.host == origin.host, url.port == origin.port,
              url.user == nil, url.password == nil, url.fragment == nil,
              url.path.hasPrefix("/api/v1/theatre/"), bound > 0 else {
            _ = completion(IosCatalogReply(status: 0, contentType: nil, body: nil, failure: "transport"))
            return
        }
        let config = URLSessionConfiguration.ephemeral
        config.httpCookieStorage = nil
        config.httpShouldSetCookies = false
        config.urlCache = nil
        config.urlCredentialStorage = nil
        config.requestCachePolicy = .reloadIgnoringLocalCacheData
        config.timeoutIntervalForRequest = 15
        config.timeoutIntervalForResource = 30
        let owned = URLSession(configuration: config, delegate: self, delegateQueue: nil)
        session = owned
        var nativeRequest = URLRequest(url: url)
        nativeRequest.httpMethod = "GET"
        nativeRequest.setValue("application/json", forHTTPHeaderField: "Accept")
        task = owned.dataTask(with: nativeRequest)
        task?.resume()
    }

    func cancel() {
        lock.lock()
        cancelled = true
        let activeTask = task
        let activeSession = session
        lock.unlock()
        activeTask?.cancel()
        activeSession?.invalidateAndCancel()
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        completionHandler(nil)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse,
                    completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        self.response = response as? HTTPURLResponse
        guard response.expectedContentLength <= Int64(bound) else {
            oversized = true
            completionHandler(.cancel)
            return
        }
        completionHandler(.allow)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        guard data.count <= bound - bytes.count else {
            oversized = true
            dataTask.cancel()
            return
        }
        bytes.append(data)
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        lock.lock()
        let wasCancelled = cancelled
        self.session = nil
        self.task = nil
        lock.unlock()
        defer { session.finishTasksAndInvalidate() }
        guard !wasCancelled else { return }
        let body = String(data: bytes, encoding: .utf8)
        let failure = oversized ? "oversized" : (error != nil ? "transport" : (body == nil ? "malformed" : nil))
        let result = IosCatalogReply(status: Int32(response?.statusCode ?? 0),
            contentType: response?.value(forHTTPHeaderField: "Content-Type"), body: body, failure: failure)
        DispatchQueue.main.async { _ = self.completion(result) }
    }
}
