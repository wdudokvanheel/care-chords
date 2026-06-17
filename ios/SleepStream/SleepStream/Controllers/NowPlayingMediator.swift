import AVKit
import Combine
import os
import UIKit

/// This class mediates the communication between various input and output components
///
/// Inputs:
///     The OS can publish user input events (play/pause/etc) from the Lock Screen/'Now Playing' widget
///     The music controller publishes current song information
///     The gstreamer controller publishes the actual on-device audio playback state
///
/// Outputs:
///     OsMediaPlayer: Current status & song info to the iOS Lock Sceen / 'Now Playing' widget
///     Gstreamer: start/stop on-device audio playback
///     Music: Control the current song & playback state through the MusicController
class NowPlayingMediator: ObservableObject {
    let logger = Logger.new("NowPlayingMediator")
    let audioOutput: AudioOutputController
    let gstreamer: GStreamerController
    let musicController: MusicController
    let videoController: LiveStreamController
    let osMediaPlayer: OsMediaPlayerController

    private var cancellables = Set<AnyCancellable>()
    private var artworkTimer: DispatchSourceTimer?
    private var startedVideoForArtwork = false

    init(audioOutput: AudioOutputController, gstreamer: GStreamerController, musicController: MusicController, videoController: LiveStreamController, osMediaPlayer: OsMediaPlayerController, cancellables: Set<AnyCancellable> = Set<AnyCancellable>()) {
        self.audioOutput = audioOutput
        self.gstreamer = gstreamer
        self.musicController = musicController
        self.videoController = videoController
        self.osMediaPlayer = osMediaPlayer

        start()
    }

    deinit {
        stopNowPlayingArtworkStream()
    }

    func start() {
        musicController.$status.sink { status in
            self.osMediaPlayer.updateNowPlayingMetaData(status, self.gstreamer.state)
            self.osMediaPlayer.updateShuffleState(status.shuffle)
        }
        .store(in: &cancellables)

        gstreamer.$state.sink { state in
            self.osMediaPlayer.updateNowPlayingMetaData(self.musicController.status, state)
        }
        .store(in: &cancellables)

        osMediaPlayer.events.sink { event in
            switch event {
            case .play:

                if self.gstreamer.state == .stopped {
                    self.gstreamer.play()
                }
                else {
                    self.musicController.play()
                }
            case .pause:
                // Toggle play/pause with just the pause button
                if self.musicController.status.playing {
                    self.musicController.pause()
                }
                else {
                    self.musicController.play()
                }
            case .next:
                // When music playback is paused start playing instead
                if self.musicController.status.playing {
                    self.musicController.next()
                }
                else {
                    self.musicController.play()
                }
            case .prev:
                self.musicController.play()
                self.musicController.startSleepTimer(60 * 30)
//                self.musicController.previous()
            case .setShuffle(let shuffle):
                self.musicController.setShuffle(shuffle)
            default:
                break
            }
        }
        .store(in: &cancellables)

        NotificationCenter.default.publisher(for: UIApplication.didEnterBackgroundNotification)
            .sink { [weak self] _ in
                self?.startNowPlayingArtworkStreamIfNeeded()
            }
            .store(in: &cancellables)

        NotificationCenter.default.publisher(for: UIApplication.didBecomeActiveNotification)
            .sink { [weak self] _ in
                self?.stopNowPlayingArtworkStream()
            }
            .store(in: &cancellables)

        ServerConfig.shared.$streamVideoToNowPlaying
            .removeDuplicates()
            .sink { [weak self] enabled in
                if !enabled {
                    self?.stopNowPlayingArtworkStream()
                }
            }
            .store(in: &cancellables)
    }

    private func startNowPlayingArtworkStreamIfNeeded() {
        guard ServerConfig.shared.streamVideoToNowPlaying else {
            return
        }

        guard artworkTimer == nil else {
            return
        }

        if !videoController.isStreamPlaying {
            startedVideoForArtwork = true
            videoController.play()
        }

        let timer = DispatchSource.makeTimerSource(queue: DispatchQueue.global(qos: .utility))
        timer.schedule(deadline: .now(), repeating: .seconds(3))
        timer.setEventHandler { [weak self] in
            self?.updateNowPlayingArtworkFromVideo()
        }
        artworkTimer = timer
        timer.resume()
    }

    private func stopNowPlayingArtworkStream() {
        artworkTimer?.cancel()
        artworkTimer = nil

        if startedVideoForArtwork {
            videoController.stop()
            startedVideoForArtwork = false
        }
    }

    private func updateNowPlayingArtworkFromVideo() {
        guard let image = videoController.currentFrameImage() else {
            return
        }

        DispatchQueue.main.async {
            self.osMediaPlayer.updateNowPlayingArtwork(image)
        }
    }
}
