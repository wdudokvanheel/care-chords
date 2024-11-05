import SwiftUI

struct ContentView: View {
    @StateObject 
    private var gstreamerController = GStreamerController(video: true)
    
    var body: some View {
        VStack{
            Button("Switch"){
                gstreamerController.toggleVideo()
            }
            GStreamerView()
                .frame(width: 2560 / 8, height: 1920 / 8)
                .onAppear {
//                    gstreamerController.startPipeline()
                }
                .onDisappear {
//                    gstreamerController.stopPipeline()
                }
                .environmentObject(self.gstreamerController)
        }
    }
}
