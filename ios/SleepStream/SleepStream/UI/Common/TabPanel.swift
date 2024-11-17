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

            ZStack{
                tabs[selectedTab].view
                    .edgesIgnoringSafeArea(.init())
                VStack{
                    Spacer()
                    Rectangle()
                        .foregroundStyle(LinearGradient(colors: [Color.black.opacity(0), Color.black.opacity(0.6)], startPoint: .top, endPoint: .bottom))
                        .frame(maxWidth: .infinity, maxHeight: 6)
                }
                .allowsHitTesting(false)
            }

            Spacer(minLength: 0)

            HStack {
                ForEach(tabs.indices, id: \.self) { index in
                    Button(action: {
                        selectedTab = index
                    }) {
                        VStack {
                            Text(tabs[index].title)
                                .font(.headline)
                                .fontWeight(selectedTab == index ? .regular : .thin)
                                .foregroundColor(selectedTab == index ? .orange : .white)
                                .animation(.none)
                        }
                        .frame(maxWidth: .infinity)
                    }
                }
            }
            .padding()
            .background(Color.darkerBlue.opacity(0.8))
        }
        .background(Color.black.opacity(0.5)
            .edgesIgnoringSafeArea(.top)
        )
        .shadow(radius: 8)
        .frame(maxWidth: .infinity)
    }
    
//    func selectTab(id: Int){
//        DispatchQueue.main.async {
//            self.selectedTab = id
//        }
//    }
}

struct VisualEffect: UIViewRepresentable {
    var effect: UIVisualEffect?
    func makeUIView(context: UIViewRepresentableContext<Self>) -> UIVisualEffectView { UIVisualEffectView() }
    func updateUIView(_ uiView: UIVisualEffectView, context: UIViewRepresentableContext<Self>) { uiView.effect = effect }
}
