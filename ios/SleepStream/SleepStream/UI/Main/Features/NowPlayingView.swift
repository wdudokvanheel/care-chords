import SwiftUI

struct NowPlayingView: View {
    @ObservedObject var controller: MusicController
    
    var body: some View {
        VStack {
            if let metadata = controller.status.metadata {
                if let url = URL(string: metadata.artwork_url) {
                    RemoteImageView(imageUrl: url)
                        .clipShape(RoundedRectangle(cornerRadius: 8))
                        .padding([.top, .horizontal])
                }
                        
                Spacer()
                        
                VStack {
                    Text(metadata.title)
                        .foregroundStyle(.white)
                        .fontWeight(.bold)
                        .font(.title2)
                        .multilineTextAlignment(.center)
                        .lineLimit(1)
                    Text(metadata.artist)
                        .foregroundStyle(.white)
                        .opacity(0.9)
                        .font(.title3)
                        .fontWeight(.light)
                }
                .padding(.bottom, 8)
                        
                Spacer()
            }
            else {
                Spacer()
                
                Text("Nothing playing right now")
                    .frame(maxWidth: .infinity, alignment: .center)
                    .foregroundColor(.white.opacity(0.8))
                
                Spacer()
            }
        }
    }
}
