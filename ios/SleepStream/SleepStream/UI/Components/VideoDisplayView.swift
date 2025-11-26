import UIKit
import AVFoundation

class VideoDisplayView: UIView {
    
    override class var layerClass: AnyClass {
        return AVSampleBufferDisplayLayer.self
    }
    
    var videoLayer: AVSampleBufferDisplayLayer {
        return layer as! AVSampleBufferDisplayLayer
    }
    
    override init(frame: CGRect) {
        super.init(frame: frame)
        setupLayer()
    }
    
    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupLayer()
    }
    
    private func setupLayer() {
        videoLayer.videoGravity = .resizeAspect
        videoLayer.isOpaque = true
    }
    
    func enqueue(_ sampleBuffer: CMSampleBuffer) {
        if videoLayer.status == .failed {
            print("VideoLayer failed: \(String(describing: videoLayer.error))")
            videoLayer.flush()
        }
        
        if videoLayer.isReadyForMoreMediaData {
            videoLayer.enqueue(sampleBuffer)
        } else {
            print("VideoLayer not ready for more media data")
        }
    }
    
    func flush() {
        videoLayer.flush()
    }
    
    func reset() {
        videoLayer.flush()
        videoLayer.flushAndRemoveImage()
    }
}
