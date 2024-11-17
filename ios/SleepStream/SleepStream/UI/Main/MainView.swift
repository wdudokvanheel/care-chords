import SwiftUI

struct MainView: View {
    @EnvironmentObject private var model: ViewModel

    var body: some View {
        VStack {
//            TabPanel {
//                Tab(title: "Camera") {
            VideoFeedView(controller: model.video, onNew: model.newVideo)
//                }
//                Tab(title: "Now playing") {
//                    NowPlayingView(controller: model.music)
//                }
//                Tab(title: "Playlists") {
//                    PlaylistSelectorView(spotify: model.spotify, playlistSelect: model.selectPlaylist)
//                }
//            }
//            .padding(0)
//
//            Spacer(minLength: 30)
//
//            MusicControlsView(musicController: model.music, gstreamerController: model.gstreamer, toggleMute: model.toggleOutput, startSleepTimer: model.startSleepTimer)
        }
        .padding(0)
        .shadow(radius: 4)
        .background(
            Image("Background")
                .resizable()
                .scaledToFill()
                .ignoresSafeArea()
        )
        .onAppear(perform: model.onAppear)
        .onDisappear(perform: model.onDisappear)
    }
}
