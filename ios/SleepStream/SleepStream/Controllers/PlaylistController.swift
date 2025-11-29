import Combine
import Foundation

/// Lightweight playlist provider that pulls data from the backend.
final class PlaylistController: ObservableObject {
    @Published var playlists: [Playlist] = []
    @Published var isLoading: Bool = false
    @Published var errorMessage: String? = nil

    private var cancellables: Set<AnyCancellable> = []
    private var hasLoaded = false

    func loadPlaylists() {
        guard !isLoading, !hasLoaded else { return }
        isLoading = true
        errorMessage = nil

        let url = "http://\(SleepStreamApp.SERVER):7755/playlists"
        NetworkService.get(url)
            .decode(type: [BackendPlaylist].self, decoder: backendDecoder)
            .map { backend in
                backend
                    .filter { $0.name.lowercased().contains("sleep") }
                    .map { item in
                        Playlist(
                            item.name,
                            item.uri,
                            item.imageURL,
                            folder: item.folder
                        )
                    }
            }
            .sink { [weak self] completion in
                guard let self else { return }
                self.isLoading = false
                switch completion {
                case .failure(let error):
                    self.errorMessage = error.localizedDescription
                case .finished:
                    self.hasLoaded = true
                }
            } receiveValue: { [weak self] playlists in
                self?.playlists = playlists
            }
            .store(in: &cancellables)
    }
}

private let backendDecoder: JSONDecoder = {
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    return decoder
}()

private struct BackendPlaylist: Decodable {
    let uri: String
    let name: String
    let imageUri: String?
    let folder: String?

    var imageURL: URL? {
        guard let imageUri, let url = URL(string: imageUri) else { return nil }
        return url
    }
}
