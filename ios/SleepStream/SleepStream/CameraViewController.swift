import Dispatch
import Foundation
import SwiftUI
import UIKit

@objc class CameraViewController: NSObject, GStreamerBackendDelegate, ObservableObject {
    func updateAudioState() {
    }
    
    func gstreamerAudioState(state: AudioState) {
    }
    
    var gstBackend: GStreamerVideoBackend?
    var camUIView: UIView
    
    @Published
    var gStreamerInitializationStatus: Bool = false
    @Published
    var messageFromGstBackend: String?
    
    init(camUIView: UIView) {
        self.camUIView = camUIView
    }
    
    func initBackend() {
        self.gstBackend = GStreamerVideoBackend(self, videoView: self.camUIView)
        let queue = DispatchQueue(label: "run_app_q")
        queue.async {
            self.gstBackend?.run_app_pipeline_threaded()
        }
    }
    
    func play() {
        if self.gstBackend == nil {
            self.initBackend()
        }
        self.gstBackend!.play()
    }
    
    func pause() {
        self.gstBackend!.pause()
    }
    
    @objc func gStreamerInitialized() {
        DispatchQueue.main.async{
            print("Init complete")
            self.gStreamerInitializationStatus = true
        }
    }
    
    @objc func gstreamerMessage(message: String) {
        DispatchQueue.main.async{
            self.messageFromGstBackend = message
        }
    }
}
