import SwiftUI
import MediaPlayer

@main
struct SleepStreamApp: App {
    public static let SERVER = "10.0.0.20"
    
    var audioViewModel: ViewModel

    init() {
        setenv("GST_DEBUG", "4", 1)
        setenv("GST_DEBUG_NO_COLOR", "1", 1)
        gst_ios_init()
      
        let playlists: PlaylistController = .init()
        audioViewModel = .init(playlists: playlists)
    }

    var body: some Scene {
        WindowGroup {
            MainView()
                .environmentObject(audioViewModel)
        }
    }
}
