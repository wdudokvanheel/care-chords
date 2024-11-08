import Foundation
import SwiftUI
import AVKit

class AudioManager: ObservableObject {
    @Published var currentOutput: String = "Unknown"

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
            
            switch output.portType {
            case .builtInSpeaker:
                currentOutput = "\(output.portName) iPhone Speaker"
            case .bluetoothA2DP, .bluetoothLE, .bluetoothHFP:
                currentOutput = "\(output.portName) Bluetooth Device"
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
}
