import SwiftUI

struct MusicControlsView: View {
    @ObservedObject var controller: MusicController
    @State var isAnimatingPlayButton = false

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
                        .frame(width: 32, height: 32)
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
                    ZStack {
                        Circle()
                            .foregroundStyle(controller.status.playing ? Color.orange.gradient : Color.white.gradient)
                            .frame(width: 72, height: 72)

                        Image(systemName: controller.status.playing ? "pause.fill" : "play.fill")
                            .resizable()
                            .scaledToFit()
                            .frame(width: 32, height: 32)
                            .foregroundColor(controller.status.playing ? .white : .black)
                            .padding(.leading, !controller.status.playing ? 8 : 0)
                            .shadow(radius: 4)
                    }
                    .onChange(of: controller.status.playing) { playing in
                        isAnimatingPlayButton = playing
                    }
                    .scaleEffect(isAnimatingPlayButton ? 1.1 : 1.0)
                    .animation(
                        isAnimatingPlayButton ?
                        Animation.easeInOut(duration: 1.6).repeatForever(autoreverses: true) :
                            .default,
                        value: isAnimatingPlayButton
                    )
                }
                .background {}

                Spacer()

                // Next Song Button
                Button(action: {
                    controller.next()
                }) {
                    Image(systemName: "forward.fill")
                        .resizable()
                        .scaledToFit()
                        .frame(width: 32, height: 32)
                        .foregroundColor(.white)
                }

                Spacer()
            }
            .padding(.bottom)
        }
        .frame(maxWidth: .infinity)
//        .shadow(color: Color.lightForestGreen.opacity(0.2), radius: 10, x: 0, y: 0)
        .padding()
    }
}
