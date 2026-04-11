package com.oubli.wallet.ui.balance

import android.content.ClipboardManager
import android.content.Context
import androidx.compose.animation.core.EaseOutCubic
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Bolt
import androidx.compose.material.icons.filled.Cancel
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.scale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.oubli.wallet.ui.QRScannerScreen
import com.oubli.wallet.ui.components.FullScreenTaskDialog
import com.oubli.wallet.ui.components.TaskPrimaryButton
import com.oubli.wallet.ui.theme.OubliReceived
import com.oubli.wallet.ui.theme.OubliSuccessBg
import com.oubli.wallet.ui.theme.OubliErrorBg
import com.oubli.wallet.ui.util.normalizeLightningInvoice
import com.oubli.wallet.ui.util.parseOubliUri
import com.oubli.wallet.ui.util.parseBolt11AmountSats
import com.oubli.wallet.viewmodel.LightningSendStatus
import com.oubli.wallet.viewmodel.LightningSendUiState

@Composable
fun SendDialog(
    balanceSats: String,
    calculateSendFee: (amount: String, recipient: String) -> String,
    onConfirm: (amount: String, recipient: String) -> Unit,
    onStartLightningPayment: (bolt11: String) -> Unit,
    lightningOperation: String?,
    lightningSendState: LightningSendUiState,
    onClearLightningSendState: () -> Unit,
    onDismiss: () -> Unit,
    initialRecipient: String? = null,
    contacts: List<uniffi.oubli.ContactFfi> = emptyList(),
    onContactSelected: ((String) -> Unit)? = null,
    satsToFiatRaw: (String) -> String? = { null },
    fiatToSats: (String) -> String? = { null },
    fiatCurrency: String = "usd",
    fiatSymbol: String = "$",
) {
    var amount by rememberSaveable { mutableStateOf("") }
    var recipient by rememberSaveable { mutableStateOf(initialRecipient ?: "") }
    var showScanner by rememberSaveable { mutableStateOf(false) }
    var lnNoAmountError by rememberSaveable { mutableStateOf(false) }
    var showSendConfirmation by rememberSaveable { mutableStateOf(false) }

    // Auto-process scanned code on first composition
    LaunchedEffect(initialRecipient) {
        if (initialRecipient != null) {
            val normalized = normalizeLightningInvoice(initialRecipient)
            if (normalized != null) {
                val parsedAmount = parseBolt11AmountSats(normalized)
                if (parsedAmount != null) {
                    recipient = normalized
                    amount = parsedAmount
                }
            } else {
                val parsed = parseOubliUri(initialRecipient)
                if (parsed != null) {
                    recipient = parsed.first
                    if (parsed.second != null) amount = parsed.second!!
                }
            }
        }
    }

    // Debounced fee calculation
    var debouncedFeeSats by remember { mutableStateOf("0") }
    LaunchedEffect(amount, recipient) {
        if (amount.isBlank()) {
            debouncedFeeSats = "0"
        } else {
            kotlinx.coroutines.delay(300)
            debouncedFeeSats = calculateSendFee(amount, recipient)
        }
    }

    val enteredAmount = amount.toLongOrNull()
    val availableBalance = balanceSats.toLongOrNull()
    val normalizedLightningInvoice = normalizeLightningInvoice(recipient)
    val lightningInvoiceAmountSats = normalizedLightningInvoice?.let(::parseBolt11AmountSats)
    val hasLightningInvoice = normalizedLightningInvoice != null
    val lightningInvoiceMissingAmount = hasLightningInvoice && lightningInvoiceAmountSats == null
    val feeSats = debouncedFeeSats
    val feeAmount = feeSats.toLongOrNull() ?: 0L
    val totalAmount = (enteredAmount ?: 0L) + feeAmount
    val insufficientFunds =
        enteredAmount != null && availableBalance != null && totalAmount > availableBalance
    val canReview = recipient.isNotBlank() && amount.isNotBlank() && !insufficientFunds && !lightningInvoiceMissingAmount

    fun syncAmountWithLightningInvoice(value: String) {
        val parsedAmount = normalizeLightningInvoice(value)?.let(::parseBolt11AmountSats) ?: return
        if (amount != parsedAmount) {
            amount = parsedAmount
        }
    }

    fun prepareLightningInvoiceForReview(invoice: String) {
        val parsedAmount = parseBolt11AmountSats(invoice)
        if (parsedAmount == null) {
            lnNoAmountError = true
            return
        }
        recipient = invoice
        amount = parsedAmount
    }

    val swapProcessing = lightningSendState.status == LightningSendStatus.Processing
    val swapSuccessMessage = lightningSendState.message.takeIf {
        lightningSendState.status == LightningSendStatus.Success
    }
    val swapErrorMessage = lightningSendState.message.takeIf {
        lightningSendState.status == LightningSendStatus.Error
    }

    val dismissSendDialog = {
        if (!swapProcessing) {
            onClearLightningSendState()
        }
        onDismiss()
    }

    val bottomBar: (@Composable () -> Unit)? = when {
        swapProcessing -> null
        swapSuccessMessage != null -> ({
            TaskPrimaryButton(
                title = "Done",
                onClick = dismissSendDialog,
            )
        })
        swapErrorMessage != null -> ({
            TaskPrimaryButton(
                title = "Close",
                onClick = dismissSendDialog,
            )
        })
        else -> ({
            TaskPrimaryButton(
                title = if (hasLightningInvoice) "Pay Invoice" else "Send",
                enabled = canReview,
                onClick = {
                    if (normalizedLightningInvoice != null) {
                        onStartLightningPayment(normalizedLightningInvoice)
                    } else {
                        showSendConfirmation = true
                    }
                },
            )
        })
    }

    if (showScanner) {
        Dialog(
            onDismissRequest = { showScanner = false },
            properties = DialogProperties(usePlatformDefaultWidth = false),
        ) {
            QRScannerScreen(
                onCodeScanned = { code ->
                    val normalized = normalizeLightningInvoice(code)
                    if (normalized != null) {
                        showScanner = false
                        prepareLightningInvoiceForReview(normalized)
                    } else {
                        val parsed = parseOubliUri(code)
                        if (parsed != null) {
                            recipient = parsed.first
                            if (parsed.second != null) amount = parsed.second!!
                        } else {
                            recipient = code
                        }
                        showScanner = false
                    }
                },
                onClose = { showScanner = false },
            )
        }
        return
    }

    if (lnNoAmountError) {
        AlertDialog(
            onDismissRequest = { lnNoAmountError = false },
            title = { Text("No Amount in Invoice") },
            text = {
                Text("This Lightning invoice doesn't include an amount. Please ask the recipient for an invoice with a specific amount.")
            },
            confirmButton = {
                Button(onClick = { lnNoAmountError = false }) { Text("OK") }
            },
        )
        return
    }

    if (showSendConfirmation) {
        val shortRecipient = if (recipient.length > 28) {
            recipient.take(16) + "..." + recipient.takeLast(8)
        } else {
            recipient
        }
        AlertDialog(
            onDismissRequest = { showSendConfirmation = false },
            title = { Text("Confirm Send") },
            text = { Text("Send $amount sats to $shortRecipient?") },
            confirmButton = {
                Button(onClick = {
                    showSendConfirmation = false
                    onConfirm(amount, recipient)
                }) { Text("Send") }
            },
            dismissButton = {
                TextButton(onClick = { showSendConfirmation = false }) { Text("Cancel") }
            },
        )
    }

    FullScreenTaskDialog(
        title = when {
            swapProcessing -> "Sending"
            swapSuccessMessage != null -> "Payment Sent"
            swapErrorMessage != null -> "Payment Failed"
            else -> "Send"
        },
        onDismissRequest = dismissSendDialog,
        dismissEnabled = !swapProcessing,
        leadingActionLabel = "Close",
        leadingAction = dismissSendDialog,
        bottomBar = bottomBar,
    ) {
        when {
            // Animated sending state
            swapProcessing -> {
                val pulseTransition = rememberInfiniteTransition(label = "pulse")
                val pulseAlpha by pulseTransition.animateFloat(
                    initialValue = 1f,
                    targetValue = 0.4f,
                    animationSpec = infiniteRepeatable(
                        animation = tween(1000),
                        repeatMode = RepeatMode.Reverse,
                    ),
                    label = "pulseAlpha",
                )
                var animStarted by remember { mutableStateOf(false) }
                LaunchedEffect(Unit) { animStarted = true }
                val iconScale by animateFloatAsState(
                    targetValue = if (animStarted) 1f else 0.3f,
                    animationSpec = tween(400, easing = EaseOutCubic),
                    label = "iconScale",
                )
                val iconOffsetY by animateFloatAsState(
                    targetValue = if (animStarted) 0f else 60f,
                    animationSpec = tween(400, easing = EaseOutCubic),
                    label = "iconOffsetY",
                )

                Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center,
                ) {
                    Column(
                        modifier = Modifier.padding(24.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        Box(
                            modifier = Modifier
                                .size(80.dp)
                                .scale(iconScale)
                                .offset { IntOffset(0, iconOffsetY.toInt()) }
                                .alpha(pulseAlpha)
                                .background(
                                    color = MaterialTheme.colorScheme.primary.copy(alpha = 0.15f),
                                    shape = CircleShape,
                                )
                                .semantics { contentDescription = "Processing payment" },
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(
                                Icons.Filled.Bolt,
                                contentDescription = null,
                                modifier = Modifier.size(40.dp),
                                tint = MaterialTheme.colorScheme.primary,
                            )
                        }
                        Spacer(modifier = Modifier.height(24.dp))
                        Text(
                            text = "Sending $amount sats",
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.SemiBold,
                            textAlign = TextAlign.Center,
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = lightningOperation ?: "This may take a few minutes",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            textAlign = TextAlign.Center,
                        )
                    }
                }
            }

            // Success state — green tinted screen
            swapSuccessMessage != null -> {
                var animStarted by remember { mutableStateOf(false) }
                LaunchedEffect(Unit) { animStarted = true }
                val iconScale by animateFloatAsState(
                    targetValue = if (animStarted) 1f else 0.3f,
                    animationSpec = tween(400, easing = EaseOutCubic),
                    label = "successScale",
                )

                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .background(OubliSuccessBg),
                    contentAlignment = Alignment.Center,
                ) {
                    Column(
                        modifier = Modifier.padding(24.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        Icon(
                            Icons.Filled.CheckCircle,
                            contentDescription = "Payment successful",
                            modifier = Modifier.size(72.dp).scale(iconScale),
                            tint = OubliReceived,
                        )
                        Spacer(modifier = Modifier.height(16.dp))
                        Text(
                            text = "$amount sats sent",
                            style = MaterialTheme.typography.headlineSmall,
                            fontWeight = FontWeight.Bold,
                            textAlign = TextAlign.Center,
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = swapSuccessMessage!!,
                            style = MaterialTheme.typography.bodyMedium,
                            textAlign = TextAlign.Center,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }

            // Error state — red tinted screen
            swapErrorMessage != null -> {
                var animStarted by remember { mutableStateOf(false) }
                LaunchedEffect(Unit) { animStarted = true }
                val iconScale by animateFloatAsState(
                    targetValue = if (animStarted) 1f else 0.3f,
                    animationSpec = tween(400, easing = EaseOutCubic),
                    label = "errorScale",
                )

                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .background(OubliErrorBg),
                    contentAlignment = Alignment.Center,
                ) {
                    Column(
                        modifier = Modifier.padding(24.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        Icon(
                            Icons.Filled.Cancel,
                            contentDescription = "Payment failed",
                            modifier = Modifier.size(72.dp).scale(iconScale),
                            tint = MaterialTheme.colorScheme.error,
                        )
                        Spacer(modifier = Modifier.height(16.dp))
                        Text(
                            text = "Payment Failed",
                            style = MaterialTheme.typography.headlineSmall,
                            fontWeight = FontWeight.Bold,
                            textAlign = TextAlign.Center,
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = swapErrorMessage!!,
                            style = MaterialTheme.typography.bodyMedium,
                            textAlign = TextAlign.Center,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }

            else -> {
                val context = LocalContext.current
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .verticalScroll(rememberScrollState())
                        .padding(24.dp),
                    verticalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    com.oubli.wallet.ui.components.DualAmountInput(
                        satsAmount = amount,
                        onSatsChange = { amount = it },
                        satsToFiatRaw = satsToFiatRaw,
                        fiatToSats = fiatToSats,
                        fiatCurrency = fiatCurrency,
                        fiatSymbol = fiatSymbol,
                        readOnly = hasLightningInvoice,
                        showMaxButton = !hasLightningInvoice,
                        maxSats = balanceSats,
                    )
                    if (lightningInvoiceMissingAmount) {
                        Text(
                            "This Lightning invoice doesn't include an amount.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.error,
                        )
                    } else if (hasLightningInvoice) {
                        Text(
                            "Amount comes from the Lightning invoice.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    if (insufficientFunds) {
                        Text(
                            if (feeAmount > 0L) "Insufficient funds (need $totalAmount, available: $balanceSats)"
                            else "Insufficient funds (available: $balanceSats)",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.error,
                        )
                    }
                    // Inline fee/total summary
                    if (amount.isNotBlank() && feeAmount > 0L) {
                        Card(
                            modifier = Modifier.fillMaxWidth(),
                            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
                            shape = RoundedCornerShape(12.dp),
                        ) {
                            Column(
                                modifier = Modifier.padding(12.dp),
                                verticalArrangement = Arrangement.spacedBy(4.dp),
                            ) {
                                Row(
                                    modifier = Modifier.fillMaxWidth(),
                                    horizontalArrangement = Arrangement.SpaceBetween,
                                ) {
                                    Text("Est. fee", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                    Text("$feeSats sats", style = MaterialTheme.typography.bodySmall)
                                }
                                HorizontalDivider()
                                Row(
                                    modifier = Modifier.fillMaxWidth(),
                                    horizontalArrangement = Arrangement.SpaceBetween,
                                ) {
                                    Text("Total", style = MaterialTheme.typography.bodySmall, fontWeight = FontWeight.SemiBold)
                                    Text("$totalAmount sats", style = MaterialTheme.typography.bodySmall, fontWeight = FontWeight.SemiBold)
                                }
                            }
                        }
                    }
                    // Contact quick-pick
                    if (contacts.isNotEmpty() && recipient.isBlank()) {
                        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                            Text("Recent Contacts", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                contacts.take(3).forEach { contact ->
                                    androidx.compose.material3.AssistChip(
                                        onClick = {
                                            contact.addresses.firstOrNull()?.let { addr ->
                                                recipient = addr.address
                                                onContactSelected?.invoke(contact.id)
                                            }
                                        },
                                        label = { Text(contact.name, maxLines = 1) },
                                    )
                                }
                            }
                        }
                    }

                    OutlinedTextField(
                        value = recipient,
                        onValueChange = {
                            recipient = it
                            syncAmountWithLightningInvoice(it)
                        },
                        label = { Text("Recipient") },
                        modifier = Modifier.fillMaxWidth(),
                        minLines = 3,
                        maxLines = 5,
                        trailingIcon = {
                            Row {
                                IconButton(onClick = {
                                    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                                    val clip = clipboard.primaryClip
                                    val text = clip?.getItemAt(0)?.text?.toString() ?: ""
                                    if (text.isNotBlank()) {
                                        val normalized = normalizeLightningInvoice(text)
                                        if (normalized != null) {
                                            prepareLightningInvoiceForReview(normalized)
                                        } else {
                                            recipient = text.trim()
                                        }
                                    }
                                }) {
                                    Icon(
                                        imageVector = Icons.Filled.ContentPaste,
                                        contentDescription = "Paste from clipboard",
                                    )
                                }
                                IconButton(onClick = { showScanner = true }) {
                                    Icon(
                                        imageVector = Icons.Filled.QrCodeScanner,
                                        contentDescription = "Scan QR code",
                                    )
                                }
                            }
                        },
                    )
                    if (hasLightningInvoice) {
                        Text(
                            text = "Lightning invoice detected.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Text(
                        text = "Paste or scan a Starknet address or Lightning invoice.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}
