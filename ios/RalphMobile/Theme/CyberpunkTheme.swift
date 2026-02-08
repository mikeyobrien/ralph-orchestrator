import SwiftUI

/// Cyberpunk theme colors from V5 architecture specification
/// This is the single source of truth for all colors in the app
struct CyberpunkTheme {
    // MARK: - Background Colors (V5 Spec)

    /// Deepest black - main background
    static let bgPrimary = Color(hex: "#030306")
    /// Sidebar/headers
    static let bgSecondary = Color(hex: "#07070c")
    /// Elevated surfaces
    static let bgTertiary = Color(hex: "#0b0b12")
    /// Card backgrounds
    static let bgCard = Color(hex: "#09090f")
    /// Modal/elevated backgrounds
    static let bgElevated = Color(hex: "#0e0e16")
    /// Interactive hover state
    static let bgHover = Color(hex: "#12121a")

    // MARK: - Accent Colors (Neon - V5 Spec)

    /// Primary actions, running states - bright cyan
    static let accentCyan = Color(hex: "#00fff2")
    /// Tool calls, signals - neon magenta
    static let accentMagenta = Color(hex: "#ff00ff")
    /// Warnings, paused states - gold yellow
    static let accentYellow = Color(hex: "#ffd000")
    /// Secondary actions - orange
    static let accentOrange = Color(hex: "#ff6b00")
    /// Success, completed - neon green
    static let accentGreen = Color(hex: "#00ff88")
    /// Errors, stop actions - neon red
    static let accentRed = Color(hex: "#ff3366")
    /// System events - purple
    static let accentPurple = Color(hex: "#a855f7")

    // MARK: - Text Colors

    /// Main text - white
    static let textPrimary = Color.white
    /// Secondary info - light gray
    static let textSecondary = Color(hex: "#a0a0b0")
    /// Disabled/hints - muted gray
    static let textMuted = Color(hex: "#606070")

    // MARK: - Border

    /// Subtle borders
    static let border = Color(hex: "#1a1a2e")

    // MARK: - Status Colors

    /// Running status
    static var statusRunning: Color { accentCyan }
    /// Completed status
    static var statusCompleted: Color { accentGreen }
    /// Pending status
    static var statusPending: Color { accentYellow }
    /// Error status
    static var statusError: Color { accentRed }
    /// Paused status
    static var statusPaused: Color { accentOrange }

    // MARK: - Tool Call Colors

    /// bash tool
    static var toolBash: Color { accentCyan }
    /// read_file tool
    static var toolReadFile: Color { accentGreen }
    /// write_file tool
    static var toolWriteFile: Color { accentMagenta }
    /// edit_file tool
    static var toolEditFile: Color { accentOrange }
    /// search tool
    static var toolSearch: Color { accentYellow }
    /// mcp_tool
    static var toolMCP: Color { accentPurple }

    // MARK: - Hat Colors

    static func hatColor(for hat: String) -> Color {
        switch hat.lowercased() {
        case "planner", "📋 planner": return accentCyan
        case "builder", "🔨 builder", "⚙️ builder": return accentGreen
        case "reviewer", "👁️ reviewer", "⚖️ design critic": return accentYellow
        case "tester", "✅ validator": return accentOrange
        case "inquisitor", "🎯 inquisitor": return accentMagenta
        case "architect", "💭 architect": return accentPurple
        case "explorer", "🔍 explorer": return accentCyan
        case "committer", "📦 committer": return accentGreen
        case "task writer", "📝 task writer": return accentOrange
        default: return accentCyan
        }
    }
}

// MARK: - Color Extension for Hex Support

extension Color {
    init(hex: String) {
        let hex = hex.trimmingCharacters(in: CharacterSet.alphanumerics.inverted)
        var int: UInt64 = 0
        Scanner(string: hex).scanHexInt64(&int)
        let a, r, g, b: UInt64
        switch hex.count {
        case 3: // RGB (12-bit)
            (a, r, g, b) = (255, (int >> 8) * 17, (int >> 4 & 0xF) * 17, (int & 0xF) * 17)
        case 6: // RGB (24-bit)
            (a, r, g, b) = (255, int >> 16, int >> 8 & 0xFF, int & 0xFF)
        case 8: // ARGB (32-bit)
            (a, r, g, b) = (int >> 24, int >> 16 & 0xFF, int >> 8 & 0xFF, int & 0xFF)
        default:
            (a, r, g, b) = (255, 0, 0, 0)
        }
        self.init(
            .sRGB,
            red: Double(r) / 255,
            green: Double(g) / 255,
            blue: Double(b) / 255,
            opacity: Double(a) / 255
        )
    }
}

// MARK: - Glow Effect Modifier

struct GlowEffect: ViewModifier {
    let color: Color
    let radius: CGFloat
    let opacity: Double

    func body(content: Content) -> some View {
        content
            .shadow(color: color.opacity(opacity), radius: radius)
            .shadow(color: color.opacity(opacity * 0.5), radius: radius * 2)
    }
}

extension View {
    func glow(_ color: Color, radius: CGFloat = 10, opacity: Double = 0.5) -> some View {
        modifier(GlowEffect(color: color, radius: radius, opacity: opacity))
    }

    func subtleGlow(_ color: Color) -> some View {
        glow(color, radius: 5, opacity: 0.3)
    }

    func mediumGlow(_ color: Color) -> some View {
        glow(color, radius: 10, opacity: 0.5)
    }

    func intenseGlow(_ color: Color) -> some View {
        glow(color, radius: 15, opacity: 0.7)
    }
}

// MARK: - Pulsing Animation Modifier

struct PulsingGlow: ViewModifier {
    let color: Color
    @State private var isPulsing = false

    func body(content: Content) -> some View {
        content
            .shadow(color: color.opacity(isPulsing ? 0.6 : 0.2), radius: isPulsing ? 15 : 5)
            .animation(.easeInOut(duration: 1.5).repeatForever(autoreverses: true), value: isPulsing)
            .onAppear { isPulsing = true }
    }
}

extension View {
    func pulsingGlow(_ color: Color) -> some View {
        modifier(PulsingGlow(color: color))
    }
}

// MARK: - Cyberpunk Card Style

struct CyberpunkCardStyle: ViewModifier {
    var isHighlighted: Bool = false
    var accentColor: Color = CyberpunkTheme.accentCyan

    func body(content: Content) -> some View {
        content
            .padding()
            .background(CyberpunkTheme.bgCard)
            .cornerRadius(8)
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(
                        isHighlighted ? accentColor : CyberpunkTheme.border,
                        lineWidth: isHighlighted ? 1.5 : 1
                    )
            )
            .shadow(color: isHighlighted ? accentColor.opacity(0.3) : .clear, radius: 10)
    }
}

extension View {
    func cyberpunkCard(highlighted: Bool = false, accent: Color = CyberpunkTheme.accentCyan) -> some View {
        modifier(CyberpunkCardStyle(isHighlighted: highlighted, accentColor: accent))
    }
}

// MARK: - Scanline Overlay Effect

struct ScanlineOverlay: View {
    var body: some View {
        GeometryReader { geometry in
            Canvas { context, size in
                for y in stride(from: 0, to: size.height, by: 4) {
                    let rect = CGRect(x: 0, y: y, width: size.width, height: 1)
                    context.fill(Path(rect), with: .color(.black.opacity(0.03)))
                }
            }
        }
        .allowsHitTesting(false)
    }
}
