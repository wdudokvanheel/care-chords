import SwiftUI
import MediaPlayer

@main
struct SleepStreamApp: App {
    var audioViewModel: ViewModel

    init() {
        setenv("GST_DEBUG", "4", 1)
        setenv("GST_DEBUG_NO_COLOR", "1", 1)
        gst_ios_init()
      
        let audioLibrary: AudioLibraryController = .init()
        audioViewModel = .init(audioLibrary: audioLibrary)
    }

    var body: some Scene {
        WindowGroup {
            MainView()
                .environmentObject(audioViewModel)
        }
    }
}
