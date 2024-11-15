import SwiftUI

@resultBuilder
enum TabContentViewBuilder {
    static func buildBlock(_ components: Tab...) -> [Tab] {
        components
    }
}

struct Tab: View, Identifiable {
    let id = UUID()
    let title: String
    let view: AnyView

    init<Content: View>(title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.view = AnyView(content())
    }

    var body: some View {
        view
    }
}

struct TabPanel: View {
    let tabs: [Tab]
    @State var selectedTab: Int = 0

    init(@TabContentViewBuilder content: () -> [Tab]) {
        self.tabs = content()
    }

    var body: some View {
        VStack(spacing: 0) {
            tabs[selectedTab].view

            Spacer(minLength: 10)

            HStack {
                ForEach(tabs.indices, id: \.self) { index in
                    Button(action: {
                        selectedTab = index
                    }) {
                        VStack {
                            Text(tabs[index].title)
                                .font(.headline)
                                .foregroundColor(selectedTab == index ? .orange : .white)
                        }
                        .frame(maxWidth: .infinity)
                    }
                }
            }
            .padding()
            .background(.white.opacity(0.1))
        }
        .background(
            LinearGradient(
                gradient: Gradient(colors: [.white.opacity(0.3), .white.opacity(0.1)]),
                startPoint: .top,
                endPoint: .bottom
            )
            .opacity(0.3)
            .clipShape(RoundedRectangle(cornerRadius: 20, style: .continuous))
        )
        .clipShape(RoundedRectangle(cornerRadius: 20, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .stroke(Color.white.opacity(0.2), lineWidth: 2)
        )
        .frame(maxWidth: .infinity)
        .padding(.bottom)
    }
}
