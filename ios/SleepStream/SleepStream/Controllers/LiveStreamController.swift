import Dispatch
import Foundation
import SwiftUI
import UIKit
import AVKit

@objc class LiveStreamController: NSObject, GStreamerVideoBackendDelegate, ObservableObject, AVPictureInPictureControllerDelegate {
    var gstBackend: GStreamerVideoBackend?
    @Published
    var view: VideoDisplayView = VideoDisplayView()
    
    @Published
    var gStreamerInitializationStatus: Bool = false
    @Published
    var messageFromGstBackend: String?
    
    private var pipController: AVPictureInPictureController?
    
    override init() {
        super.init()
        self.gstBackend = GStreamerVideoBackend(self, videoView: self.view)
    }
    
    deinit {
        self.stop()
    }
    
    func setupPiP() {
        if AVPictureInPictureController.isPictureInPictureSupported() {
            let contentSource = AVPictureInPictureController.ContentSource(sampleBufferDisplayLayer: view.videoLayer, playbackDelegate: self)
            pipController = AVPictureInPictureController(contentSource: contentSource)
            pipController?.delegate = self
            pipController?.canStartPictureInPictureAutomaticallyFromInline = true
            print("PiP setup successful")
        } else {
            print("PiP is not supported on this device")
        }
    }
    
    func togglePiP() {
        guard let pipController = pipController else {
            print("PiP controller is nil")
            return
        }
        
        if pipController.isPictureInPictureActive {
            print("Stopping PiP")
            pipController.stopPictureInPicture()
        } else {
            print("Starting PiP")
            pipController.startPictureInPicture()
        }
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
    
    @objc func gstreamerDidReceiveSampleBuffer(_ sampleBuffer: CMSampleBuffer) {
        view.enqueue(sampleBuffer)
        
        if pipController == nil {
            DispatchQueue.main.async {
                self.setupPiP()
            }
        }
    }
    
    // MARK: - AVPictureInPictureSampleBufferPlaybackDelegate
    
    // These methods are required for AVPictureInPictureController.ContentSource(sampleBufferDisplayLayer:playbackDelegate:)
    // Since we are not implementing full playback control (seek, etc.), we can leave them empty or minimal.
}

extension LiveStreamController: AVPictureInPictureSampleBufferPlaybackDelegate {
    func pictureInPictureController(_ pictureInPictureController: AVPictureInPictureController, setPlaying playing: Bool) {
        // Handle play/pause from PiP controls if needed
    }
    
    func pictureInPictureController(_ pictureInPictureController: AVPictureInPictureController, didTransitionToRenderSize newRenderSize: CMVideoDimensions) {
        // Handle size change
    }
    
    func pictureInPictureController(_ pictureInPictureController: AVPictureInPictureController, skipByInterval skipInterval: CMTime, completion completionHandler: @escaping () -> Void) {
        completionHandler()
    }
    
    func pictureInPictureControllerIsPlaybackPaused(_ pictureInPictureController: AVPictureInPictureController) -> Bool {
        return false
    }
    
    func pictureInPictureControllerTimeRangeForPlayback(_ pictureInPictureController: AVPictureInPictureController) -> CMTimeRange {
        return CMTimeRange(start: .zero, duration: .positiveInfinity) // Live stream
    }
}
