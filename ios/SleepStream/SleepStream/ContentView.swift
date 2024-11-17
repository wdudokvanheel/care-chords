//import SwiftUI
//
//struct ContentView: View {
//    @ObservedObject var camViewController:CameraController
//    
//    init() {
////        self.camViewController = CameraController(camUIView: UIView())
//    }
//    
//    func play_stream(){
//        self.camViewController.play()
//    }
//    
//    
//    func pause_stream(){
//        self.camViewController.pause()
//    }
//    
//    var body: some View {
//        GeometryReader { geometry in
//            VStack {
//                CameraContainerView(camContainerViewController: self.camViewController).padding(.all, 20)
//                Spacer()
//                GStreamerStatusMessageView(camContainerViewController: self.camViewController)
//                HStack(spacing: 10){
//                    Button{
//                        play_stream()
//                    }label: {
//                        Image(systemName: "play")
//                    }.padding()
//                    Button{
//                        pause_stream()
//                    }label: {
//                        Image(systemName: "pause")
//                    }.padding()
//                }.frame(height: CGFloat(geometry.size.height * 0.10))
//                    .disabled(!self.camViewController.gStreamerInitializationStatus)
//            }.position(x: geometry.size.width / 2, y: geometry.size.height / 2)
//        }
//    }
//}
//
//

//
//struct GStreamerStatusMessageView: View{
//    @ObservedObject var camContainerViewController:CameraController
//    var body: some View{
//        if self.camContainerViewController.gStreamerInitializationStatus, let msg = self.camContainerViewController.messageFromGstBackend{
//            Text(msg)
//        }else{
//            EmptyView()
//        }
//        
//    }
//}
