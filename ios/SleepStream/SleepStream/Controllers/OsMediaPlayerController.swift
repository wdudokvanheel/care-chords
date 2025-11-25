import Combine
import MediaPlayer
import UIKit
import os

enum OsMediaPlayerEvent {
    case play
    case pause
    case toggle
    case next
    case prev
    case setShuffle(Bool)
}

class OsMediaPlayerController: ObservableObject {
    let logger = Logger.new("OsMediaPlayerController")
    
    // Publishes media control events from the iOS lockscreen or the 'Now Playing' widget
    let events = PassthroughSubject<OsMediaPlayerEvent, Never>()

    init() {
        setupRemoteCommandCenter()
    }

    func updateNowPlayingMetaData(_ musicStatus: PlayerStatus, _ gstreamer: AudioState) {
        print(musicStatus)
        print(gstreamer.description)

        var nowPlayingInfo = [String: Any]()

        if gstreamer == .playing {
            if musicStatus.status == .playing, let metadata = musicStatus.metadata {
                nowPlayingInfo[MPMediaItemPropertyTitle] = metadata.title
                nowPlayingInfo[MPMediaItemPropertyArtist] = "Monitor & \(metadata.artist)"
            }
            else {
                nowPlayingInfo[MPMediaItemPropertyTitle] = "No music"
                nowPlayingInfo[MPMediaItemPropertyArtist] = "Monitor only"
            }

            nowPlayingInfo[MPNowPlayingInfoPropertyPlaybackRate] = 1.0
        }
        else {
            nowPlayingInfo[MPMediaItemPropertyArtist] = "Stopped"
            nowPlayingInfo[MPNowPlayingInfoPropertyPlaybackRate] = 1.0
        }
        MPNowPlayingInfoCenter.default().nowPlayingInfo = nowPlayingInfo
    }
    
    func updateShuffleState(_ shuffle: Bool) {
        MPRemoteCommandCenter.shared().changeShuffleModeCommand.currentShuffleType = shuffle ? .items : .off
    }

    private func setupRemoteCommandCenter() {
        // Start receiving remote control events
        UIApplication.shared.beginReceivingRemoteControlEvents()
        let commandCenter = MPRemoteCommandCenter.shared()

        // Handle Play Command
        commandCenter.playCommand.addTarget { [weak self] _ in
            self?.events.send(.play)
            return .success
        }

        // Handle Pause Command
        commandCenter.pauseCommand.addTarget { [weak self] _ in
            self?.events.send(.pause)
            return .commandFailed
        }

        // Handle Toggle Play/Pause Command
        commandCenter.togglePlayPauseCommand.addTarget { [weak self] _ in
            self?.events.send(.toggle)
            return .success
        }

        // Handle Next Track Command
        commandCenter.nextTrackCommand.addTarget { [weak self] _ in
            self?.events.send(.next)
            return .success
        }

        // Handle Previous Track Command
        commandCenter.previousTrackCommand.addTarget { [weak self] _ in
            self?.events.send(.prev)
            return .success
        }
        
        // Handle Shuffle Command
        commandCenter.changeShuffleModeCommand.isEnabled = true
        commandCenter.changeShuffleModeCommand.addTarget { [weak self] event in
            guard let self = self, let event = event as? MPChangeShuffleModeCommandEvent else { return .commandFailed }
            let shuffle = event.preservesShuffleMode
            self.events.send(.setShuffle(shuffle))
            return .success
        }
    }
}
