import SwiftUI
import UIKit

typealias guintptr = UInt
typealias GstVideoOverlay = OpaquePointer

struct VideoView: UIViewRepresentable {
    @EnvironmentObject
    var gstreamerController: GStreamerController

    func makeUIView(context: Context) -> UIView {
        let view = UIView()
        view.layer.backgroundColor = UIColor.blue.cgColor
        view.contentMode = .scaleAspectFit
        gstreamerController.videoView = view
        view.frame = CGRect(x: 0, y: 0, width: 2560, height: 1920) // Adjust as needed

        return view
    }

    func updateUIView(_ uiView: UIView, context: Context) {}
}
