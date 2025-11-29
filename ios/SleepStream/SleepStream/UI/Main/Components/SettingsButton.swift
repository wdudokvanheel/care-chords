import SwiftUI

struct SettingsButton: View {
    @State private var showSettings = false

    var body: some View {
        Button(action: {
            showSettings = true
        }) {
            Image(systemName: "gearshape.fill")
                .foregroundColor(Color.sleepTimerInactiveButton)
                .font(.system(size: 32))
        }
        .sheet(isPresented: $showSettings) {
            SettingsView()
        }
    }
}
