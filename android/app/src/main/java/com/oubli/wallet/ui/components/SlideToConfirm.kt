package com.oubli.wallet.ui.components

import android.view.HapticFeedbackConstants
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt

/**
 * Slide-to-confirm gesture button.
 *
 * Replaces a tap + AlertDialog confirmation: dragging the thumb to the right
 * edge of the rail commits, anything less than that snaps back.
 *
 * Used in [SendDialog] to make a payment a single, deliberate, hard-to-mistap
 * gesture instead of two taps across two surfaces.
 */
@Composable
fun SlideToConfirm(
    label: String,
    enabled: Boolean = true,
    onConfirm: () -> Unit,
) {
    val view = LocalView.current
    val density = LocalDensity.current
    val thumbSize = 56.dp
    val railHeight = 64.dp
    val thumbSizePx = with(density) { thumbSize.toPx() }
    val railPaddingPx = with(density) { 4.dp.toPx() }

    BoxWithConstraints(
        modifier = Modifier
            .fillMaxWidth()
            .height(railHeight)
            .clip(RoundedCornerShape(railHeight / 2))
            .background(
                if (enabled) MaterialTheme.colorScheme.surfaceContainerHigh
                else MaterialTheme.colorScheme.surfaceContainerHighest,
            )
            .alpha(if (enabled) 1f else 0.5f),
    ) {
        val widthPx = with(density) { maxWidth.toPx() }
        val maxOffset = widthPx - thumbSizePx - railPaddingPx * 2
        var offsetX by remember { mutableFloatStateOf(0f) }
        var confirmed by remember { mutableStateOf(false) }

        // Animate offset back to 0 when released short, or to end on confirm.
        val animatedOffset by animateFloatAsState(
            targetValue = offsetX,
            label = "slideOffset",
        )

        LaunchedEffect(confirmed) {
            if (confirmed) {
                view.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
                onConfirm()
            }
        }

        Text(
            text = if (confirmed) "" else label,
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.Medium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier
                .align(Alignment.Center)
                .alpha(1f - (offsetX / maxOffset).coerceIn(0f, 1f)),
        )

        Box(
            modifier = Modifier
                .offset {
                    IntOffset(
                        x = (railPaddingPx + animatedOffset).roundToInt(),
                        y = 0,
                    )
                }
                .align(Alignment.CenterStart)
                .size(thumbSize)
                .clip(CircleShape)
                .background(MaterialTheme.colorScheme.primary)
                .pointerInput(enabled, maxOffset) {
                    if (!enabled) return@pointerInput
                    detectHorizontalDragGestures(
                        onDragStart = {
                            view.performHapticFeedback(HapticFeedbackConstants.GESTURE_START)
                        },
                        onDragEnd = {
                            if (offsetX >= maxOffset * 0.92f) {
                                offsetX = maxOffset
                                confirmed = true
                            } else {
                                offsetX = 0f
                            }
                        },
                        onDragCancel = { offsetX = 0f },
                        onHorizontalDrag = { _, delta ->
                            offsetX = (offsetX + delta).coerceIn(0f, maxOffset)
                        },
                    )
                },
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                imageVector = Icons.AutoMirrored.Filled.ArrowForward,
                contentDescription = label,
                tint = MaterialTheme.colorScheme.onPrimary,
            )
        }
    }
}
