package com.oubli.wallet.ui

import androidx.compose.animation.AnimatedContent
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Check
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import kotlinx.coroutines.delay
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp

@Composable
fun OnboardingScreen(
    onGenerateMnemonic: (callback: (String) -> Unit) -> Unit,
    onValidateMnemonic: (phrase: String, callback: (Boolean) -> Unit) -> Unit,
    onSetPin: (pin: String, callback: (Boolean) -> Unit) -> Unit,
    onComplete: (mnemonic: String) -> Unit,
    onShowMessage: (String) -> Unit = {},
) {
    var step by rememberSaveable { mutableIntStateOf(0) }
    var mnemonic by rememberSaveable { mutableStateOf("") }
    var errorText by rememberSaveable { mutableStateOf<String?>(null) }

    AnimatedContent(
        targetState = step,
        label = "onboarding_step",
    ) { currentStep ->
        when (currentStep) {
            0 -> WelcomeStep(onGetStarted = { step = 1 }, onRestore = { step = 4 })
            1 -> DisclaimerStep(onContinue = {
                onGenerateMnemonic { generated ->
                    mnemonic = generated
                    step = 3
                }
            })
            3 -> MnemonicDisplayStep(
                mnemonic = mnemonic,
                onContinue = { step = 5 },
                onShowMessage = onShowMessage,
            )
            4 -> MnemonicRestoreStep(
                errorText = errorText,
                onValidate = { phrase ->
                    onValidateMnemonic(phrase) { valid ->
                        if (valid) {
                            errorText = null
                            mnemonic = phrase
                            step = 5
                        } else {
                            errorText = "Invalid seed phrase. Please check your words and try again."
                        }
                    }
                },
                onBack = {
                    errorText = null
                    step = 0
                },
            )
            5 -> PinSetupStep(
                onSetPin = { pin ->
                    onSetPin(pin) { success ->
                        if (success) onComplete(mnemonic)
                    }
                },
                onSkip = { onComplete(mnemonic) },
            )
        }
    }
}

@Composable
private fun WelcomeStep(onGetStarted: () -> Unit, onRestore: () -> Unit) {
    Column(
        modifier = Modifier.fillMaxSize().padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            text = "Oubli",
            style = MaterialTheme.typography.displayMedium,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.semantics { heading() },
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Your keys. Your Bitcoin. Secured by Starknet.",
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        Spacer(modifier = Modifier.height(48.dp))
        Button(onClick = onGetStarted, modifier = Modifier.fillMaxWidth()) {
            Text("Get Started")
        }
        Spacer(modifier = Modifier.height(12.dp))
        TextButton(onClick = onRestore) {
            Text("I already have a wallet")
        }
    }
}

@Composable
private fun DisclaimerStep(onContinue: () -> Unit) {
    var accepted by rememberSaveable { mutableStateOf(false) }

    Column(
        modifier = Modifier.fillMaxSize().padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.surfaceBright.copy(alpha = 0.4f),
            ),
        ) {
            Column(
                modifier = Modifier.padding(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Icon(
                    imageVector = Icons.Filled.Lock,
                    contentDescription = null,
                    modifier = Modifier.size(64.dp),
                    tint = MaterialTheme.colorScheme.primary,
                )
                Spacer(modifier = Modifier.height(16.dp))
                Text(
                    text = "You Are in Control",
                    style = MaterialTheme.typography.headlineSmall,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.semantics { heading() },
                )
                Spacer(modifier = Modifier.height(12.dp))
                Text(
                    text = "Oubli is a self-custodial wallet. You alone hold your private keys. No one \u2014 not even Oubli \u2014 can recover your funds if you lose your seed phrase. Make sure to back it up and store it safely.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                )
                Spacer(modifier = Modifier.height(24.dp))
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Checkbox(checked = accepted, onCheckedChange = { accepted = it })
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(
                        text = "I understand that I am responsible for keeping my seed phrase safe.",
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
            }
        }
        Spacer(modifier = Modifier.height(32.dp))
        Button(onClick = onContinue, modifier = Modifier.fillMaxWidth(), enabled = accepted) {
            Text("Continue")
        }
    }
}

@Composable
private fun MnemonicDisplayStep(mnemonic: String, onContinue: () -> Unit, onShowMessage: (String) -> Unit = {}) {
    val words = remember(mnemonic) { mnemonic.split(" ") }
    val clipboardManager = LocalClipboardManager.current
    val context = LocalContext.current
    var copied by remember { mutableStateOf(false) }
    var showClipboardWarning by remember { mutableStateOf(false) }

    LaunchedEffect(copied) {
        if (copied) {
            delay(2000)
            copied = false
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp)
            .verticalScroll(rememberScrollState()),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Spacer(modifier = Modifier.height(24.dp))
        Text(
            text = "Your Recovery Phrase",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.semantics { heading() },
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Write down these words in order. Never share them with anyone.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        Spacer(modifier = Modifier.height(24.dp))

        for (i in words.indices step 2) {
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                horizontalArrangement = Arrangement.SpaceEvenly,
            ) {
                WordChip(index = i + 1, word = words[i], modifier = Modifier.weight(1f))
                Spacer(modifier = Modifier.width(8.dp))
                if (i + 1 < words.size) {
                    WordChip(index = i + 2, word = words[i + 1], modifier = Modifier.weight(1f))
                } else {
                    Spacer(modifier = Modifier.weight(1f))
                }
            }
        }

        Spacer(modifier = Modifier.height(24.dp))
        OutlinedButton(
            onClick = { showClipboardWarning = true },
            modifier = Modifier.fillMaxWidth(),
        ) {
            Icon(
                imageVector = if (copied) Icons.Filled.Check else Icons.Filled.ContentCopy,
                contentDescription = null,
                modifier = Modifier.size(18.dp),
            )
            Spacer(modifier = Modifier.width(8.dp))
            Text(if (copied) "Copied!" else "Copy to Clipboard")
        }

        if (showClipboardWarning) {
            AlertDialog(
                onDismissRequest = { showClipboardWarning = false },
                title = { Text("Clipboard Warning") },
                text = {
                    Text("Your seed phrase will be copied to the clipboard, where other apps may be able to read it. Only do this if you intend to paste it immediately and clear your clipboard afterward.")
                },
                confirmButton = {
                    TextButton(onClick = {
                        showClipboardWarning = false
                        clipboardManager.setText(AnnotatedString(mnemonic))
                        copied = true
                        onShowMessage("Copied to clipboard")
                    }) {
                        Text("Copy Anyway")
                    }
                },
                dismissButton = {
                    TextButton(onClick = { showClipboardWarning = false }) {
                        Text("Cancel")
                    }
                },
            )
        }

        Spacer(modifier = Modifier.height(16.dp))
        Button(onClick = onContinue, modifier = Modifier.fillMaxWidth()) {
            Text("I've Written It Down")
        }
    }
}

@Composable
private fun WordChip(index: Int, word: String, modifier: Modifier = Modifier) {
    androidx.compose.material3.Card(
        modifier = modifier.semantics(mergeDescendants = true) {
            contentDescription = "Word $index: $word"
        },
        colors = androidx.compose.material3.CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant,
        ),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = String.format("%02d.", index),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.width(28.dp),
            )
            Text(
                text = word,
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.Medium,
            )
        }
    }
}

