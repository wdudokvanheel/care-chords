import SwiftUI

extension Color {
    static let veryDarkBlue = Color(red: 0.02, green: 0.08, blue: 0.15) // Very dark blue
    static let darkerBlue = Color(red: 0.03, green: 0.1, blue: 0.2) // Darker blue
    static let darkForestGreen = Color(red: 0.0, green: 0.27, blue: 0.13)
    static let lightForestGreen = Color(red: 0.13, green: 0.55, blue: 0.13)
    static let moonWhite = Color(red: 0.92, green: 0.92, blue: 0.88)
}

extension Color {
    init(hex: String) {
        let hex = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        let scanner = Scanner(string: hex)

        // Remove `#` if present
        if hex.hasPrefix("#") {
            scanner.currentIndex = hex.index(after: hex.startIndex)
        }

        var rgbValue: UInt64 = 0
        scanner.scanHexInt64(&rgbValue)

        let r, g, b, a: Double
        if hex.count == 6 {
            // RGB (24-bit)
            r = Double((rgbValue & 0xFF0000) >> 16) / 255.0
            g = Double((rgbValue & 0x00FF00) >> 8) / 255.0
            b = Double(rgbValue & 0x0000FF) / 255.0
            a = 1.0
        }
        else if hex.count == 8 {
            // RGBA (32-bit)
            r = Double((rgbValue & 0xFF000000) >> 24) / 255.0
            g = Double((rgbValue & 0x00FF0000) >> 16) / 255.0
            b = Double((rgbValue & 0x0000FF00) >> 8) / 255.0
            a = Double(rgbValue & 0x000000FF) / 255.0
        }
        else {
            // Default to white if the format is incorrect
            r = 1.0
            g = 1.0
            b = 1.0
            a = 1.0
        }

        self.init(.sRGB, red: r, green: g, blue: b, opacity: a)
    }
}
