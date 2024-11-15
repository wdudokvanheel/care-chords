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
            VStack(spacing: 0) {
                Image(systemName: "timer")
                    .foregroundColor(controller.status.sleep_timer == nil ? Color.sleepTimerInactiveButton : Color.sleepTimerActiveButton)
                    .font(.system(size: 32))
                    .animation(Animation.easeInOut(duration: 0.4), value: controller.status.sleep_timer)
                if let timer = controller.status.sleep_timer {
                    Text("\(Int(floor(Double(timer) / 60.0) + 1)) min")
                        .foregroundStyle(Color.sleepTimerLabel)
                        .font(.system(size: 10))
                        .fontWeight(.thin)
                }
                Spacer()
            }
        }
    }
}
