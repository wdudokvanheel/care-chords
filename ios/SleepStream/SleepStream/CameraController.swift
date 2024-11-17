import Dispatch
import Foundation
import SwiftUI
import UIKit

@objc class CameraController: NSObject, GStreamerBackendDelegate, ObservableObject {
    func updateAudioState() {
    }
    
    func gstreamerAudioState(state: AudioState) {
    }
    
    var gstBackend: GStreamerVideoBackend?
    @Published
    var camUIView: UIView = UIView()
    
    @Published
    var gStreamerInitializationStatus: Bool = false
    @Published
    var messageFromGstBackend: String?
    
    func initBackend() {
        self.gstBackend = GStreamerVideoBackend(self, videoView: self.camUIView)
        let queue = DispatchQueue(label: "run_app_q")
        queue.async {
//            print("STARTING VIDEO BACKEND XXX")
//            self.gstBackend?.run_app_pipeline_threaded()
        }
    }
    
    func play() {
        if self.gstBackend == nil {
            print("XX NIL BACKEND")
            self.initBackend()
        }
        print("XX GO PLAY CONTROLLER")
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
