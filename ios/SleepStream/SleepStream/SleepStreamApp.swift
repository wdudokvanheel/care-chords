import SwiftUI

@main
struct SleepStreamApp: App {
    init() {
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
