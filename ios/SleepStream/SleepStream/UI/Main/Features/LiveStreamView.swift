import SwiftUI

struct LiveStreamView: View {
    @ObservedObject var controller: LiveStreamController
    @Environment(\.scenePhase) private var scenePhase
    @State var shouldResume = false
    
    var body: some View {
        VStack {
            GeometryReader { geometry in
                Spacer()
                ZStack {
                    Color.darkerBlue
                    Image(systemName: "hourglass")
                        .font(.system(size: 256))
                        .foregroundColor(.white)
                    UIViewWrapper(view: controller.view)
                        .frame(width: 2560, height: 1920)
                }
                .clipShape(RoundedRectangle(cornerRadius: 48.0))
                .onAppear {
                    self.controller.play()
                }
                .onDisappear {
                    self.controller.stop()
                }
                .scaleEffect(calculateScale(for: geometry.size))
                .frame(width: geometry.size.width, height: geometry.size.height)
                Spacer()
            }
            .padding(.horizontal)
        }
        .onChange(of: scenePhase) { newPhase in
            switch newPhase {
            case .active:
                if shouldResume {
                    self.controller.play()
                }
            case .inactive, .background:
                self.controller.stop()
                shouldResume = true
            @unknown default:
                break
            }
        }
    }

    private func calculateScale(for size: CGSize) -> CGFloat {
        let maxDimension = size.width
        let targetDimension = 2560.0
        return min(maxDimension / targetDimension, 1.0)
    }
}
