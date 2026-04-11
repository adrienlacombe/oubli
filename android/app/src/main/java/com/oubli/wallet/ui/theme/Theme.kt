package com.oubli.wallet.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

// Oubli Obsidian Design System

// Surface hierarchy
val OubliBackground = Color(0xFF131313)
val OubliSurface = Color(0xFF131313)
val OubliSurfaceDim = Color(0xFF131313)
val OubliSurfaceBright = Color(0xFF3A3939)
val OubliSurfaceContainerLowest = Color(0xFF0E0E0E)
val OubliSurfaceContainerLow = Color(0xFF1C1B1B)
val OubliSurfaceContainer = Color(0xFF201F1F)
val OubliSurfaceContainerHigh = Color(0xFF2A2A2A)
val OubliSurfaceContainerHighest = Color(0xFF353534)
val OubliSurfaceVariant = Color(0xFF353534)

// Primary (Blue)
val OubliPrimary = Color(0xFFADC6FF)
val OubliPrimaryContainer = Color(0xFF80AAFF)
val OubliOnPrimary = Color(0xFF002E69)
val OubliOnPrimaryContainer = Color(0xFF003D85)

// Secondary (Bitcoin Orange)
val OubliSecondary = Color(0xFFFFB874)
val OubliSecondaryContainer = Color(0xFFE78603)
val OubliOnSecondary = Color(0xFF4B2800)
val OubliOnSecondaryContainer = Color(0xFF522C00)

// Tertiary
val OubliTertiary = Color(0xFFC7C6CB)
val OubliTertiaryContainer = Color(0xFFABABB0)
val OubliOnTertiary = Color(0xFF2F3034)

// Error
val OubliError = Color(0xFFFFB4AB)
val OubliErrorContainer = Color(0xFF93000A)
val OubliOnError = Color(0xFF690005)
val OubliOnErrorContainer = Color(0xFFFFDAD6)

// On-Surface / Text
val OubliOnSurface = Color(0xFFE5E2E1)
val OubliOnSurfaceVariant = Color(0xFFDBC2AE)
val OubliOnBackground = Color(0xFFE5E2E1)

// Outline
val OubliOutline = Color(0xFFA38D7B)
val OubliOutlineVariant = Color(0xFF554335)

// Inverse
val OubliInverseSurface = Color(0xFFE5E2E1)
val OubliInverseOnSurface = Color(0xFF313030)
val OubliInversePrimary = Color(0xFF005BC1)

// Semantic
val OubliReceived = Color(0xFF4CAF50)
val OubliSent = Color(0xFFE0A89E)
val OubliPending = Color(0xFFFFB874)
val OubliSuccessBg = Color(0xFF1B3A1B)
val OubliErrorBg = Color(0xFF3A1B1B)

private val OubliDarkColorScheme = darkColorScheme(
    primary = OubliPrimary,
    onPrimary = OubliOnPrimary,
    primaryContainer = OubliPrimaryContainer,
    onPrimaryContainer = OubliOnPrimaryContainer,
    secondary = OubliSecondary,
    onSecondary = OubliOnSecondary,
    secondaryContainer = OubliSecondaryContainer,
    onSecondaryContainer = OubliOnSecondaryContainer,
    tertiary = OubliTertiary,
    onTertiary = OubliOnTertiary,
    tertiaryContainer = OubliTertiaryContainer,
    error = OubliError,
    onError = OubliOnError,
    errorContainer = OubliErrorContainer,
    onErrorContainer = OubliOnErrorContainer,
    background = OubliBackground,
    onBackground = OubliOnBackground,
    surface = OubliSurface,
    onSurface = OubliOnSurface,
    surfaceVariant = OubliSurfaceVariant,
    onSurfaceVariant = OubliOnSurfaceVariant,
    outline = OubliOutline,
    outlineVariant = OubliOutlineVariant,
    inverseSurface = OubliInverseSurface,
    inverseOnSurface = OubliInverseOnSurface,
    inversePrimary = OubliInversePrimary,
    surfaceDim = OubliSurfaceDim,
    surfaceBright = OubliSurfaceBright,
    surfaceContainerLowest = OubliSurfaceContainerLowest,
    surfaceContainerLow = OubliSurfaceContainerLow,
    surfaceContainer = OubliSurfaceContainer,
    surfaceContainerHigh = OubliSurfaceContainerHigh,
    surfaceContainerHighest = OubliSurfaceContainerHighest,
)

@Composable
fun OubliTheme(
    content: @Composable () -> Unit,
) {
    MaterialTheme(
        colorScheme = OubliDarkColorScheme,
        content = content,
    )
}
