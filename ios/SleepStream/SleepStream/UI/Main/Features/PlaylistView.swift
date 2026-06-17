import SwiftUI

struct AudioLibraryView: View {
    @ObservedObject var library: AudioLibraryController
    let itemSelect: (AudioItem) -> Void

    private let columns: [GridItem] = [
        GridItem(.flexible()),
        GridItem(.flexible()),
        GridItem(.flexible())
    ]

    var body: some View {
        VStack(spacing: 0) {
            if library.isLoading && library.localItems.isEmpty {
                ProgressView("Loading audio…")
                    .padding()
            } else if let error = library.errorMessage, library.localItems.isEmpty {
                Text(error)
                    .foregroundColor(.red)
                    .padding()
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        if !library.systemPlaylists.isEmpty {
                            section("Playlists", items: library.systemPlaylists)
                        }

                        localSection

                        if library.spotifyAvailable {
                            section("Spotify", items: library.spotifyPlaylists)
                        }
                    }
                    .padding(.vertical, 8)
                    .padding(.horizontal, 8)
                }
            }
        }
        .onAppear {
            library.load()
        }
    }

    private var localSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Local")
                    .font(.headline)
                    .foregroundColor(.white)
                Spacer()
                if library.localPath != nil {
                    Button(action: library.openLocalRoot) {
                        Image(systemName: "house")
                            .foregroundColor(.orange)
                    }
                }
                Button(action: library.refresh) {
                    Image(systemName: "arrow.clockwise")
                        .foregroundColor(.orange)
                }
            }
            if !localFolders.isEmpty {
                grid(items: localFolders)
            }
            if !localFiles.isEmpty {
                fileRows(items: localFiles)
            }
        }
    }

    private var localFolders: [AudioItem] {
        library.localItems.filter { $0.kind == .folder }
    }

    private var localFiles: [AudioItem] {
        library.localItems.filter { $0.kind == .file }
    }

    private func section(_ title: String, items: [AudioItem]) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.headline)
                .foregroundColor(.white)
            grid(items: items)
        }
    }

    private func grid(items: [AudioItem]) -> some View {
        LazyVGrid(columns: columns, spacing: 6) {
            ForEach(items) { item in
                Button(action: {
                    if item.kind == .folder && item.source == "local" {
                        library.openLocalFolder(item)
                    } else {
                        itemSelect(item)
                    }
                }) {
                    VStack(spacing: 0) {
                        artwork(for: item)
                        Text(item.name)
                            .foregroundColor(Color.playlistItemLabel)
                            .font(.caption)
                            .fontWeight(.light)
                            .lineLimit(1)
                            .multilineTextAlignment(.center)
                            .padding(.horizontal, 4)
                            .padding(.top, 1)
                        if let subtitle = item.subtitle {
                            Text(subtitle)
                                .foregroundColor(Color.playlistItemLabel.opacity(0.75))
                                .font(.caption2)
                                .fontWeight(.light)
                                .lineLimit(1)
                                .multilineTextAlignment(.center)
                                .padding(.horizontal, 4)
                                .padding(.bottom, 2)
                        } else {
                            Spacer()
                                .frame(height: 2)
                        }
                    }
                    .background(Color.playlistItem)
                    .padding(0)
                }
                .padding(2)
            }
        }
    }

    private func fileRows(items: [AudioItem]) -> some View {
        LazyVStack(spacing: 6) {
            ForEach(items) { item in
                Button(action: {
                    itemSelect(item)
                }) {
                    HStack(spacing: 10) {
                        ZStack {
                            RoundedRectangle(cornerRadius: 8)
                                .fill(Color.white.opacity(0.08))
                            Image(systemName: "music.note")
                                .font(.title3)
                                .foregroundColor(.orange)
                        }
                        .frame(width: 44, height: 44)

                        VStack(alignment: .leading, spacing: 3) {
                            Text(item.name)
                                .foregroundColor(Color.playlistItemLabel)
                                .font(.subheadline)
                                .fontWeight(.medium)
                                .lineLimit(1)
                            if let subtitle = item.subtitle {
                                Text(subtitle)
                                    .foregroundColor(Color.playlistItemLabel.opacity(0.72))
                                    .font(.caption)
                                    .lineLimit(1)
                            }
                        }

                        Spacer(minLength: 8)

                        Image(systemName: "play.fill")
                            .font(.caption)
                            .foregroundColor(.orange)
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 8)
                    .background(Color.playlistItem)
                    .cornerRadius(8)
                }
                .buttonStyle(.plain)
            }
        }
    }

    @ViewBuilder
    private func artwork(for item: AudioItem) -> some View {
        if let img = item.image {
            RemoteImageView(imageUrl: img)
        } else {
            ZStack {
                Rectangle()
                    .foregroundColor(Color.playlistItem)
                Image(systemName: iconName(for: item))
                    .font(.largeTitle)
                    .foregroundColor(.secondary)
            }
            .aspectRatio(1, contentMode: .fit)
        }
    }

    private func iconName(for item: AudioItem) -> String {
        switch item.kind {
        case .folder:
            return "folder"
        case .playlist:
            return item.source == "spotify" ? "music.note.list" : "text.badge.plus"
        case .file:
            return "music.note"
        }
    }
}

typealias PlaylistSelectorView = AudioLibraryView
