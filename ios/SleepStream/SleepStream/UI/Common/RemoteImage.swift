import SwiftUI

struct RemoteImageView: View {
    let imageUrl: URL

    var body: some View {
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
        .clipShape(RoundedRectangle(cornerRadius: 8.0))
//        .overlay(
//            RoundedRectangle(cornerRadius: 8.0, style: .continuous)
//                .stroke(Color.black, lineWidth: 1)
//        )
    }
}
