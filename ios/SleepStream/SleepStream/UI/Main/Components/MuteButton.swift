import SwiftUI

struct MuteButton: View {
    var audioState: AudioState
    let action: () -> Void

    var body: some View {
        HStack {
            Button(action: action) {
                Image(systemName: audioState == .playing ? "speaker.wave.2" : "speaker.slash.fill")
                    .foregroundStyle(audioState == .playing ? Color.muteButtonInactive : Color.muteButtonActive)
                    .font(.system(size: 32))
            }
        }
    }
}
