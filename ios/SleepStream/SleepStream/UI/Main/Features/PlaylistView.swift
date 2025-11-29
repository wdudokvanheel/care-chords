import SwiftUI

struct PlaylistSelectorView: View {
    @ObservedObject var playlists: PlaylistController
    let playlistSelect: (Playlist) -> Void

    private let playlistSize: CGFloat = 110.0
    
    let columns: [GridItem] = [
         GridItem(.flexible()),
         GridItem(.flexible()),
         GridItem(.flexible())
     ]
    
    var body: some View {
        VStack {
            if playlists.isLoading {
                ProgressView("Loading playlists…")
                    .padding()
            } else if let error = playlists.errorMessage {
                Text(error)
                    .foregroundColor(.red)
                    .padding()
            } else {
                ScrollView {
                    LazyVGrid(columns: columns, spacing: 6) {
                        ForEach(playlists.playlists) { playlist in
                            Button(action: {
                                self.playlistSelect(playlist)
                            }) {
                                VStack(spacing: 0) {
                                    artwork(for: playlist)
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
                        }
                    }
                    .padding(.vertical, 8)
                    .padding(.horizontal, 8)
                }
            }
        }
        .onAppear {
            playlists.loadPlaylists()
        }
    }

    @ViewBuilder
    private func artwork(for playlist: Playlist) -> some View {
        if let img = playlist.image {
            RemoteImageView(imageUrl: img)
        } else {
            ZStack {
                Rectangle()
                    .foregroundColor(Color.playlistItem)
                Text(placeholderText(for: playlist.name))
                    .font(.headline)
                    .foregroundColor(.secondary)
            }
            .aspectRatio(1, contentMode: .fit)
        }
    }

    private func placeholderText(for name: String) -> String {
        let words = name.split(separator: " ")
        if let first = words.first {
            return String(first.prefix(2)).uppercased()
        }
        return String(name.prefix(2)).uppercased()
    }
}
