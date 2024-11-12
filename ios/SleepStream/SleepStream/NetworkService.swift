import Combine

enum HTTPMethod: String {
    case GET
    case POST
}

class NetworkService {
    static func sendRequest<T: Encodable>(
        with object: T?,
        to url: String,
        method: HTTPMethod
    ) -> AnyPublisher<Data, URLError> {
        guard let url = URL(string: url) else {
            return Fail(error: URLError(.requestBodyStreamExhausted)).eraseToAnyPublisher()
        }

        var request = URLRequest(url: url)
        request.httpMethod = method.rawValue
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        if let object = object, method == .POST {
            do {
                let jsonData = try JSONEncoder().encode(object)
                request.httpBody = jsonData
            } catch {
                return Fail(error: URLError(.requestBodyStreamExhausted)).eraseToAnyPublisher()
            }
        }

        return URLSession.shared.dataTaskPublisher(for: request)
            .map { $0.data }
            .receive(on: RunLoop.main)
            .eraseToAnyPublisher()
    }
}

// Placeholder type for empty body requests
struct EmptyBody: Encodable {}
