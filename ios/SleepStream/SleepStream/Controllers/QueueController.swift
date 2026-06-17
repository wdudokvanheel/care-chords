import Combine
import Foundation

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
