import SwiftUI

struct SpotifyView: View {
    @ObservedObject var spotify: SpotifyController
    let playlistSelect: (Playlist) -> Void

    private let playlistSize: CGFloat = 110.0
    var body: some View {
        if !spotify.isAuthorized {
            Button("Login with Spotify to view your playlsits") {
                spotify.authorize()
            }
            .buttonStyle(.borderedProminent)
        }
        else {
            VStack {
                VStack {
                    ScrollView {
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: playlistSize, maximum: playlistSize))], spacing: 8.0) {
                            ForEach(spotify.playlists) { playlist in
                                Button(action: {
                                    self.playlistSelect(playlist)
                                }) {
                                    VStack(spacing: 0) {
                                        if let img = playlist.image {
                                            RemoteImageView(imageUrl: img)
                                        }
                                        Text(playlist.name)
                                            .foregroundColor(.white)
                                            .font(.caption)
                                            .fontWeight(.light)
                                            .lineLimit(1)
                                            .multilineTextAlignment(.center)
                                            .padding(0)
                                    }
                                    .padding(0)
                                }
                                .padding(0)
                                .frame(width: playlistSize, height: playlistSize)
                            }
                        }
                        .padding(0)
                    }
                }
                .padding(0)
            }
        }
    }
}
