import SwiftUI

@main
struct SleepStreamApp: App {
    init() {
        setenv("GST_DEBUG", "4", 1)
        setenv("GST_DEBUG_NO_COLOR", "1", 1)

        print("Init gst")
        gst_ios_init()
        print("Init gst done")
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}
