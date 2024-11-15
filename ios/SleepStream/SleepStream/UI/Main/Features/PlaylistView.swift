import SwiftUI

struct SpotifyView: View {
    @ObservedObject var spotify: SpotifyController
    let playlistSelect: (Playlist) -> Void

    private let playlistSize: CGFloat = 110.0
    var body: some View {
        if !spotify.isAuthorized {
            Button("Login with Spotify") {
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
                                .padding(.all, 0)
                                .frame(width: playlistSize, height: playlistSize)
                            }
                        }
                        .padding(.all, 0)
                    }
                    .padding(.horizontal, 4)
                    .padding(.vertical, 12)
                }
                .padding(.all, 0)
            }
            .background(
                LinearGradient(
                    gradient: Gradient(colors: [.black.opacity(0.3), .black.opacity(0.6)]),
                    startPoint: .top,
                    endPoint: .bottom
                )
                .clipShape(RoundedRectangle(cornerRadius: 20, style: .continuous))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 20, style: .continuous)
                    .stroke(Color.white.opacity(0.1), lineWidth: 2)
            )
            .frame(maxWidth: .infinity)
            .padding()
        }
    }
}
