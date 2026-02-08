import Foundation

/// Time formatting utilities for consistent display across the app
enum TimeFormatting {
    /// Formats seconds into HH:MM:SS format
    /// - Parameter seconds: Total seconds to format
    /// - Returns: Formatted string like "00:23:45"
    static func formatTime(_ seconds: Int) -> String {
        let hours = seconds / 3600
        let minutes = (seconds % 3600) / 60
        let secs = seconds % 60
        return String(format: "%02d:%02d:%02d", hours, minutes, secs)
    }

    /// Formats seconds into MM:SS format (short form)
    /// - Parameter seconds: Total seconds to format
    /// - Returns: Formatted string like "23:45"
    static func formatElapsed(_ seconds: Int) -> String {
        let minutes = seconds / 60
        let secs = seconds % 60
        return String(format: "%02d:%02d", minutes, secs)
    }
}
