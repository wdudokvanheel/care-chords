import SwiftUI

struct RemoteImageView: View {
    let imageUrl: URL

    var body: some View {
        ZStack {
            Rectangle()
                .aspectRatio(1.0, contentMode: .fit)
                .foregroundColor(.gray.opacity(0.3))
            AsyncImage(url: imageUrl) { phase in
                switch phase {
                case .empty:
                    ProgressView()
                case .success(let image):
                    image
                        .resizable()
                        .scaledToFit()
                case .failure:
                    Image(systemName: "photo")
                        .resizable()
                        .scaledToFit()
                        .foregroundColor(.gray)
                @unknown default:
                    EmptyView()
                }
            }
        }
    }
}
