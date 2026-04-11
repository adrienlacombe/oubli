package com.oubli.wallet.ui.components

import android.content.res.Resources
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties

/**
 * Navigation bar height for Dialog windows.
 *
 * Full-screen Dialogs on Android 15+ (API 35) don't dispatch navigation bar
 * insets to Compose, so `navigationBarsPadding()` returns 0. We read the
 * system resource and enforce a minimum so the bottom bar is never clipped
 * — even on custom ROMs (e.g. GrapheneOS) that report smaller values.
 */
@Composable
private fun dialogNavBarHeight(): Dp {
    val density = LocalDensity.current
    return remember {
        val res = Resources.getSystem()
        val id = res.getIdentifier("navigation_bar_height", "dimen", "android")
        val system = if (id > 0) with(density) { res.getDimensionPixelSize(id).toDp() } else 0.dp
        maxOf(system, 48.dp) + 16.dp
    }
}

@Composable
fun FullScreenTaskDialog(
    title: String,
    onDismissRequest: () -> Unit,
    dismissEnabled: Boolean = true,
    leadingActionLabel: String = "Close",
    leadingAction: (() -> Unit)? = null,
    bottomBar: (@Composable () -> Unit)? = null,
    content: @Composable BoxScope.() -> Unit,
) {
    val navBarHeight = dialogNavBarHeight()

    Dialog(
        onDismissRequest = { if (dismissEnabled) onDismissRequest() },
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        Scaffold(
            modifier = Modifier.fillMaxSize(),
            containerColor = MaterialTheme.colorScheme.background,
            contentWindowInsets = WindowInsets(0, 0, 0, 0),
            topBar = {
                Column {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .statusBarsPadding()
                            .padding(horizontal = 8.dp, vertical = 8.dp),
                    ) {
                        Text(
                            text = title,
                            style = MaterialTheme.typography.titleLarge,
                            fontWeight = FontWeight.SemiBold,
                            modifier = Modifier.align(Alignment.Center),
                        )
                        TextButton(
                            onClick = { (leadingAction ?: onDismissRequest)() },
                            enabled = dismissEnabled,
                            modifier = Modifier.align(Alignment.CenterStart),
                        ) {
                            Text(leadingActionLabel)
                        }
                    }
                    HorizontalDivider()
                }
            },
            bottomBar = {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(
                            if (bottomBar != null) MaterialTheme.colorScheme.surface
                            else MaterialTheme.colorScheme.background,
                        ),
                ) {
                    if (bottomBar != null) {
                        HorizontalDivider()
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .imePadding()
                                .padding(start = 24.dp, top = 16.dp, end = 24.dp, bottom = 16.dp),
                        ) {
                            bottomBar()
                        }
                    }
                    Spacer(modifier = Modifier.height(navBarHeight))
                }
            },
        ) { paddingValues ->
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(paddingValues),
            ) {
                content()
            }
        }
    }
}

@Composable
fun TaskPrimaryButton(
    title: String,
    enabled: Boolean = true,
    trailingIcon: @Composable (() -> Unit)? = null,
    onClick: () -> Unit,
) {
    Button(
        onClick = onClick,
        enabled = enabled,
        modifier = Modifier
            .fillMaxWidth()
            .height(56.dp),
        colors = ButtonDefaults.buttonColors(
            containerColor = MaterialTheme.colorScheme.primary,
            contentColor = MaterialTheme.colorScheme.onPrimary,
            disabledContainerColor = MaterialTheme.colorScheme.surfaceContainerHighest,
            disabledContentColor = MaterialTheme.colorScheme.onSurfaceVariant,
        ),
    ) {
        Text(title)
        if (trailingIcon != null) {
            androidx.compose.foundation.layout.Spacer(modifier = Modifier.padding(start = 4.dp))
            trailingIcon()
        }
    }
}
