import SwiftUI

struct ShuffleButton: View {
    @ObservedObject var controller: MusicController
    let setShuffle: (Bool) -> Void

    var body: some View {
        Button(action: {
            self.setShuffle(!controller.status.shuffle)
        }) {
            Image(systemName: "shuffle")
                .foregroundColor(controller.status.shuffle ? Color.sleepTimerActiveButton : Color.sleepTimerInactiveButton)
                .font(.system(size: 32))
                .animation(Animation.easeInOut(duration: 0.4), value: controller.status.shuffle)
        }
    }
}
