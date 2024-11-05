import SwiftUI

struct CameraView: UIViewRepresentable {
    
    var placeholderView:UIView
    
    init(placeholderView: UIView) {
        self.placeholderView = placeholderView
    }
    
    func makeUIView(context: Context) -> some UIView {
        return placeholderView
    }
    
    func updateUIView(_ uiView: UIViewType, context: Context) {
        
    }
    
}

