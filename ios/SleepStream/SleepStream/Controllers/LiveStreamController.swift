import Dispatch
import Foundation
import SwiftUI
import UIKit

@objc class LiveStreamController: NSObject, GStreamerVideoBackendDelegate, ObservableObject {
    var gstBackend: GStreamerVideoBackend?
    @Published
    var view: UIView = .init()
    
    @Published
    var gStreamerInitializationStatus: Bool = false
    @Published
    var messageFromGstBackend: String?
    
    override init() {
        super.init()
        self.gstBackend = GStreamerVideoBackend(self, videoView: self.view)
    }
    
    deinit {
        self.stop()
    }
    
    func play() {
        self.gstBackend?.run_app_pipeline_threaded()
    }
    
    func stop() {
        self.gstBackend?.stopAndCleanup()
    }
    
    @objc func gStreamerInitialized() {
        DispatchQueue.main.async {
            self.gStreamerInitializationStatus = true
        }
    }
    
    @objc func gstreamerMessage(message: String) {
        DispatchQueue.main.async {
            self.messageFromGstBackend = message
        }
    }
    
    @objc func gstreamerDidReceiveVideoResolution(width: Int, height: Int) {
        print("Video resolution: \(width)x\(height)")
    }
}
