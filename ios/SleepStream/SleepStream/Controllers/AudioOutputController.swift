import AVKit
import Combine
import Foundation
import os
import SwiftUI

class AudioOutputController: ObservableObject {
    @Published var currentOutputDescription: String = "Unknown"
    @Published var currentOutput: AVAudioSession.Port = .virtual
    @Published var pauseOnSpeaker = true

    init() {
        configureAudioSession()
    }

    private func configureAudioSession() {
        do {
            try AVAudioSession.sharedInstance().setCategory(.playback, mode: .default, options: [])
            try AVAudioSession.sharedInstance().setActive(true)
        } catch {
            print("Failed to configure audio session:", error)
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

    func stopMonitoringAudioRoute() {
        NotificationCenter.default.removeObserver(self, name: AVAudioSession.routeChangeNotification, object: nil)
    }

    @objc private func audioRouteChanged(notification: Notification) {
        updateCurrentOutput()
    }

    private func updateCurrentOutput() {
        let audioSession = AVAudioSession.sharedInstance()
        if let output = audioSession.currentRoute.outputs.first {
            currentOutput = output.portType

            switch output.portType {
            case .builtInSpeaker:
                currentOutputDescription = "\(output.portName)"
            case .bluetoothA2DP, .bluetoothLE, .bluetoothHFP:
                currentOutputDescription = "\(output.portName)"
            case .airPlay:
                currentOutputDescription = "AirPlay"
            case .headphones, .headsetMic:
                currentOutputDescription = "Headphones"
            case .usbAudio:
                currentOutputDescription = "USB Audio Device"
            default:
                currentOutputDescription = output.portName
            }
        } else {
            currentOutputDescription = "No Output"
        }
    }
}
