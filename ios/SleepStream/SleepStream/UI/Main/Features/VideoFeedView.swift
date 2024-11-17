import SwiftUI

struct VideoFeedView: View {
    @ObservedObject var controller: CameraController
    let onNew: () -> Void
    let queue = DispatchQueue(label: "run_app_q2")
    
    @State var controller2 = CameraController()
    
    var body: some View {
        VStack {
            GeometryReader { geometry in
                Spacer()
                CameraContainerView(camContainerViewController: self.controller)
                    .onAppear {

//                        print("GOGOGOOGOGO XX #($@*$(*@&^#(*$")
//                        self.controller.gstBackend?.run_app_pipeline_threaded()
                        
                        //                    self.controller.play()
//                        print("GOGO XXX DONESO!!")
                    }
                    .onDisappear {
//                        self.controller.gstBackend?.stopAndCleanup()
                    }
                    .scaleEffect(calculateScale(for: geometry.size))
                    .frame(width: geometry.size.width, height: geometry.size.height)
                    .clipShape(RoundedRectangle(cornerRadius: 8.0))
                Spacer()
            }
            .padding(16)
//            
//            GeometryReader { geometry in
//                Spacer()
//
//                CameraContainerView(camContainerViewController: controller2)
//                    .onAppear {
////                        print("GOGOGOOGOGO XX #($@*$(*@&^#(*$")
//                        controller2.initBackend()
//                        DispatchQueue.main.asyncAfter(deadline: .now() + 5) {
//                            controller2.play()
//                        }
//
//                        //                    self.controller.play()
////                        print("GOGO XXX DONESO!!")
//                    }
//                    .onDisappear {
////                        self.controller.gstBackend?.stopAndCleanup()
//                    }
//                    .scaleEffect(calculateScale(for: geometry.size))
//                    .frame(width: geometry.size.width, height: geometry.size.height)
//                    .clipShape(RoundedRectangle(cornerRadius: 8.0))
//                Spacer()
//            }
//            .padding(16)
            
            HStack {
                Button("First"){
                    controller.initBackend()
                }
                Button("Init") {
//                    let subviewsCopy = controller.camUIView.subviews.reversed() // Make a copy of the subviews array
//                    for subview in subviewsCopy {
//                        print("REMOVE \(subview)")
//                        subview.removeFromSuperview()
//                    }
                    
//                    controller.camUIView = UIView()
                    print("XX NEW BACKEND REQUESET")
//                    self.onNew()
                    print("XX NEW BACKEND DONE REQUESET")
                    queue.async {
                        print("STARTING VIDEO BACKEND XXX")
//                        self.controller.gstBackend?.setWindow(controller.camUIView)
                        if let bk = self.controller.gstBackend{
                            bk.run_app_pipeline_threaded()
                        }
                        else{
                            print("XX FAILED TO START PIPLINE BACKEND IS NIL")
                        }
                        print("STARTIED VIDEO BACKEND XXX")
                    }
                }
                
                Button("Play") {
                    controller.gstBackend?.play()
                }
                
                Button("pause") {
                    controller.gstBackend?.pause()
                }
                
                Button("Stop") {
                    controller.gstBackend?.stopAndCleanup()
                }
            }
        }
    }

    private func calculateScale(for size: CGSize) -> CGFloat {
        let maxDimension = size.width
        let targetDimension = 2560.0
        return min(maxDimension / targetDimension, 1.0)
    }
}

struct CameraContainerView: View {
    @ObservedObject var camContainerViewController: CameraController
    var body: some View {
        if camContainerViewController.gstBackend != nil {
            CameraView(placeholderView: camContainerViewController.camUIView)
                .frame(width: 2560, height: 1920)
        }
//        else {
//            let _ = camContainerViewController.initBackend()
//        }
    }
}
