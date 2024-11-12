import Combine

class AudioPlayerViewModel: ObservableObject {
    let spotify: Spotify
    
    @Published var music: MusicController = .init()
    @Published var controller: AudioController = .init()

    private var cancellables = Set<AnyCancellable>()

    init(spotify: Spotify){
        self.spotify = spotify
    }
    
    func toggleOutput() {
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
        let request = PlaybackRequestDto(uri: playlist.uri)
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
