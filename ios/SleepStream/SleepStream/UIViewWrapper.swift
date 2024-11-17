import SwiftUI

struct UIViewWrapper: UIViewRepresentable {
    var view: UIView
    
    init(view: UIView) {
        self.view = view
    }
    
    func makeUIView(context: Context) -> some UIView {
        view.contentMode = .center
        view.clipsToBounds = true
        return view
    }
    
    func updateUIView(_ uiView: UIViewType, context: Context) {}
}
