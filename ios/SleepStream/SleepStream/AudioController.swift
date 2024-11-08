import AVKit
import Foundation
import os
import SwiftUI

class AudioController: GStreamerBackendDelegate, ObservableObject {
    @Published var currentOutput: String = "Unknown"
    @Published var state: AudioState = .initializing
    @Published var backendMessage: String = ""
    
    @Published var pauseOnSpeaker = true

    private var gstBackend: GStreamerAudioBackend?

    init() {
        print("Starting audio")
        self.gstBackend = GStreamerAudioBackend(self)
        configureAudioSession()

        let queue = DispatchQueue(label: "gstreamer_audio_queue")
        queue.async {
            self.gstBackend?.run_app_pipeline_threaded()
        }
    }

    func pause() {
        gstBackend?.pause()
    }

    func play() {
        gstBackend?.play()
    }

    private func configureAudioSession() {
        do {
            try AVAudioSession.sharedInstance().setCategory(.playback, mode: .default, options: [])
            try AVAudioSession.sharedInstance().setActive(true)
        } catch {
            print("Failed to configure audio session:", error)
        }
    }

    func gStreamerInitialized() {
        print("Init AUDIO complete")
    }

    func gstreamerMessage(message: String) {
        DispatchQueue.main.async {
            print("Got message: \(message)")
            self.backendMessage = message
        }
    }

    func startMonitoringAudioRoute() {
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(audioRouteChanged),
            name: AVAudioSession.routeChangeNotification,
            object: nil
        )
        updateCurrentOutput()
    }

    func gstreamerAudioState(state newstate: AudioState) {
        DispatchQueue.main.async {
            self.state = newstate
        }
    }

    func stopMonitoringAudioRoute() {
        NotificationCenter.default.removeObserver(self, name: AVAudioSession.routeChangeNotification, object: nil)
    }

    @objc private func audioRouteChanged(notification: Notification) {
        updateCurrentOutput()
    }

    private func updateCurrentOutput() {
        let audioSession = AVAudioSession.sharedInstance()
        if let output = audioSession.currentRoute.outputs.first {
            updatePlaybackStatus(output.portType)

            switch output.portType {
            case .builtInSpeaker:
                currentOutput = "\(output.portName)"
            case .bluetoothA2DP, .bluetoothLE, .bluetoothHFP:
                currentOutput = "\(output.portName)"
            case .airPlay:
                currentOutput = "AirPlay"
            case .headphones, .headsetMic:
                currentOutput = "Headphones"
            case .usbAudio:
                currentOutput = "USB Audio Device"
            default:
                currentOutput = output.portName
            }
        } else {
            currentOutput = "No Output"
        }
    }

    private func updatePlaybackStatus(_ port: AVAudioSession.Port) {
        if port == .builtInSpeaker, state == .playing, pauseOnSpeaker {
            pause()
        }
        if port == .bluetoothA2DP, pauseOnSpeaker {
            play()
        }
    }
}
