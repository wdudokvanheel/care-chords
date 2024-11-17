import SwiftUI

struct RoundCornerRectangle: Shape {
    var cornerRadius: CGFloat
    var roundedCorners: UIRectCorner = .allCorners
    var inset: CGFloat = 0

    func path(in rect: CGRect) -> Path {
        let path = UIBezierPath(roundedRect: rect, byRoundingCorners: roundedCorners, cornerRadii: CGSize(width: cornerRadius, height: cornerRadius))
        return Path(path.cgPath).insetBy(self.inset)
    }
}
