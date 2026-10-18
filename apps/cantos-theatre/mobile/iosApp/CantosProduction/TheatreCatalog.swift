import Foundation
import CantosTheatre

/// Public GET transport. ATS and platform TLS trust remain enabled, including in Debug.
final class TheatreCatalogPlatform: IosCatalogPlatform {
    private let apiOrigin: String
    init(apiOrigin: String) { self.apiOrigin = apiOrigin }

    func get(request: CatalogHttpRequest, completion: @escaping (IosCatalogHttpResponse) -> KotlinUnit) -> IosCatalogRequestHandle {
        TheatreCatalogCall(request: request, apiOrigin: apiOrigin, completion: completion)
    }
}

/// One delegate queue owns response bytes. Cancellation fences completion and tears down the
/// task/session; the shared controller additionally rejects stale request generations.
private final class TheatreCatalogCall: NSObject, URLSessionDataDelegate, IosCatalogRequestHandle {
    private var session: URLSession?
    private var task: URLSessionDataTask?
    private var bytes = Data()
    private var response: HTTPURLResponse?
    private var tooLarge = false
    private var cancelled = false
    private let lock = NSLock()
    private let limit: Int
    private let completion: (IosCatalogHttpResponse) -> KotlinUnit

    init(request: CatalogHttpRequest, apiOrigin: String, completion: @escaping (IosCatalogHttpResponse) -> KotlinUnit) {
        self.completion = completion
        limit = Int(request.maxBodyBytes)
        super.init()
        guard let url = URL(string: request.url), let origin = URL(string: apiOrigin),
              origin.scheme == "https", url.scheme == "https",
              url.host == origin.host, url.port == origin.port,
              url.user == nil, url.password == nil, url.fragment == nil,
              limit > 0 else {
            _ = completion(IosCatalogHttpResponse(status: 0, contentType: nil, body: nil, failure: .transport))
            return
        }
        let config = URLSessionConfiguration.ephemeral
        config.httpCookieStorage = nil
        config.httpShouldSetCookies = false
        config.urlCredentialStorage = nil
        config.urlCache = nil
        config.requestCachePolicy = .reloadIgnoringLocalCacheData
        config.timeoutIntervalForRequest = 10
        config.timeoutIntervalForResource = 25
        let ownedSession = URLSession(configuration: config, delegate: self, delegateQueue: nil)
        session = ownedSession
        var nativeRequest = URLRequest(url: url)
        nativeRequest.httpMethod = "GET"
        nativeRequest.setValue("application/json", forHTTPHeaderField: "Accept")
        task = ownedSession.dataTask(with: nativeRequest)
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
        guard response.expectedContentLength <= Int64(limit) else {
            tooLarge = true
            completionHandler(.cancel)
            return
        }
        completionHandler(.allow)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        guard data.count <= limit - bytes.count else {
            tooLarge = true
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
        let failure: IosCatalogHttpFailure? = tooLarge ? .tooLarge : (error == nil ? nil : .transport)
        // Copy exact bytes, preserving malformed UTF-8 for the common strict decoder to refuse.
        let body = KotlinByteArray(size: Int32(bytes.count))
        for (index, byte) in bytes.enumerated() { body.set(index: Int32(index), value: Int8(bitPattern: byte)) }
        let result = IosCatalogHttpResponse(status: Int32(response?.statusCode ?? 0),
            contentType: response?.value(forHTTPHeaderField: "Content-Type"), body: body, failure: failure)
        DispatchQueue.main.async {
            self.lock.lock()
            let stillActive = !self.cancelled
            self.lock.unlock()
            if stillActive { _ = self.completion(result) }
        }
    }
}
