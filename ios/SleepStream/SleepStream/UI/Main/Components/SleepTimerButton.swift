import SwiftUI

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
