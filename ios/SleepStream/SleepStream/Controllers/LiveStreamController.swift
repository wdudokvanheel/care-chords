import AVKit
import Combine
import Dispatch
import Foundation
import SwiftUI
import UIKit

struct MonitorResponse: Codable {
    let url: String
}

@objc class LiveStreamController: NSObject, GStreamerVideoBackendDelegate, ObservableObject, AVPictureInPictureControllerDelegate {
    var gstBackend: GStreamerVideoBackend?
    @Published
    var view: VideoDisplayView = .init()
    
    @Published
    var gStreamerInitializationStatus: Bool = false
    @Published
    var messageFromGstBackend: String?
    
    @Published
    var isPipActive: Bool = false
    @Published
    var hasVideo: Bool = false
    
    private var pipController: AVPictureInPictureController?
    private var isPlaying: Bool = false
    private var cancellables = Set<AnyCancellable>()
    private var monitorUrl: String?
    
    override init() {
        super.init()
        self.gstBackend = GStreamerVideoBackend(self, videoView: view)
        
        // Observe app becoming active to refresh PiP if needed
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(appDidBecomeActive),
            name: UIApplication.didBecomeActiveNotification,
            object: nil
        )
    }
    
    deinit {
        NotificationCenter.default.removeObserver(self)
        self.stop()
    }
    
    @objc private func appDidBecomeActive() {
        // If PiP is active, flush the layer to refresh it
        if isPipActive {
            print("[LiveStreamController] App became active with PiP active, flushing video layer")
            view.flush()
        }
    }
    
    func setupPiP() {
        if AVPictureInPictureController.isPictureInPictureSupported() {
            let contentSource = AVPictureInPictureController.ContentSource(sampleBufferDisplayLayer: view.videoLayer, playbackDelegate: self)
            pipController = AVPictureInPictureController(contentSource: contentSource)
            pipController?.delegate = self
            pipController?.canStartPictureInPictureAutomaticallyFromInline = true
            print("PiP setup successful")
        } else {
//            print("PiP is not supported on this device")
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
        print("[LiveStreamController] play() called")
        isPlaying = true
        
        if let url = monitorUrl {
            gstBackend?.setMonitorUrl(url)
            gstBackend?.run_app_pipeline_threaded()
        } else {
            fetchMonitorUrl { [weak self] url in
                guard let self = self, let url = url else { return }
                self.monitorUrl = url
                self.gstBackend?.setMonitorUrl(url)
                if self.isPlaying {
                    self.gstBackend?.run_app_pipeline_threaded()
                }
            }
        }
    }
    
    private func fetchMonitorUrl(completion: @escaping (String?) -> Void) {
        let url = URL(string: "http://\(SleepStreamApp.SERVER):7755/monitor")!
        URLSession.shared.dataTaskPublisher(for: url)
            .map { $0.data }
            .decode(type: MonitorResponse.self, decoder: JSONDecoder())
            .receive(on: DispatchQueue.main)
            .sink(receiveCompletion: { completion in
                switch completion {
                case .failure(let error):
                    print("Error fetching monitor URL: \(error)")
                case .finished:
                    break
                }
            }, receiveValue: { response in
                completion(response.url)
            })
            .store(in: &cancellables)
    }
    
    func stop() {
        print("[LiveStreamController] stop() called")
        isPlaying = false
        pipController = nil
        hasVideo = false
        view.reset()
        gstBackend?.stopAndCleanup()
    }
    
    func flush() {
        view.flush()
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
        // print("Received sample buffer") // Commented out to avoid spam, uncomment if needed
        guard isPlaying else { return }
        
        view.enqueue(sampleBuffer)
        
        if !hasVideo {
            DispatchQueue.main.async {
                self.hasVideo = true
            }
        }
        
        if pipController == nil {
            DispatchQueue.main.async {
                self.setupPiP()
            }
        }
    }
    
    // MARK: - AVPictureInPictureControllerDelegate
    
    func pictureInPictureControllerDidStartPictureInPicture(_ pictureInPictureController: AVPictureInPictureController) {
        DispatchQueue.main.async {
            self.isPipActive = true
        }
    }
    
    func pictureInPictureControllerDidStopPictureInPicture(_ pictureInPictureController: AVPictureInPictureController) {
        DispatchQueue.main.async {
            self.isPipActive = false
        }
    }
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
