import Foundation
import os
import SwiftUI
import AVKit

class AudioController: GStreamerBackendProtocol {
    var gstBackend: GStreamerAudioBackend?
    
    init() {
        print("Starting audio")
        self.gstBackend = GStreamerAudioBackend(self)
        configureAudioSession()
        
        let queue = DispatchQueue(label: "gstreamer_audio_queue")
        queue.async {
            self.gstBackend?.run_app_pipeline_threaded()
        }
        
    }
    
    func configureAudioSession() {
        let session = AVAudioSession.sharedInstance()
        do {
            try session.setCategory(.playback, mode: .default, options: [])
            try session.setActive(true)
        } catch {
            print("Failed to set up audio session: \(error)")
        }
    }
    
    func gStreamerInitialized() {
        print("Init AUDIO complete")
        gstBackend?.play()
    }
    
    func gstreamerSetUIMessageWithMessage(message: String) {
        print("Got message: \(message)")
    }
 
}
