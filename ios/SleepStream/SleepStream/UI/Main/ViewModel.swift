import AVKit
import Combine
import MediaPlayer
import os

class ViewModel: ObservableObject {
    private let logger = Logger.new("ViewModel")

    @Published var music: MusicController
    @Published var audioOutput: AudioOutputController
    @Published var gstreamer: GStreamerController
    @Published var video: LiveStreamController = .init()
    @Published var nowPlaying: NowPlayingMediator

    let spotify: SpotifyController

    private var cancellables = Set<AnyCancellable>()

    init(spotify: SpotifyController) {
        let music = MusicController()
        let audio = AudioOutputController()
        let gstreamer = GStreamerController()
        let osMediaPlayer = OsMediaPlayerController()
        let nowPlaying = NowPlayingMediator(audioOutput: audio, gstreamer: gstreamer, musicController: music, osMediaPlayer: osMediaPlayer)

        self.spotify = spotify
        self.music = music
        self.audioOutput = audio
        self.gstreamer = gstreamer
        self.nowPlaying = nowPlaying

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
        case .stopped:
            gstreamer.play()
        case .ready:
            gstreamer.play()
        }
    }

    func startSleepTimer(seconds: Int) {
        music.startSleepTimer(seconds)
    }

    func setShuffle(shuffle: Bool) {
        music.setShuffle(shuffle)
    }

    func selectPlaylist(playlist: Playlist) {
        let request = PlaybackRequestDto(uri: playlist.uri)
        NetworkService.sendRequest(with: request, to: "http://\(SleepStreamApp.SERVER):7755/playlist", method: .POST).sink(receiveCompletion: { completion in
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
