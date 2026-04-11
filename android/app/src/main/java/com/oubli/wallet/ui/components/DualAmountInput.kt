package com.oubli.wallet.ui.components

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp

/**
 * Bidirectionally synced sats/fiat amount input.
 * Typing in either field updates the other in real time.
 */
@Composable
fun DualAmountInput(
    satsAmount: String,
    onSatsChange: (String) -> Unit,
    satsToFiatRaw: (String) -> String?,
    fiatToSats: (String) -> String?,
    fiatCurrency: String,
    fiatSymbol: String,
    readOnly: Boolean = false,
    showMaxButton: Boolean = false,
    maxSats: String? = null,
) {
    // Track which field the user is actively editing to prevent feedback loops
    var editingFiat by rememberSaveable { mutableStateOf(false) }
    var fiatAmount by rememberSaveable { mutableStateOf("") }

    val hasFiatPrice = satsToFiatRaw("100000") != null

    // Sats field
    OutlinedTextField(
        value = satsAmount,
        onValueChange = { newValue ->
            val filtered = newValue.filter { it.isDigit() }
            editingFiat = false
            onSatsChange(filtered)
            if (filtered.isNotEmpty()) {
                satsToFiatRaw(filtered)?.let { fiatAmount = it }
            } else {
                fiatAmount = ""
            }
        },
        label = { Text("Amount (sats)") },
        modifier = Modifier.fillMaxWidth(),
        singleLine = true,
        readOnly = readOnly,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
        trailingIcon = {
            if (showMaxButton && !readOnly && maxSats != null) {
                TextButton(onClick = {
                    onSatsChange(maxSats)
                    satsToFiatRaw(maxSats)?.let { fiatAmount = it }
                }) {
                    Text("Max")
                }
            }
        },
    )

    // Fiat field
    if (hasFiatPrice) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.fillMaxWidth(),
        ) {
            OutlinedTextField(
                value = fiatAmount,
                onValueChange = { newValue ->
                    val filtered = newValue.filter { it.isDigit() || it == '.' }
                    editingFiat = true
                    fiatAmount = filtered
                    if (filtered.isNotEmpty()) {
                        fiatToSats(filtered)?.let { onSatsChange(it) }
                    } else {
                        onSatsChange("")
                    }
                },
                label = { Text("Amount ($fiatSymbol${fiatCurrency.uppercase()})") },
                modifier = Modifier.weight(1f),
                singleLine = true,
                readOnly = readOnly,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
            )
        }
    } else {
        Text(
            text = "Fiat price unavailable",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
