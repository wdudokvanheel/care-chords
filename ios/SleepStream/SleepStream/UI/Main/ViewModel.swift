import AVKit
import Combine

class ViewModel: ObservableObject {
    @Published var music: MusicController = .init()
    @Published var audioOutput: AudioOutputController = .init()
    @Published var gstreamer: GStreamerController = .init()
    @Published var video: LiveStreamController = .init()

    let spotify: SpotifyController

    private var cancellables = Set<AnyCancellable>()

    init(spotify: SpotifyController) {
        self.spotify = spotify

        audioOutput.$currentOutput
            .sink(receiveValue: onOutputChange)
            .store(in: &cancellables)
    }

    func onOutputChange(port: AVAudioSession.Port) {
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

    func startSleepTimer(seconds: Int) {
        music.startSleepTimer(seconds)
    }
    
    func setShuffle(shuffle: Bool){
        music.setShuffle(shuffle)
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
        audioOutput.startMonitoringAudioRoute()
    }

    func onDisappear() {
        audioOutput.stopMonitoringAudioRoute()
    }
}
