import SwiftUI

struct ContentView: View {
    @StateObject private var gstreamerController = GStreamerController()
    
    var body: some View {
        VideoView()
            .onAppear {
                gstreamerController.startPipeline()
            }
            .onDisappear {
                gstreamerController.stopPipeline()
            }
            .environmentObject(self.gstreamerController)
    }
}
