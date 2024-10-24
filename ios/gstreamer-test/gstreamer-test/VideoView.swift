import SwiftUI
import UIKit

typealias guintptr = UInt
typealias GstVideoOverlay = OpaquePointer

struct VideoView: UIViewRepresentable {
    @EnvironmentObject
    var gstreamerController: GStreamerController

    func makeUIView(context: Context) -> UIView {
        let view = UIView()
        view.layer.backgroundColor = UIColor.black.cgColor
        gstreamerController.videoView = view
        return view
    }

    func updateUIView(_ uiView: UIView, context: Context) {}
//    private func setupGStreamerOverlay(for view: UIView) {
//        print("go")
//        if let pipeline = gstreamerController.pipeline {
//            let videoSinkName = "glimagesink"
//            print("1")
//            if let bin = UnsafeMutablePointer<GstBin>(OpaquePointer(pipeline)),
//               let videoSink = gst_bin_get_by_name(bin, videoSinkName)
//            {
//                print("2")
//                if let instance = UnsafeMutableRawPointer(videoSink)?.assumingMemoryBound(to: GTypeInstance.self),
//                   g_type_check_instance_is_a(instance, gst_video_overlay_get_type()) != 0
//                {
//                    print("1")
//                    let windowHandle = guintptr(bitPattern: Unmanaged.passUnretained(view).toOpaque())
//                    gst_video_overlay_set_window_handle(OpaquePointer(videoSink), windowHandle)
//                } else {
//                    print("Video sink does not support video overlay")
//                }
//                gst_object_unref(videoSink)
//            } else {
//                print("Could not find video sink named \(videoSinkName)")
//            }
//        }
//    }
}
