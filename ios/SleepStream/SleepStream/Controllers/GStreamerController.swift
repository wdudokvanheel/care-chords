import AVKit
import Foundation
import os
import SwiftUI

class GStreamerController: GStreamerAudioBackendDelegate, ObservableObject {
    @Published var state: AudioState = .initializing
    @Published var backendMessage: String = ""
    @Published var backendError: Bool = false

    private var gstBackend: GStreamerAudioBackend?

    init() {
        self.gstBackend = GStreamerAudioBackend(self)

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
