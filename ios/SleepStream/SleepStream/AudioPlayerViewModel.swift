import AVKit
import Combine

class AudioPlayerViewModel: ObservableObject {
    @Published var music: MusicController = .init()
    @Published var controller: AudioOutputController = .init()
    @Published var gstreamer: GStreamerController = .init()

    let spotify: SpotifyController

    private var cancellables = Set<AnyCancellable>()

    init(spotify: SpotifyController) {
        self.spotify = spotify

        controller.$currentOutput
            .sink(receiveValue: onOutputChange)
            .store(in: &cancellables)
    }

    func onOutputChange(port: AVAudioSession.Port) {
        print("Output changed to \(port) XX")
        switch port {
        case .builtInSpeaker:
            gstreamer.pause()
        case .bluetoothA2DP, .bluetoothLE, .bluetoothHFP:
            gstreamer.play()
        default:
            break
        }
    }

    func toggleOutput() {
        switch gstreamer.state {
        case .playing:
            gstreamer.pause()
        case .paused:
            gstreamer.play()
        case .initializing:
            break
        case .ready:
            gstreamer.play()
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
