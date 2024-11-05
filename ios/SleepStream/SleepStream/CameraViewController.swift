import Dispatch
import Foundation
import SwiftUI
import UIKit

@objc class CameraViewController: NSObject, GStreamerBackendProtocol, ObservableObject {
    var gstBackend: GStreamerBackend?
    var camUIView: UIView
    
    @Published
    var gStreamerInitializationStatus: Bool = false
    @Published
    var messageFromGstBackend: String?
    
    init(camUIView: UIView) {
        self.camUIView = camUIView
    }
    
    func initBackend() {
        self.gstBackend = GStreamerBackend(self, videoView: self.camUIView)
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
    
    @objc func gstreamerSetUIMessageWithMessage(message: String) {
        DispatchQueue.main.async{
            self.messageFromGstBackend = message
        }
    }
}
