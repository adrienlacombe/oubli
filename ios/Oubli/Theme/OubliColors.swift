import SwiftUI

// MARK: - Oubli Obsidian Design System Colors

extension Color {
    // MARK: Surface

    static let oubliBackground = Color(hex: 0x131313)
    static let oubliSurface = Color(hex: 0x131313)
    static let oubliSurfaceDim = Color(hex: 0x131313)
    static let oubliSurfaceBright = Color(hex: 0x3A3939)
    static let oubliSurfaceContainerLowest = Color(hex: 0x0E0E0E)
    static let oubliSurfaceContainerLow = Color(hex: 0x1C1B1B)
    static let oubliSurfaceContainer = Color(hex: 0x201F1F)
    static let oubliSurfaceContainerHigh = Color(hex: 0x2A2A2A)
    static let oubliSurfaceContainerHighest = Color(hex: 0x353534)
    static let oubliSurfaceVariant = Color(hex: 0x353534)

    // MARK: Primary (Blue)

    static let oubliPrimary = Color(hex: 0xADC6FF)
    static let oubliPrimaryContainer = Color(hex: 0x80AAFF)
    static let oubliOnPrimary = Color(hex: 0x002E69)
    static let oubliOnPrimaryContainer = Color(hex: 0x003D85)

    // MARK: Secondary (Bitcoin Orange)

    static let oubliSecondary = Color(hex: 0xFFB874)
    static let oubliSecondaryContainer = Color(hex: 0xE78603)
    static let oubliOnSecondary = Color(hex: 0x4B2800)
    static let oubliOnSecondaryContainer = Color(hex: 0x522C00)

    // MARK: Tertiary

    static let oubliTertiary = Color(hex: 0xC7C6CB)
    static let oubliTertiaryContainer = Color(hex: 0xABABB0)
    static let oubliOnTertiary = Color(hex: 0x2F3034)

    // MARK: Error

    static let oubliError = Color(hex: 0xFFB4AB)
    static let oubliErrorContainer = Color(hex: 0x93000A)
    static let oubliOnError = Color(hex: 0x690005)
    static let oubliOnErrorContainer = Color(hex: 0xFFDAD6)

    // MARK: Text / On-Surface

    static let oubliOnSurface = Color(hex: 0xE5E2E1)
    static let oubliOnSurfaceVariant = Color(hex: 0xDBC2AE)
    static let oubliOnBackground = Color(hex: 0xE5E2E1)

    // MARK: Outline

    static let oubliOutline = Color(hex: 0xA38D7B)
    static let oubliOutlineVariant = Color(hex: 0x554335)

    // MARK: Inverse

    static let oubliInverseSurface = Color(hex: 0xE5E2E1)
    static let oubliInverseOnSurface = Color(hex: 0x313030)
    static let oubliInversePrimary = Color(hex: 0x005BC1)

    // MARK: Semantic Aliases

    static let oubliReceived = Color(hex: 0x4CAF50)
    static let oubliSent = Color(hex: 0xE0A89E)
    static let oubliPending = Color(hex: 0xFFB874)
}

// MARK: - Hex Initializer

extension Color {
    init(hex: UInt, opacity: Double = 1.0) {
        self.init(
            .sRGB,
            red: Double((hex >> 16) & 0xFF) / 255.0,
            green: Double((hex >> 8) & 0xFF) / 255.0,
            blue: Double(hex & 0xFF) / 255.0,
            opacity: opacity
        )
    }
}
