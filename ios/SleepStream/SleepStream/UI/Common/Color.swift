import SwiftUI

extension Color {
    static let appBackground = LinearGradient(
        gradient: Gradient(colors: [Color.darkerBlue, Color.veryDarkBlue]),
        startPoint: .top,
        endPoint: .bottom
    )

    static let playButtonActive = Color.orange
    static let playButtonActiveLabel = Color.white
    static let playButtonInactive = Color.white
    static let playButtonInactiveLabel = Color.black
    static let prevNextButton = Color.white

    static let sleepTimerInactiveButton = Color.white.opacity(0.3)
    static let sleepTimerActiveButton = Color.orange
    static let sleepTimerLabel = Color.white.opacity(0.8)

    static let muteButtonActive = Color.orange
    static let muteButtonInactive = Color.white.opacity(0.3)

    static let playlistItem = darkerBlue
    static let playlistItemLabel = Color.white.opacity(0.75)

    static let veryDarkBlue = Color(red: 0.02, green: 0.08, blue: 0.15)
    static let darkerBlue = Color(red: 0.03, green: 0.1, blue: 0.2)
    static let moonWhite = Color(red: 0.92, green: 0.92, blue: 0.88)
}

extension Color {
    init(hex: String) {
        let hex = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        let scanner = Scanner(string: hex)

        if hex.hasPrefix("#") {
            scanner.currentIndex = hex.index(after: hex.startIndex)
        }

        var rgbValue: UInt64 = 0
        scanner.scanHexInt64(&rgbValue)

        let r, g, b, a: Double
        if hex.count == 6 {
            r = Double((rgbValue & 0xFF0000) >> 16) / 255.0
            g = Double((rgbValue & 0x00FF00) >> 8) / 255.0
            b = Double(rgbValue & 0x0000FF) / 255.0
            a = 1.0
        }
        else if hex.count == 8 {
            r = Double((rgbValue & 0xFF000000) >> 24) / 255.0
            g = Double((rgbValue & 0x00FF0000) >> 16) / 255.0
            b = Double((rgbValue & 0x0000FF00) >> 8) / 255.0
            a = Double(rgbValue & 0x000000FF) / 255.0
        }
        else {
            r = 1.0
            g = 1.0
            b = 1.0
            a = 1.0
        }

        self.init(.sRGB, red: r, green: g, blue: b, opacity: a)
    }
}
