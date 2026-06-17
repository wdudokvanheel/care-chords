import Combine
import Foundation

final class AudioLibraryController: ObservableObject {
    @Published var sources: [AudioSourceStatus] = []
    @Published var localItems: [AudioItem] = []
    @Published var spotifyPlaylists: [AudioItem] = []
    @Published var systemPlaylists: [AudioItem] = []
    @Published var localPath: String? = nil
    @Published var isLoading: Bool = false
    @Published var errorMessage: String? = nil

    private var cancellables: Set<AnyCancellable> = []

    var spotifyAvailable: Bool {
        sources.first { $0.id == "spotify" }?.available == true
    }

    func load() {
        guard !isLoading else { return }
        isLoading = true
        errorMessage = nil

        Publishers.Zip3(loadSources(), loadSystemPlaylists(), loadLocal(path: localPath))
            .sink { [weak self] completion in
                guard let self else { return }
                self.isLoading = false
                if case .failure(let error) = completion {
                    self.errorMessage = error.localizedDescription
                }
            } receiveValue: { [weak self] sources, systemPlaylists, localItems in
                guard let self else { return }
                self.sources = sources
                self.systemPlaylists = systemPlaylists
                self.localItems = localItems
                if sources.first(where: { $0.id == "spotify" })?.available == true {
                    self.loadSpotifyPlaylists()
                } else {
                    self.spotifyPlaylists = []
                }
            }
            .store(in: &cancellables)
    }

    func refresh() {
        cancellables.removeAll()
        load()
    }

    func openLocalFolder(_ item: AudioItem) {
        localPath = item.reference
        load()
    }

    func openLocalRoot() {
        localPath = nil
        load()
    }

    func loadPlaylists() {
        load()
    }

    private func loadSources() -> AnyPublisher<[AudioSourceStatus], URLError> {
        let url = "http://\(ServerConfig.shared.getURL()):7755/sources"
        return NetworkService.get(url)
            .decode(type: [AudioSourceStatus].self, decoder: backendDecoder)
            .mapError(asUrlError)
            .eraseToAnyPublisher()
    }

    private func loadSystemPlaylists() -> AnyPublisher<[AudioItem], URLError> {
        let url = "http://\(ServerConfig.shared.getURL()):7755/system-playlists"
        return NetworkService.get(url)
            .decode(type: [BackendSystemPlaylist].self, decoder: backendDecoder)
            .map { playlists in
                playlists.map {
                    AudioItem(
                        id: $0.id,
                        name: $0.name,
                        reference: "system:playlist:\($0.id)",
                        source: "system",
                        kind: .playlist
                    )
                }
            }
            .mapError(asUrlError)
            .eraseToAnyPublisher()
    }

    private func loadLocal(path: String?) -> AnyPublisher<[AudioItem], URLError> {
        var url = "http://\(ServerConfig.shared.getURL()):7755/library/local"
        if let path, let encoded = path.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) {
            url += "?path=\(encoded)"
        }

        return NetworkService.get(url)
            .decode(type: [BackendLocalAudioEntry].self, decoder: backendDecoder)
            .map { entries in
                entries.map {
                    let kind: AudioItemKind = $0.kind == "folder" ? .folder : .file
                    let prefix = kind == .folder ? "local:folder:" : "local:file:"
                    return AudioItem(
                        id: $0.id,
                        name: $0.name,
                        reference: "\(prefix)\($0.path)",
                        source: "local",
                        kind: kind,
                        subtitle: $0.metadata?.displaySubtitle
                    )
                }
            }
            .mapError(asUrlError)
            .eraseToAnyPublisher()
    }

    private func loadSpotifyPlaylists() {
        let url = "http://\(ServerConfig.shared.getURL()):7755/playlists"
        NetworkService.get(url)
            .decode(type: [BackendPlaylist].self, decoder: backendDecoder)
            .map { backend in
                backend
                    .filter { $0.name.lowercased().contains("sleep") }
                    .map { item in
                        AudioItem(
                            id: item.uri,
                            name: item.name,
                            reference: item.uri,
                            source: "spotify",
                            kind: .playlist,
                            image: item.imageURL,
                            folder: item.folder
                        )
                    }
            }
            .sink { [weak self] completion in
                if case .failure(let error) = completion {
                    self?.errorMessage = error.localizedDescription
                }
            } receiveValue: { [weak self] playlists in
                self?.spotifyPlaylists = playlists
            }
            .store(in: &cancellables)
    }
}

private let backendDecoder: JSONDecoder = {
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    return decoder
}()

private func asUrlError(_ error: Error) -> URLError {
    error as? URLError ?? URLError(.cannotParseResponse)
}

struct AudioSourceStatus: Decodable {
    let id: String
    let name: String
    let available: Bool
    let reason: String?
}

private struct BackendLocalAudioEntry: Decodable {
    let id: String
    let name: String
    let kind: String
    let path: String
    let metadata: BackendLocalAudioMetadata?
}

private struct BackendLocalAudioMetadata: Decodable {
    let title: String?
    let artist: String?
    let album: String?
    let albumArtist: String?
    let trackNumber: String?
    let discNumber: String?
    let date: String?
    let genre: String?

    var displaySubtitle: String? {
        let primaryArtist = artist ?? albumArtist

        if let primaryArtist, let album {
            return "\(primaryArtist) - \(album)"
        }

        return primaryArtist ?? album ?? genre ?? date
    }
}

private struct BackendSystemPlaylist: Decodable {
    let id: String
    let name: String
}

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
