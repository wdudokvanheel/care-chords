import SwiftUI

struct AudioPlayerView: View {
    @StateObject private var model = AudioPlayerViewModel()

    var body: some View {
        VStack {
            ControllerStateView(controller: model.controller)

            HStack {
                ForEach(model.playlists) { playlist in
                    Button(action: {
                        model.selectPlaylist(playlist: playlist)
                    }) {
                        Text(playlist.name)
                            .foregroundColor(.white)
                    }
                    .padding(.all, 8)
                    .background {
                        RoundedRectangle(cornerRadius: 8.0)
                            .foregroundColor(.indigo)
                    }
                }
            }
            Spacer()
            PlayPauseButton(audioState: model.controller.state, action: model.togglePlayState)
                .buttonStyle(.borderedProminent)

            MusicStateView(controller: model.music)
        }
        .background(
            LinearGradient(
                gradient: Gradient(colors: [Color.darkerBlue, Color.veryDarkBlue]),
                startPoint: .top,
                endPoint: .bottom
            )
            .edgesIgnoringSafeArea(.all)
        )
        .onAppear(perform: model.onAppear)
        .onDisappear(perform: model.onDisappear)
    }
}

struct ControllerStateView: View {
    @ObservedObject var controller: AudioController

    var body: some View {
        VStack {
            Text("Audio state: \(controller.state.description)")
            Text("Output: \(controller.currentOutput)")
            Text("Gstreamer message: \(controller.backendMessage)")
        }
    }
}

struct MusicStateView: View {
    @ObservedObject var controller: MusicController

    var body: some View {
        VStack {
            VStack {
                if let metadata = controller.status.metadata {
                    if let url = URL(string: metadata.artwork_url) {
                        RemoteImageView(imageUrl: url)
                    }

                    Text(metadata.title)
                        .foregroundStyle(.white)
                        .fontWeight(.bold)
                        .multilineTextAlignment(.center)
                        .lineLimit(1)
                    Text(metadata.artist)
                        .foregroundStyle(.white)
                        .opacity(0.9)
                        .fontWeight(.light)
                }
            }
            HStack {
                Spacer()
                // Previous Song Button
                Button(action: {
                    controller.previous()
                }) {
                    Image(systemName: "backward.fill")
                        .resizable()
                        .scaledToFit()
                        .frame(width: 40, height: 40)
                        .foregroundColor(.white)
                }

                Spacer()

                // Play/Pause Button
                Button(action: {
                    if controller.status.playing {
                        controller.pause()
                    }
                    else {
                        controller.play()
                    }
                }) {
                    Image(systemName: controller.status.playing ? "pause.fill" : "play.fill")
                        .resizable()
                        .scaledToFit()
                        .frame(width: 50, height: 50)
                        .foregroundColor(.white)
                        .padding(.leading, 6)
                }

                Spacer()

                // Next Song Button
                Button(action: {
                    controller.next()
                }) {
                    Image(systemName: "forward.fill")
                        .resizable()
                        .scaledToFit()
                        .frame(width: 40, height: 40)
                        .foregroundColor(.white)
                }

                Spacer()
            }
            .padding(.bottom)
        }
        .frame(maxWidth: .infinity)
        .padding()
        .background(
            LinearGradient(
                gradient: Gradient(colors: [.lightForestGreen, .darkForestGreen]),
                startPoint: .top,
                endPoint: .bottom
            )
            .clipShape(RoundedRectangle(cornerRadius: 20, style: .continuous))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .stroke(Color.black.opacity(0.8), lineWidth: 2)
        )
        .shadow(color: Color.lightForestGreen.opacity(0.2), radius: 10, x: 0, y: 0)
        .padding()
    }
}

struct RemoteImageView: View {
    let imageUrl: URL

    var body: some View {
        AsyncImage(url: imageUrl) { phase in
            switch phase {
            case .empty:
                ProgressView() // Show a loading indicator while the image loads
            case .success(let image):
                image
                    .resizable()
                    .scaledToFit() // You can adjust this to `.scaledToFill()` or other as needed
            case .failure:
                Image(systemName: "photo")
                    .resizable()
                    .scaledToFit()
                    .foregroundColor(.gray) // Fallback in case of an error
            @unknown default:
                EmptyView()
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 8.0))
        .overlay(
            RoundedRectangle(cornerRadius: 8.0, style: .continuous)
                .stroke(Color.black, lineWidth: 1)
        )
//        .frame(width: 200, height: 200) // Adjust the frame as needed
    }
}

struct ActionRequest: Encodable {
    let action: String
}

struct PlaybackRequest: Encodable {
    let uri: String
}

struct Playlist: Identifiable {
    let id = UUID()
    let name: String
    let uri: String

    init(_ name: String, _ uri: String) {
        self.name = name
        self.uri = uri
    }
}

extension Color {
    static let veryDarkBlue = Color(red: 0.02, green: 0.08, blue: 0.15) // Very dark blue
    static let darkerBlue = Color(red: 0.03, green: 0.1, blue: 0.2) // Darker blue
    static let darkForestGreen = Color(red: 0.0, green: 0.27, blue: 0.13)
    static let lightForestGreen = Color(red: 0.13, green: 0.55, blue: 0.13)
}

struct PlayPauseButton: View {
    let audioState: AudioState
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            ZStack {
                Circle()
                    .fill(LinearGradient(
                        gradient: Gradient(colors: [.blue, .purple]),
                        startPoint: .topLeading,
                        endPoint: .bottomTrailing
                    ))
                    .frame(width: 100, height: 100)
                    .shadow(radius: 10)
                    .scaleEffect(audioState == .playing ? 1.1 : 1.0)
                    .animation(
                        audioState == .playing ?
                            Animation.easeInOut(duration: 1.4).repeatForever(autoreverses: true) :
                            .default,
                        value: audioState
                    )

                Image(systemName: audioState == .playing ? "pause.fill" : "play.fill")
                    .resizable()
                    .scaledToFit()
                    .frame(width: 100, height: 50)
                    .foregroundColor(.white)
                    .padding(audioState != .playing ? EdgeInsets(top: 0, leading: 6, bottom: 0, trailing: 0) : EdgeInsets())
            }
        }
        .buttonStyle(PlainButtonStyle())
    }
}
