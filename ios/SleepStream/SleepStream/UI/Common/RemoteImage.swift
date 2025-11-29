import SwiftUI

struct RemoteImageView: View {
    let imageUrl: URL

    var body: some View {
        ZStack {
            Rectangle()
                .foregroundColor(.gray.opacity(0.15))
                .aspectRatio(1, contentMode: .fit)
            AsyncImage(url: imageUrl) { phase in
                switch phase {
                case .empty:
                    ProgressView()
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                case .success(let image):
                    image
                        .resizable()
                        .scaledToFill()
                        .clipped()
                case .failure:
                    Image(systemName: "photo")
                        .resizable()
                        .scaledToFit()
                        .foregroundColor(.gray)
                        .padding(12)
                @unknown default:
                    EmptyView()
                }
            }
            .aspectRatio(1, contentMode: .fit)
        }
    }
}
