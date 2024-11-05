import Foundation
import os
import SwiftUI

struct AudioPlayerView: View {
    let controller: AudioController
    
    init() {
        self.controller = AudioController()
    }

    var body: some View {
        Text("Audio")
    }
}
