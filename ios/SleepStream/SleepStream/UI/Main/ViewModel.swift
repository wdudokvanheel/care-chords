import AVKit
import Combine
import MediaPlayer
import os

class ViewModel: ObservableObject {
    private let logger = Logger.new("ViewModel")

    @Published var music: MusicController
    @Published var audioOutput: AudioOutputController
    @Published var gstreamer: GStreamerController
    @Published var video: LiveStreamController
    @Published var nowPlaying: NowPlayingMediator
    @Published var queue: QueueController

    let audioLibrary: AudioLibraryController

    private var cancellables = Set<AnyCancellable>()

    init(audioLibrary: AudioLibraryController) {
        let music = MusicController()
        let audio = AudioOutputController()
        let gstreamer = GStreamerController()
        let video = LiveStreamController()
        let queue = QueueController()
        let osMediaPlayer = OsMediaPlayerController()
        let nowPlaying = NowPlayingMediator(audioOutput: audio, gstreamer: gstreamer, musicController: music, videoController: video, osMediaPlayer: osMediaPlayer)

        self.audioLibrary = audioLibrary
        self.music = music
        self.audioOutput = audio
        self.gstreamer = gstreamer
        self.video = video
        self.queue = queue
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

    func selectAudioItem(_ item: AudioItem) {
        let request = PlayRefRequestDto(ref: item.reference)
        let serverURL = ServerConfig.shared.getURL()
        NetworkService.sendRequest(with: request, to: "http://\(serverURL):7755/queue/play-ref", method: .POST).sink(receiveCompletion: { completion in
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

    func selectPlaylist(playlist: Playlist) {
        selectAudioItem(playlist)
    }

    func onAppear() {
        audioOutput.startMonitoringAudioRoute()
        audioLibrary.load()
        queue.load()
    }

    func onDisappear() {
        audioOutput.stopMonitoringAudioRoute()
    }
}