@Composable
private fun MnemonicRestoreStep(
    errorText: String?,
    onValidate: (String) -> Unit,
    onBack: () -> Unit,
) {
    var phraseInput by rememberSaveable { mutableStateOf("") }

    Column(
        modifier = Modifier.fillMaxSize().padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            text = "Restore Wallet",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.semantics { heading() },
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Enter your 12-word seed phrase, separated by spaces.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        Spacer(modifier = Modifier.height(24.dp))

        OutlinedTextField(
            value = phraseInput,
            onValueChange = { phraseInput = it.lowercase().trim() },
            label = { Text("Recovery Phrase") },
            modifier = Modifier.fillMaxWidth(),
            minLines = 3,
            maxLines = 5,
            keyboardOptions = KeyboardOptions(
                keyboardType = KeyboardType.Text,
                imeAction = ImeAction.Done,
            ),
            keyboardActions = KeyboardActions(onDone = { onValidate(phraseInput) }),
            isError = errorText != null,
            supportingText = errorText?.let { { Text(it) } },
        )

        Spacer(modifier = Modifier.height(24.dp))
        Button(
            onClick = { onValidate(phraseInput) },
            modifier = Modifier.fillMaxWidth(),
            enabled = phraseInput.split(" ").size >= 12,
        ) {
            Text("Restore Wallet")
        }
        Spacer(modifier = Modifier.height(8.dp))
        TextButton(onClick = onBack) {
            Text("Back")
        }
    }
}

@Composable
private fun PinSetupStep(
    onSetPin: (String) -> Unit,
    onSkip: () -> Unit,
) {
    var pin by rememberSaveable { mutableStateOf("") }
    var confirmPin by rememberSaveable { mutableStateOf("") }
    var confirming by rememberSaveable { mutableStateOf(false) }
    var error by rememberSaveable { mutableStateOf<String?>(null) }

    Column(
        modifier = Modifier.fillMaxSize().padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            text = if (confirming) "Confirm PIN" else "Set a PIN",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.semantics { heading() },
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = if (confirming) "Enter the same PIN again."
            else "Used as a fallback when biometrics aren't available.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        Spacer(modifier = Modifier.height(24.dp))

        val currentPin = if (confirming) confirmPin else pin

        // PIN dots
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            for (i in 0 until 6) {
                Text(
                    text = if (i < currentPin.length) "\u25CF" else "\u25CB",
                    style = MaterialTheme.typography.headlineMedium,
                    color = if (i < currentPin.length) MaterialTheme.colorScheme.primary
                    else MaterialTheme.colorScheme.outlineVariant,
                )
            }
        }

        if (error != null) {
            Spacer(modifier = Modifier.height(12.dp))
            Text(
                text = error!!,
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
                        "del" -> androidx.compose.material3.IconButton(
                            onClick = {
                                if (confirming) {
                                    if (confirmPin.isNotEmpty()) confirmPin = confirmPin.dropLast(1)
                                } else {
                                    if (pin.isNotEmpty()) pin = pin.dropLast(1)
                                }
                            },
                            modifier = Modifier.size(64.dp),
                        ) {
                            Icon(
                                Icons.Filled.Backspace,
                                contentDescription = "Delete",
                            )
                        }
                        else -> androidx.compose.material3.FilledTonalButton(
                            onClick = {
                                if (confirming) {
                                    if (confirmPin.length < 6) confirmPin += key
                                } else {
                                    if (pin.length < 6) pin += key
                                }
                                error = null
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
            onClick = {
                if (!confirming) {
                    confirming = true
                } else {
                    if (confirmPin == pin) {
                        onSetPin(pin)
                    } else {
                        error = "PINs don't match. Try again."
                        confirmPin = ""
                    }
                }
            },
            enabled = currentPin.length >= 4,
            modifier = Modifier.fillMaxWidth(0.7f),
        ) {
            Text(if (confirming) "Confirm" else "Continue")
        }

        Spacer(modifier = Modifier.height(8.dp))
        TextButton(onClick = onSkip) {
            Text("Skip")
        }
    }
}
