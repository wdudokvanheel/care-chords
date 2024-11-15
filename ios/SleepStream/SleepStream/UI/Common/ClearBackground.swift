import SwiftUI

struct ClearBackground: UIViewControllerRepresentable {

    public func makeUIViewController(context: UIViewControllerRepresentableContext<ClearBackground>) -> UIViewController {
        return Controller()
    }

    public func updateUIViewController(_ uiViewController: UIViewController, context: UIViewControllerRepresentableContext<ClearBackground>) {}

    class Controller: UIViewController {
        override func viewDidLoad() {
            super.viewDidLoad()
            view.backgroundColor = .clear
        }

        override func willMove(toParent parent: UIViewController?) {
            super.willMove(toParent: parent)
            parent?.view?.backgroundColor = .clear
        }
    }
}
