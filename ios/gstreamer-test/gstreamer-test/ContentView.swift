import SwiftUI

struct ContentView: View {
    @StateObject private var gstreamerController = GStreamerController()
    
    var body: some View {
        VStack{
            VideoView()
                .frame(width: 2560 / 2, height: 1920 / 2)
                .onAppear {
                    gstreamerController.startPipeline()
                }
                .onDisappear {
                    gstreamerController.stopPipeline()
                }
                .environmentObject(self.gstreamerController)
            Spacer()
        }
    }
}
