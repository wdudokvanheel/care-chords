import SwiftUI

extension Path {
    func insetBy(_ inset: CGFloat) -> Path {
        return self.insetBy(inset, inset)
    }

    func insetBy(_ dx: CGFloat, _ dy: CGFloat) -> Path {
        let wMultiplier = 1.0 - (dx / self.boundingRect.width)
        let hMultiplier = 1.0 - (dy / self.boundingRect.height)
        let tr = CGAffineTransform(scaleX: wMultiplier, y: hMultiplier).translatedBy(x: dx * 0.5, y: dy * 0.5)
        return self.applying(tr)
    }
}
