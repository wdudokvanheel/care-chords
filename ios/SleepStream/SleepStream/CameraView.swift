import SwiftUI

struct CameraView: UIViewRepresentable {
    
    var placeholderView: UIView
    
    init(placeholderView: UIView) {
        self.placeholderView = placeholderView
    }
    
    func makeUIView(context: Context) -> some UIView {
//        placeholderView.contentMode = .scaleAspectFit
        placeholderView.contentMode = .center
        placeholderView.clipsToBounds = true;
//        placeholderView.magnificationFilter = .
        return placeholderView
    }
    
    func updateUIView(_ uiView: UIViewType, context: Context) {
        print("XXXX UPDATE VIEW")
    }
}
