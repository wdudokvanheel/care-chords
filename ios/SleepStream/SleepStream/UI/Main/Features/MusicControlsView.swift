import SwiftUI

struct MusicControlsView: View {
    @ObservedObject var musicController: MusicController
    @ObservedObject var gstreamerController: GStreamerController

    let toggleMute: () -> Void
    let startSleepTimer: (Int) -> Void
    let setShuffle: (Bool) -> Void

    @State var isAnimatingPlayButton = false

    var body: some View {
        VStack {
            HStack(alignment: .top) {
                VStack {
                    MuteButton(audioState: gstreamerController.state, action: toggleMute)
                    Spacer()
                }
                Spacer()
            }
            .frame(height: 65)

            HStack(alignment: .center) {
                ShuffleButton(controller: musicController, setShuffle: setShuffle)
                Spacer()

                // Previous Song Button
                Button(action: {
                    musicController.previous()
                }) {
                    Image(systemName: "backward.end")
                        .resizable()
                        .scaledToFit()
                        .frame(width: 32, height: 32)
                        .foregroundColor(Color.prevNextButton)
                }
                Spacer()

                // Play/Pause Button
                Button(action: {
                    if musicController.status.playing {
                        musicController.pause()
                    }
                    else {
                        musicController.play()
                    }
                }) {
                    ZStack {
                        Circle()
                            .foregroundStyle(musicController.status.playing ? Color.playButtonActive : Color.playButtonInactive)
                            .frame(width: 72, height: 72)

                        Image(systemName: musicController.status.playing ? "pause.fill" : "play.fill")
                            .resizable()
                            .scaledToFit()
                            .frame(width: 32, height: 32)
                            .foregroundColor(musicController.status.playing ? Color.playButtonActiveLabel : Color.playButtonInactiveLabel)
                            .padding(.leading, !musicController.status.playing ? 8 : 0)
                            .shadow(radius: 4)
                    }
                    .onChange(of: musicController.status.playing) { playing in
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
                    musicController.next()
                }) {
                    Image(systemName: "forward.end")
                        .resizable()
                        .scaledToFit()
                        .frame(width: 32, height: 32)
                        .foregroundColor(Color.prevNextButton)
                }

                Spacer()
                SleepTimerButton(controller: musicController, startSleepTimer: startSleepTimer)
            }
        }
        .frame(maxWidth: .infinity)
        .padding()
    }
}
