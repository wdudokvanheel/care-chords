import SwiftUI

struct MainView: View {
    @EnvironmentObject private var model: AudioPlayerViewModel

    var body: some View {
        VStack {
            SpotifyView(spotify: model.spotify, playlistSelect: model.selectPlaylist)
                .frame(maxHeight: UIScreen.main.bounds.height * 0.5)
            Spacer()

            MusicControlsView(musicController: model.music, gstreamerController: model.gstreamer, toggleMute: model.toggleOutput, startSleepTimer: model.startSleepTimer)
        }
        .background(
            Color.appBackground
            .edgesIgnoringSafeArea(.all)
        )
        .onAppear(perform: model.onAppear)
        .onDisappear(perform: model.onDisappear)
    }
}

struct ControllerStateView: View {
    @ObservedObject var controller: GStreamerController
    let toggleOutput: () -> Void

    var body: some View {
        MuteButton(audioState: controller.state, action: toggleOutput)
//
//            Text("Audio state: \(controller.state.description)")
//            Text("Output: \(controller.currentOutput)")
//            Text("Gstreamer message: \(controller.backendMessage)")
    }
}




//struct PlayPauseButton: View {
//    let audioState: AudioState
//    let action: () -> Void
//
//    var body: some View {
//        Button(action: action) {
//            ZStack {
//                Circle()
//                    .fill(LinearGradient(
//                        gradient: Gradient(colors: [.blue, .purple]),
//                        startPoint: .topLeading,
//                        endPoint: .bottomTrailing
//                    ))
//                    .frame(width: 100, height: 100)
//                    .shadow(radius: 10)
//                    .scaleEffect(audioState == .playing ? 1.1 : 1.0)
//                    .animation(
//                        audioState == .playing ?
//                            Animation.easeInOut(duration: 1.4).repeatForever(autoreverses: true) :
//                            .default,
//                        value: audioState
//                    )
//
//                Image(systemName: audioState == .playing ? "pause.fill" : "play.fill")
//                    .resizable()
//                    .scaledToFit()
//                    .frame(width: 100, height: 50)
//                    .foregroundColor(.white)
//                    .padding(audioState != .playing ? EdgeInsets(top: 0, leading: 6, bottom: 0, trailing: 0) : EdgeInsets())
//            }
//        }
//        .buttonStyle(PlainButtonStyle())
//    }
//}
