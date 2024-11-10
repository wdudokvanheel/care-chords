import Combine

class AudioPlayerViewModel: ObservableObject {
    @Published var music: MusicController = .init()
    @Published var controller: AudioController = .init()
    @Published var playlists: [Playlist] = [
        Playlist("CBL & Rain", "04qC7znZ4eWnTVezaEBOF7"),
        Playlist("Handpan", "0XszLZdqIrit8epvbcEe61"),
        Playlist("Fantasy & Rain", "46ZaYOSrlpvO1qjB1ezofY"),
    ]
    
    private var cancellables = Set<AnyCancellable>()

    func togglePlayState() {
        switch controller.state {
        case .playing:
            controller.pause()
        case .paused:
            controller.play()
        case .initializing:
            break
        case .ready:
            controller.play()
        }
    }

    func selectPlaylist(playlist: Playlist) {
        let request = PlaybackRequest(uri: "playlist:\(playlist.uri)")
        NetworkService.sendRequest(with: request, to: "http://10.0.0.153:7755/play", method: .POST).sink(receiveCompletion: { completion in
            switch completion {
            case .failure(let error):
                print("Error: \(error.localizedDescription)")
            case .finished:
                break
            }
        }, receiveValue: { data in
            print("Response: \(String(data: data, encoding: .utf8) ?? "Invalid response")")
        })
        .store(in: &cancellables)
    }

    func onAppear() {
        controller.startMonitoringAudioRoute()
    }

    func onDisappear() {
        controller.stopMonitoringAudioRoute()
    }
}
