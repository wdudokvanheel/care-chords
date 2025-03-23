import AVKit
import Foundation
import MediaPlayer
import os
import SwiftUI

class GStreamerController: GStreamerAudioBackendDelegate, ObservableObject {
    @Published var state: AudioState = .stopped
    @Published var backendMessage: String = ""
    @Published var backendError: Bool = false

    private var gstBackend: GStreamerAudioBackend?

    init() {
        self.gstBackend = GStreamerAudioBackend(self)
    }

    func pause() {
        gstBackend?.stop()
    }

    func play() {
        DispatchQueue(label: "gstreamer_audio_queue").async {
            self.gstBackend?.run_app_pipeline_threaded()
        }
    }

    func gStreamerInitialized() {}

    func gstreamerMessage(message: String) {
        DispatchQueue.main.async {
            print("Got GStreamer message: \(message)")
            self.backendMessage = message
        }
    }

    func gstreamerAudioState(state newstate: AudioState) {
        DispatchQueue.main.async {
            self.state = newstate
        }
    }
}
