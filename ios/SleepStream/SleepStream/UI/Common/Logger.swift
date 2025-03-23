import Foundation
import os

extension Logger {
    private static var subsystem = Bundle.main.bundleIdentifier!

    public static func new(_ category: String) -> Self {
        return Logger(subsystem: Logger.subsystem, category: category)
    }
}
