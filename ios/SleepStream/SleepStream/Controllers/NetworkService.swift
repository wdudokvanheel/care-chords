import Combine
import Foundation

struct ActionRequestDto: Encodable {
    let action: String
}

struct PlaybackRequestDto: Encodable {
    let uri: String
}

struct PlayRefRequestDto: Encodable {
    let ref: String
}

struct SleepTimerRequestDto: Encodable {
    let timer: Int
}

struct ShuffleRequestDto: Encodable {
    let shuffle: Bool
}

enum HTTPMethod: String {
    case GET
    case POST
    case DELETE
}

class NetworkService {
    static func get(_ url: String) -> AnyPublisher<Data, URLError> {
        sendRequest(with: Optional<EmptyBody>.none, to: url, method: .GET)
    }

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
        if method == .GET {
            request.cachePolicy = .reloadIgnoringLocalAndRemoteCacheData
            request.setValue("no-cache", forHTTPHeaderField: "Cache-Control")
            request.setValue("no-cache", forHTTPHeaderField: "Pragma")
        }

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

struct QueueItemRequestDto: Encodable {
    let source: String
    let kind: String
    let ref: String
    let title: String
}

struct ReorderQueueRequestDto: Encodable {
    let fromIndex: Int
    let toIndex: Int

    enum CodingKeys: String, CodingKey {
        case fromIndex = "from_index"
        case toIndex = "to_index"
    }
}

final class QueueController: ObservableObject {
    @Published var state: QueueState = .empty
    @Published var isLoading: Bool = false
    @Published var errorMessage: String? = nil

    private let decoder: JSONDecoder = {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return decoder
    }()
    private var cancellables: Set<AnyCancellable> = []

    var items: [QueueItem] {
        state.items
    }

    func load() {
        isLoading = true
        requestQueue(
            NetworkService.get(queueURL)
                .decode(type: QueueState.self, decoder: decoder)
                .mapError(asUrlError)
                .eraseToAnyPublisher()
        )
    }

    func enqueue(_ item: AudioItem) {
        let request = QueueItemRequestDto(
            source: item.source,
            kind: item.kind.rawValue,
            ref: item.reference,
            title: item.name
        )
        requestQueue(
            NetworkService.sendRequest(with: request, to: "\(queueURL)/items", method: .POST)
                .decode(type: QueueState.self, decoder: decoder)
                .mapError(asUrlError)
                .eraseToAnyPublisher()
        )
    }

    func play(index: Int) {
        requestQueue(
            NetworkService.sendRequest(
                with: Optional<EmptyBody>.none,
                to: "\(queueURL)/play-index/\(index)",
                method: .POST
            )
            .decode(type: QueueState.self, decoder: decoder)
            .mapError(asUrlError)
            .eraseToAnyPublisher()
        )
    }

    func remove(_ item: QueueItem) {
        requestQueue(
            NetworkService.sendRequest(
                with: Optional<EmptyBody>.none,
                to: "\(queueURL)/items/\(item.id)",
                method: .DELETE
            )
            .decode(type: QueueState.self, decoder: decoder)
            .mapError(asUrlError)
            .eraseToAnyPublisher()
        )
    }

    func clear() {
        requestQueue(
            NetworkService.sendRequest(
                with: Optional<EmptyBody>.none,
                to: queueURL,
                method: .DELETE
            )
            .decode(type: QueueState.self, decoder: decoder)
            .mapError(asUrlError)
            .eraseToAnyPublisher()
        )
    }

    func moveUp(_ item: QueueItem) {
        guard let index = items.firstIndex(where: { $0.id == item.id }), index > 0 else { return }
        move(from: index, to: index - 1)
    }

    func moveDown(_ item: QueueItem) {
        guard let index = items.firstIndex(where: { $0.id == item.id }), index + 1 < items.count else { return }
        move(from: index, to: index + 1)
    }

    private func move(from: Int, to: Int) {
        let request = ReorderQueueRequestDto(fromIndex: from, toIndex: to)
        requestQueue(
            NetworkService.sendRequest(with: request, to: "\(queueURL)/reorder", method: .POST)
                .decode(type: QueueState.self, decoder: decoder)
                .mapError(asUrlError)
                .eraseToAnyPublisher()
        )
    }

    private var queueURL: String {
        "http://\(ServerConfig.shared.getURL()):7755/queue"
    }

    private func requestQueue(_ publisher: AnyPublisher<QueueState, URLError>) {
        errorMessage = nil
        publisher
            .sink { [weak self] completion in
                self?.isLoading = false
                if case .failure(let error) = completion {
                    self?.errorMessage = error.localizedDescription
                }
            } receiveValue: { [weak self] state in
                self?.state = state
            }
            .store(in: &cancellables)
    }
}

private func asUrlError(_ error: Error) -> URLError {
    error as? URLError ?? URLError(.cannotParseResponse)
}
