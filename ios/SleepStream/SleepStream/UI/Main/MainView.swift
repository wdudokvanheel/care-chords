import SwiftUI

struct MainView: View {
    @EnvironmentObject private var model: AudioPlayerViewModel

    var body: some View {
        VStack {
            TabPanel {
                Tab(title: "Now playing") {
                    VStack {
                        Text("Now playing")
                    }
                }
                Tab(title: "Playlists") {
                    SpotifyView(spotify: model.spotify, playlistSelect: model.selectPlaylist)
                }
            }
            .padding()

            Spacer(minLength: 50)

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
