import SwiftUI

struct AudioPlayerView: View {
    @EnvironmentObject private var model: AudioPlayerViewModel

    var body: some View {
        VStack {
            SpotifyView(spotify: model.spotify, playlistSelect: model.selectPlaylist)
                .frame(maxHeight: UIScreen.main.bounds.height * 0.5)
            Spacer()

            MusicStateView(controller: model.music)
            HStack {
                SleepTimerView(controller: model.music, startSleepTimer: model.startSleepTimer)
                ControllerStateView(controller: model.gstreamer, toggleOutput: model.toggleOutput)
            }
            .frame(minHeight: 75)
            .padding(.horizontal)
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
    @ObservedObject var controller: GStreamerController
    let toggleOutput: () -> Void

    var body: some View {
        AudioOutputStatusButton(audioState: controller.state, action: toggleOutput)
//
//            Text("Audio state: \(controller.state.description)")
//            Text("Output: \(controller.currentOutput)")
//            Text("Gstreamer message: \(controller.backendMessage)")
    }
}

struct SleepTimerView: View {
    @ObservedObject var controller: MusicController
    let startSleepTimer: (Int) -> Void

    var body: some View {
        Menu {
            if controller.status.sleep_timer != nil {
                Button("Cancel Timer") { startSleepTimer(0) }
            }

            Button("10 min") { startSleepTimer(10 * 60) }
            Button("15 min") { startSleepTimer(15 * 60) }
            Button("20 min") { startSleepTimer(20 * 60) }
            Button("25 min") { startSleepTimer(25 * 60) }
            Button("30 min") { startSleepTimer(30 * 60) }
        } label: {
            if let timer = controller.status.sleep_timer {
                VStack(spacing: 0) {
                    Image(systemName: "timer")
                        .foregroundColor(.indigo)
                        .font(.system(size: 32))
                    Text("\(Int(floor(Double(timer) / 60.0) + 1)) min")
                        .foregroundStyle(.white)
                        .font(.system(size: 10))
                        .fontWeight(.thin)
                }
            }
            else {
                Image(systemName: "timer")
                    .foregroundColor(.white.opacity(0.3))
                    .font(.system(size: 32))
            }
        }
    }
}

struct MusicStateView: View {
    @ObservedObject var controller: MusicController

    var body: some View {
        VStack {
            if let metadata = controller.status.metadata {

                VStack {
                    //                    if let url = URL(string: metadata.artwork_url) {
                    //                        RemoteImageView(imageUrl: url)
                    //                            .padding(.top, 8)
                    //                    }

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
                .padding(.top, 7)
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
//        .shadow(color: Color.lightForestGreen.opacity(0.2), radius: 10, x: 0, y: 0)
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
//        .overlay(
//            RoundedRectangle(cornerRadius: 8.0, style: .continuous)
//                .stroke(Color.black, lineWidth: 1)
//        )
    }
}

struct ActionRequestDto: Encodable {
    let action: String
}

struct PlaybackRequestDto: Encodable {
    let uri: String
}

struct SleepTimerRequestDto: Encodable {
    let timer: Int
}

struct Playlist: Identifiable {
    let id = UUID()
    let name: String
    let uri: String
    let image: URL?

    init(_ name: String, _ uri: String, _ image: URL? = nil) {
        self.name = name
        self.uri = uri
        self.image = image
    }
}

extension Color {
    static let veryDarkBlue = Color(red: 0.02, green: 0.08, blue: 0.15) // Very dark blue
    static let darkerBlue = Color(red: 0.03, green: 0.1, blue: 0.2) // Darker blue
    static let darkForestGreen = Color(red: 0.0, green: 0.27, blue: 0.13)
    static let lightForestGreen = Color(red: 0.13, green: 0.55, blue: 0.13)
    static let moonWhite = Color(red: 0.92, green: 0.92, blue: 0.88)
}

extension Color {
    init(hex: String) {
        let hex = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        let scanner = Scanner(string: hex)

        // Remove `#` if present
        if hex.hasPrefix("#") {
            scanner.currentIndex = hex.index(after: hex.startIndex)
        }

        var rgbValue: UInt64 = 0
        scanner.scanHexInt64(&rgbValue)

        let r, g, b, a: Double
        if hex.count == 6 {
            // RGB (24-bit)
            r = Double((rgbValue & 0xFF0000) >> 16) / 255.0
            g = Double((rgbValue & 0x00FF00) >> 8) / 255.0
            b = Double(rgbValue & 0x0000FF) / 255.0
            a = 1.0
        }
        else if hex.count == 8 {
            // RGBA (32-bit)
            r = Double((rgbValue & 0xFF000000) >> 24) / 255.0
            g = Double((rgbValue & 0x00FF0000) >> 16) / 255.0
            b = Double((rgbValue & 0x0000FF00) >> 8) / 255.0
            a = Double(rgbValue & 0x000000FF) / 255.0
        }
        else {
            // Default to white if the format is incorrect
            r = 1.0
            g = 1.0
            b = 1.0
            a = 1.0
        }

        self.init(.sRGB, red: r, green: g, blue: b, opacity: a)
    }
}

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

struct AudioOutputStatusButton: View {
    var audioState: AudioState
    let action: () -> Void

    var body: some View {
        HStack {
            Spacer()
            Button(action: action) {
                Image(systemName: audioState == .playing ? "speaker.wave.2" : "speaker.slash.fill")
                    .symbolRenderingMode(.palette)
                    .foregroundStyle(Color.moonWhite, Color.moonWhite, Color.moonWhite)
                    .font(.system(size: 32))
            }
        }
    }
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
