package com.oubli.wallet.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Backspace
import androidx.compose.material.icons.filled.Fingerprint
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp

@Composable
fun LockedScreen(
    unlockError: String?,
    hasPin: Boolean,
    onUnlockBiometric: () -> Unit,
    onUnlockPin: (String) -> Unit,
) {
    var hasFiredAutoBiometric by rememberSaveable { mutableStateOf(false) }
    var showPinEntry by rememberSaveable { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        if (!hasFiredAutoBiometric) {
            hasFiredAutoBiometric = true
            onUnlockBiometric()
        }
    }

    if (showPinEntry) {
        PinEntryScreen(
            error = unlockError,
            onSubmit = { pin -> onUnlockPin(pin) },
            onBack = { showPinEntry = false },
        )
    } else {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            Icon(
                imageVector = Icons.Filled.Lock,
                contentDescription = "Locked",
                modifier = Modifier.size(64.dp),
                tint = MaterialTheme.colorScheme.primary,
            )
            Spacer(modifier = Modifier.height(16.dp))
            Text(
                text = "Wallet Locked",
                style = MaterialTheme.typography.headlineMedium,
                fontWeight = FontWeight.Bold,
                modifier = Modifier.semantics { heading() },
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = "Authenticate to access your wallet.",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (unlockError != null) {
                Spacer(modifier = Modifier.height(16.dp))
                Text(
                    text = unlockError,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error,
                    textAlign = TextAlign.Center,
                )
            }
            Spacer(modifier = Modifier.height(32.dp))

            Button(
                onClick = onUnlockBiometric,
                modifier = Modifier.fillMaxWidth(0.6f),
            ) {
                Icon(
                    imageVector = Icons.Filled.Fingerprint,
                    contentDescription = null,
                    modifier = Modifier.size(20.dp),
                )
                Spacer(modifier = Modifier.size(8.dp))
                Text("Unlock with Biometric")
            }

            if (hasPin) {
                Spacer(modifier = Modifier.height(12.dp))
                OutlinedButton(
                    onClick = { showPinEntry = true },
                    modifier = Modifier.fillMaxWidth(0.6f),
                ) {
                    Text("Use PIN")
                }
            }
        }
    }
}

@Composable
private fun PinEntryScreen(
    error: String?,
    onSubmit: (String) -> Unit,
    onBack: () -> Unit,
) {
    var pin by rememberSaveable { mutableStateOf("") }
    val maxLen = 6

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            text = "Enter PIN",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.semantics { heading() },
        )
        Spacer(modifier = Modifier.height(24.dp))

        // PIN dots
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            for (i in 0 until maxLen) {
                Text(
                    text = if (i < pin.length) "\u25CF" else "\u25CB",
                    style = MaterialTheme.typography.headlineMedium,
                    color = if (i < pin.length) MaterialTheme.colorScheme.primary
                    else MaterialTheme.colorScheme.outlineVariant,
                )
            }
        }

        if (error != null) {
            Spacer(modifier = Modifier.height(12.dp))
            Text(
                text = error,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
                textAlign = TextAlign.Center,
            )
        }

        Spacer(modifier = Modifier.height(32.dp))

        // Number pad
        val rows = listOf(
            listOf("1", "2", "3"),
            listOf("4", "5", "6"),
            listOf("7", "8", "9"),
            listOf("", "0", "del"),
        )
        for (row in rows) {
            Row(
                modifier = Modifier.fillMaxWidth(0.7f),
                horizontalArrangement = Arrangement.SpaceEvenly,
            ) {
                for (key in row) {
                    when (key) {
                        "" -> Spacer(modifier = Modifier.size(64.dp))
                        "del" -> IconButton(
                            onClick = { if (pin.isNotEmpty()) pin = pin.dropLast(1) },
                            modifier = Modifier.size(64.dp),
                        ) {
                            Icon(Icons.Filled.Backspace, contentDescription = "Delete")
                        }
                        else -> FilledTonalButton(
                            onClick = {
                                if (pin.length < maxLen) {
                                    pin += key
                                    if (pin.length >= 4) {
                                        // Auto-submit when 4+ digits and user taps another
                                    }
                                }
                            },
                            modifier = Modifier.size(64.dp),
                        ) {
                            Text(key, style = MaterialTheme.typography.titleLarge)
                        }
                    }
                }
            }
            Spacer(modifier = Modifier.height(8.dp))
        }

        Spacer(modifier = Modifier.height(16.dp))
        Button(
            onClick = { onSubmit(pin); pin = "" },
            enabled = pin.length >= 4,
            modifier = Modifier.fillMaxWidth(0.7f),
        ) {
            Text("Unlock")
        }
        Spacer(modifier = Modifier.height(8.dp))
        TextButton(onClick = onBack) {
            Text("Back")
        }
    }
}
