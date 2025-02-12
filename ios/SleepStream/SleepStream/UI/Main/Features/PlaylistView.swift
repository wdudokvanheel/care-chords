import SwiftUI

struct PlaylistSelectorView: View {
    @ObservedObject var spotify: SpotifyController
    let playlistSelect: (Playlist) -> Void

    private let playlistSize: CGFloat = 110.0
    
    let columns: [GridItem] = [
         GridItem(.flexible()),
         GridItem(.flexible()),
         GridItem(.flexible())
     ]
    
    var body: some View {
        if !spotify.isAuthorized {
            Button("Login with Spotify") {
                spotify.authorize()
            }
            .buttonStyle(.borderedProminent)
            .tint(Color.darkerBlue)
            .buttonBorderShape(.roundedRectangle(radius: 0))
            .foregroundColor(Color.moonWhite)
            .padding()
        }
        else {
            VStack {
                VStack {
                    ScrollView {
                        LazyVGrid(columns: columns, spacing: 6) {
                            ForEach(spotify.playlists) { playlist in
                                Button(action: {
                                    self.playlistSelect(playlist)
                                }) {
                                    VStack(spacing: 0) {
                                        if let img = playlist.image {
                                            RemoteImageView(imageUrl: img)
//                                                .clipShape(RoundedRectangle(cornerRadius: 4))
                                        }
                                        Text(playlist.name)
                                            .foregroundColor(Color.playlistItemLabel)
                                            .font(.caption)
                                            .fontWeight(.light)
                                            .lineLimit(1)
                                            .multilineTextAlignment(.center)
                                            .padding(.horizontal, 4)
                                            .padding(.vertical, 1)
                                    }
                                    .background(Color.playlistItem)
                                    
                                    .padding(0)
                                }
                                .padding(2)
//                                .frame(width: playlistSize, height: playlistSize)
                            }
                        }
                        .padding(.vertical, 8)
                        .padding(.horizontal, 8)
                    }
                }
                .padding(0)
            }
        }
    }
}
