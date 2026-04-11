package com.oubli.wallet.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties

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
    Dialog(
        onDismissRequest = { if (dismissEnabled) onDismissRequest() },
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        Surface(
            modifier = Modifier.fillMaxSize(),
            color = MaterialTheme.colorScheme.background,
        ) {
            Column(
                modifier = Modifier.fillMaxSize(),
            ) {
                // Title bar
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
                // Content
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth(),
                ) {
                    content()
                }
                // Bottom bar
                if (bottomBar != null) {
                    HorizontalDivider()
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .background(MaterialTheme.colorScheme.surface)
                            .navigationBarsPadding()
                            .imePadding()
                            .padding(start = 24.dp, top = 16.dp, end = 24.dp, bottom = 16.dp),
                    ) {
                        bottomBar()
                    }
                }
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
