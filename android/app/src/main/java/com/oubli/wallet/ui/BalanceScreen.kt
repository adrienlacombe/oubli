package com.oubli.wallet.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Color
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.Download
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material.icons.filled.Public
import androidx.compose.material.icons.filled.Cancel
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Schedule
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Share
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.google.zxing.BarcodeFormat
import com.google.zxing.qrcode.QRCodeWriter
import kotlinx.coroutines.launch
import uniffi.oubli.ActivityEventFfi
import java.math.BigDecimal
import java.math.RoundingMode

@OptIn(ExperimentalFoundationApi::class, ExperimentalMaterial3Api::class)
@Composable
fun BalanceScreen(
    address: String,
    publicKey: String,
    balanceSats: String,
    pendingSats: String,
    isBalanceHidden: Boolean,
    showUsd: Boolean = false,
    onToggleCurrency: () -> Unit = {},
    satsToUsd: (String) -> String? = { null },
    activity: List<ActivityEventFfi> = emptyList(),
    onToggleBalanceHidden: () -> Unit,
    onRefresh: () -> Unit,
    onSend: (amountSats: String, recipient: String) -> Unit,
    onPayLightning: (bolt11: String, onResult: (Result<String?>) -> Unit) -> Unit = { _, _ -> },
    lightningOperation: String? = null,
    onReceiveLightningCreate: (amountSats: ULong, onResult: (Result<uniffi.oubli.SwapQuoteFfi>) -> Unit) -> Unit = { _, _ -> },
    onReceiveLightningWait: (swapId: String, onResult: (Result<Unit>) -> Unit) -> Unit = { _, _ -> },
    onGetRpcUrl: () -> String = { "" },
    onUpdateRpcUrl: (String) -> Unit = {},
    onGetMnemonic: (onResult: (Result<String>) -> Unit) -> Unit = { _ -> },
    autoFundError: String? = null,
    isRefreshing: Boolean = false,
) {
    var showDialog by rememberSaveable { mutableStateOf<ActionDialog?>(null) }
    var showMenu by remember { mutableStateOf(false) }

    PullToRefreshBox(
        isRefreshing = isRefreshing,
        onRefresh = onRefresh,
        modifier = Modifier.fillMaxSize(),
    ) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        // Balance card with menu to the right
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Card(
                modifier = Modifier.weight(1f),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.primaryContainer),
            ) {
                Column(
                    modifier = Modifier.fillMaxWidth().padding(20.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    val displayAmount = if (isBalanceHidden) {
                        "\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022"
                    } else if (showUsd) {
                        satsToUsd(balanceSats) ?: balanceSats
                    } else {
                        balanceSats
                    }
                    Text(
                        text = displayAmount,
                        style = MaterialTheme.typography.displaySmall,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.onPrimaryContainer,
                        modifier = Modifier.clickable { onToggleBalanceHidden() },
                    )
                    Spacer(modifier = Modifier.height(4.dp))
                    Text(
                        text = if (showUsd) "USD" else "sats",
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.onPrimaryContainer.copy(alpha = 0.6f),
                        modifier = Modifier.clickable { onToggleCurrency() },
                    )
                    if (!isBalanceHidden) {
                        val pendingVal = pendingSats.toLongOrNull() ?: 0L
                        if (pendingVal > 0L) {
                            Spacer(modifier = Modifier.height(4.dp))
                            val pendingDisplay = if (showUsd) {
                                "+${satsToUsd(pendingSats) ?: pendingSats} incoming"
                            } else {
                                "+$pendingSats sats incoming"
                            }
                            Text(
                                text = pendingDisplay,
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onPrimaryContainer.copy(alpha = 0.6f),
                            )
                        }
                    }
                }
            }
            Box {
                IconButton(onClick = { showMenu = true }) {
                    Icon(Icons.Filled.MoreVert, contentDescription = "More options")
                }
                DropdownMenu(
                    expanded = showMenu,
                    onDismissRequest = { showMenu = false },
                ) {
                    DropdownMenuItem(
                        text = { Text("Show Seed Phrase") },
                        onClick = { showMenu = false; showDialog = ActionDialog.ShowSeedPhrase },
                        leadingIcon = { Icon(Icons.Filled.Visibility, contentDescription = null) },
                    )
                    DropdownMenuItem(
                        text = { Text("Debug Settings") },
                        onClick = { showMenu = false; showDialog = ActionDialog.DebugSettings },
                        leadingIcon = { Icon(Icons.Filled.Settings, contentDescription = null) },
                    )
                }
            }
        }

        Spacer(modifier = Modifier.height(24.dp))

        // Action buttons — Send and Receive
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Button(
                onClick = { showDialog = ActionDialog.Send },
                modifier = Modifier.weight(1f),
            ) {
                Icon(Icons.AutoMirrored.Filled.Send, contentDescription = null, modifier = Modifier.size(20.dp))
                Spacer(modifier = Modifier.width(4.dp))
                Text("Send")
            }
            Button(
                onClick = { showDialog = ActionDialog.Receive },
                modifier = Modifier.weight(1f),
            ) {
                Icon(Icons.Filled.Download, contentDescription = null, modifier = Modifier.size(20.dp))
                Spacer(modifier = Modifier.width(4.dp))
                Text("Receive")
            }
        }

        Spacer(modifier = Modifier.height(16.dp))
        HorizontalDivider()
        Spacer(modifier = Modifier.height(16.dp))

        // Activity section
        Text(
            text = "Activity",
            style = MaterialTheme.typography.titleSmall,
            modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp),
        )

        if (activity.isEmpty()) {
            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
            ) {
                Column(
                    modifier = Modifier.padding(24.dp).fillMaxWidth(),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Icon(
                        Icons.Filled.Schedule,
                        contentDescription = null,
                        modifier = Modifier.size(32.dp),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = "No transactions yet",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        } else {
            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
            ) {
                Column(modifier = Modifier.padding(12.dp)) {
                    activity.forEachIndexed { index, event ->
                        if (index > 0) {
                            HorizontalDivider(
                                modifier = Modifier.padding(vertical = 4.dp),
                                color = MaterialTheme.colorScheme.outlineVariant,
                            )
                        }
                        Row(
                            modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp, horizontal = 4.dp),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    text = activityLabel(event.eventType),
                                    style = MaterialTheme.typography.bodyMedium,
                                    fontWeight = FontWeight.Medium,
                                )
                                Text(
                                    text = event.txHash.take(10) + "..." + event.txHash.takeLast(6),
                                    style = MaterialTheme.typography.bodySmall,
                                    fontFamily = FontFamily.Monospace,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                            if (event.amountSats != null) {
                                Text(
                                    text = "${event.amountSats} sats",
                                    style = MaterialTheme.typography.bodyMedium,
                                    fontFamily = FontFamily.Monospace,
                                    fontWeight = FontWeight.Medium,
                                )
                            }
                        }
                    }
                }
            }
        }

        // Auto-fund error banner
        if (autoFundError != null) {
            Spacer(modifier = Modifier.height(16.dp))
            val context = LocalContext.current
            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable {
                        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                        clipboard.setPrimaryClip(ClipData.newPlainText("error", autoFundError))
                        android.widget.Toast.makeText(context, "Copied to clipboard", android.widget.Toast.LENGTH_SHORT).show()
                    },
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.errorContainer),
            ) {
                Column(modifier = Modifier.padding(12.dp)) {
                    Text(
                        text = "Auto-fund error (tap to copy)",
                        style = MaterialTheme.typography.labelSmall,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.onErrorContainer,
                    )
                    Spacer(modifier = Modifier.height(4.dp))
                    Text(
                        text = autoFundError,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onErrorContainer,
                    )
                }
            }
        }
    }
    } // PullToRefreshBox

    // ---- Dialogs ----

    when (showDialog) {
        ActionDialog.Send -> {
            SendDialog(
                balanceSats = balanceSats,
                onConfirm = { amount, recipient ->
                    onSend(amount, recipient)
                    showDialog = null
                },
                onPayLightning = onPayLightning,
                lightningOperation = lightningOperation,
                onDismiss = { showDialog = null },
            )
        }
        ActionDialog.Receive -> {
            ReceiveDialog(
                address = address,
                publicKey = publicKey,
                onReceiveLightningCreate = onReceiveLightningCreate,
                onReceiveLightningWait = onReceiveLightningWait,
                onDismiss = { showDialog = null },
            )
        }
        ActionDialog.DebugSettings -> {
            DebugSettingsDialog(
                currentRpcUrl = onGetRpcUrl(),
                onSave = { url -> onUpdateRpcUrl(url); showDialog = null },
                onDismiss = { showDialog = null },
            )
        }
        ActionDialog.ShowSeedPhrase -> {
            ShowSeedPhraseDialog(
                onGetMnemonic = onGetMnemonic,
                onDismiss = { showDialog = null },
            )
        }
        null -> {}
    }
}

private enum class ActionDialog {
    Send, Receive, DebugSettings, ShowSeedPhrase
}

private fun activityLabel(type: String): String = when (type) {
    "Fund" -> "Received"
    "TransferOut" -> "Sent"
    "TransferIn" -> "Received"
    "Withdraw" -> "Sent"
    "Rollover" -> "Settled"
    "Ragequit" -> "Emergency Exit"
    else -> type
}

@Composable
private fun FullScreenTaskDialog(
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
                modifier = Modifier
                    .fillMaxSize()
                    .statusBarsPadding()
                    .navigationBarsPadding(),
            ) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
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
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth(),
                ) {
                    content()
                }
                if (bottomBar != null) {
                    HorizontalDivider()
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .background(MaterialTheme.colorScheme.surface)
                            .padding(horizontal = 24.dp, vertical = 16.dp),
                    ) {
                        bottomBar()
                    }
                }
            }
        }
    }
}

@Composable
private fun TaskPrimaryButton(
    title: String,
    enabled: Boolean = true,
    onClick: () -> Unit,
) {
    Button(
        onClick = onClick,
        enabled = enabled,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Text(title)
    }
}

@Composable
private fun SendDialog(
    balanceSats: String,
    onConfirm: (amount: String, recipient: String) -> Unit,
    onPayLightning: (bolt11: String, onResult: (Result<String?>) -> Unit) -> Unit,
    lightningOperation: String?,
    onDismiss: () -> Unit,
) {
    var amount by rememberSaveable { mutableStateOf("") }
    var recipient by rememberSaveable { mutableStateOf("") }
    var isReviewing by rememberSaveable { mutableStateOf(false) }
    var showScanner by rememberSaveable { mutableStateOf(false) }
    var lnNoAmountError by rememberSaveable { mutableStateOf(false) }
    var swapProcessing by remember { mutableStateOf(false) }
    var swapSuccessMessage by remember { mutableStateOf<String?>(null) }
    var swapErrorMessage by remember { mutableStateOf<String?>(null) }

    val enteredAmount = amount.toLongOrNull()
    val availableBalance = balanceSats.toLongOrNull()
    val insufficientFunds = enteredAmount != null && availableBalance != null && enteredAmount > availableBalance
    val normalizedLightningInvoice = normalizeLightningInvoice(recipient)
    val lightningInvoiceAmountSats = normalizedLightningInvoice?.let(::parseBolt11AmountSats)
    val hasLightningInvoice = normalizedLightningInvoice != null
    val lightningInvoiceMissingAmount = hasLightningInvoice && lightningInvoiceAmountSats == null
    val canReview = recipient.isNotBlank() && amount.isNotBlank() && !insufficientFunds && !lightningInvoiceMissingAmount

    fun startLightningPayment(bolt11: String) {
        swapProcessing = true
        swapSuccessMessage = null
        swapErrorMessage = null
        onPayLightning(bolt11) { result ->
            swapProcessing = false
            result
                .onSuccess { txHash ->
                    val msg = if (txHash != null && txHash.length > 16) {
                        "Tx: ${txHash.take(10)}...${txHash.takeLast(6)}"
                    } else {
                        txHash ?: "Payment complete"
                    }
                    swapSuccessMessage = msg
                }
                .onFailure { e ->
                    swapErrorMessage = e.message ?: "Unknown error"
                }
        }
    }

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
        isReviewing = true
    }

    val bottomBar: (@Composable () -> Unit)? = when {
        swapProcessing -> null
        swapSuccessMessage != null -> ({ TaskPrimaryButton(title = "Done", onClick = onDismiss) })
        swapErrorMessage != null -> ({ TaskPrimaryButton(title = "Close", onClick = onDismiss) })
        isReviewing -> ({
            TaskPrimaryButton(
                title = if (hasLightningInvoice) "Pay Invoice" else "Send",
                enabled = canReview,
                onClick = {
                    normalizedLightningInvoice?.let(::startLightningPayment) ?: onConfirm(amount, recipient)
                },
            )
        })
        else -> ({
            TaskPrimaryButton(
                title = "Review",
                enabled = canReview,
                onClick = { isReviewing = true },
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

    FullScreenTaskDialog(
        title = when {
            swapProcessing -> "Lightning Payment"
            swapSuccessMessage != null -> "Payment Sent"
            swapErrorMessage != null -> "Payment Failed"
            isReviewing -> if (hasLightningInvoice) "Confirm Payment" else "Confirm Send"
            else -> "Send"
        },
        onDismissRequest = onDismiss,
        dismissEnabled = !swapProcessing,
        leadingActionLabel = if (isReviewing && !swapProcessing && swapSuccessMessage == null && swapErrorMessage == null) {
            "Back"
        } else {
            "Close"
        },
        leadingAction = {
            if (isReviewing && !swapProcessing && swapSuccessMessage == null && swapErrorMessage == null) {
                isReviewing = false
            } else {
                onDismiss()
            }
        },
        bottomBar = bottomBar,
    ) {
        when {
            swapProcessing -> {
                Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center,
                ) {
                    Column(
                        modifier = Modifier.padding(24.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        CircularProgressIndicator()
                        Spacer(modifier = Modifier.height(16.dp))
                        Text(
                            text = lightningOperation ?: "This may take a few minutes",
                            style = MaterialTheme.typography.bodyMedium,
                            textAlign = TextAlign.Center,
                        )
                    }
                }
            }

            swapSuccessMessage != null -> {
                Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center,
                ) {
                    Column(
                        modifier = Modifier.padding(24.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        Icon(
                            Icons.Filled.CheckCircle,
                            contentDescription = null,
                            modifier = Modifier.size(56.dp),
                            tint = androidx.compose.ui.graphics.Color(0xFF4CAF50),
                        )
                        Spacer(modifier = Modifier.height(12.dp))
                        Text(
                            text = swapSuccessMessage!!,
                            style = MaterialTheme.typography.bodyMedium,
                            textAlign = TextAlign.Center,
                        )
                    }
                }
            }

            swapErrorMessage != null -> {
                Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center,
                ) {
                    Column(
                        modifier = Modifier.padding(24.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        Icon(
                            Icons.Filled.Cancel,
                            contentDescription = null,
                            modifier = Modifier.size(56.dp),
                            tint = MaterialTheme.colorScheme.error,
                        )
                        Spacer(modifier = Modifier.height(12.dp))
                        Text(
                            text = swapErrorMessage!!,
                            style = MaterialTheme.typography.bodyMedium,
                            textAlign = TextAlign.Center,
                        )
                    }
                }
            }

            isReviewing -> {
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(24.dp),
                    verticalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    Text(
                        text = "Review the transfer details before sending.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Card(
                        modifier = Modifier.fillMaxWidth(),
                        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
                    ) {
                        Column(
                            modifier = Modifier.padding(16.dp),
                            verticalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            if (hasLightningInvoice) {
                                Text("Payment type", style = MaterialTheme.typography.labelLarge)
                                Text("Lightning invoice", style = MaterialTheme.typography.bodyMedium)
                                HorizontalDivider()
                            }
                            Text("Amount", style = MaterialTheme.typography.labelLarge)
                            Text("$amount sats", style = MaterialTheme.typography.headlineSmall)
                            HorizontalDivider()
                            Text("Recipient", style = MaterialTheme.typography.labelLarge)
                            Text(
                                recipient,
                                style = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
                                maxLines = 4,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
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
                    OutlinedTextField(
                        value = amount,
                        onValueChange = { amount = it },
                        label = { Text("Amount (sats)") },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                        readOnly = hasLightningInvoice,
                        trailingIcon = {
                            if (!hasLightningInvoice) {
                                TextButton(onClick = { amount = balanceSats }) {
                                    Text("Max")
                                }
                            }
                        },
                        isError = insufficientFunds || lightningInvoiceMissingAmount,
                        supportingText = when {
                            lightningInvoiceMissingAmount -> {
                                { Text("This Lightning invoice doesn't include an amount.") }
                            }
                            insufficientFunds -> {
                                { Text("Insufficient funds (available: $balanceSats)") }
                            }
                            hasLightningInvoice -> {
                                { Text("Amount comes from the Lightning invoice.") }
                            }
                            else -> null
                        },
                    )
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

@Composable
private fun ReceiveDialog(
    address: String,
    publicKey: String,
    onReceiveLightningCreate: (amountSats: ULong, onResult: (Result<uniffi.oubli.SwapQuoteFfi>) -> Unit) -> Unit,
    onReceiveLightningWait: (swapId: String, onResult: (Result<Unit>) -> Unit) -> Unit,
    onDismiss: () -> Unit,
) {
    val context = LocalContext.current
    val pagerState = rememberPagerState(pageCount = { 3 })
    val scope = rememberCoroutineScope()

    // Oubli receive amount (optional)
    var oubliAmountSats by remember { mutableStateOf("") }

    // Lightning receive state
    var lnAmountSats by remember { mutableStateOf("") }
    var lnInvoice by remember { mutableStateOf<String?>(null) }
    var lnSwapId by remember { mutableStateOf<String?>(null) }
    var lnFee by remember { mutableStateOf<String?>(null) }
    var lnWaiting by remember { mutableStateOf(false) }
    var lnSuccess by remember { mutableStateOf(false) }
    var lnError by remember { mutableStateOf<String?>(null) }

    fun startLnWait(swapId: String) {
        lnError = null
        lnWaiting = true
        onReceiveLightningWait(swapId) { waitResult ->
            lnWaiting = false
            waitResult
                .onSuccess { lnSuccess = true }
                .onFailure { e -> lnError = e.message ?: "Failed to confirm Lightning payment" }
        }
    }

    FullScreenTaskDialog(
        title = "Receive",
        onDismissRequest = onDismiss,
        dismissEnabled = !lnWaiting,
    ) {
        Column(
            modifier = Modifier.fillMaxSize(),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            TabRow(selectedTabIndex = pagerState.currentPage) {
                Tab(
                    selected = pagerState.currentPage == 0,
                    onClick = { scope.launch { pagerState.animateScrollToPage(0) } },
                    text = { Text("Oubli") },
                    icon = { Icon(Icons.Filled.Lock, contentDescription = null, modifier = Modifier.size(16.dp)) },
                )
                Tab(
                    selected = pagerState.currentPage == 1,
                    onClick = { scope.launch { pagerState.animateScrollToPage(1) } },
                    text = { Text("Starknet") },
                    icon = { Icon(Icons.Filled.Public, contentDescription = null, modifier = Modifier.size(16.dp)) },
                )
                Tab(
                    selected = pagerState.currentPage == 2,
                    onClick = { scope.launch { pagerState.animateScrollToPage(2) } },
                    text = { Text("Lightning") },
                )
            }
            HorizontalPager(
                state = pagerState,
                modifier = Modifier.fillMaxSize(),
            ) { page ->
                when (page) {
                    0 -> {
                        // Oubli tab with optional amount
                        val oubliValue = if (oubliAmountSats.isEmpty()) publicKey
                            else "oubli:$publicKey?amount=$oubliAmountSats"
                        Column(
                            modifier = Modifier
                                .fillMaxSize()
                                .padding(24.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.Center,
                        ) {
                            val qrBitmap = remember(oubliValue) { generateQrBitmap(oubliValue, 400) }
                            if (qrBitmap != null) {
                                Image(
                                    bitmap = qrBitmap.asImageBitmap(),
                                    contentDescription = "QR Code",
                                    modifier = Modifier.size(220.dp),
                                )
                            }
                            Spacer(modifier = Modifier.height(12.dp))
                            Text(
                                text = "For receiving from Oubli wallets",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            Spacer(modifier = Modifier.height(8.dp))
                            Text(
                                text = publicKey,
                                style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                                textAlign = TextAlign.Center,
                                maxLines = 3,
                                overflow = TextOverflow.Ellipsis,
                            )
                            Spacer(modifier = Modifier.height(8.dp))
                            OutlinedTextField(
                                value = oubliAmountSats,
                                onValueChange = { oubliAmountSats = it.filter { c -> c.isDigit() } },
                                label = { Text("Amount (sats, optional)") },
                                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                                singleLine = true,
                                modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp),
                            )
                            Spacer(modifier = Modifier.height(16.dp))
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.spacedBy(12.dp),
                            ) {
                                Button(
                                    onClick = {
                                        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                                        clipboard.setPrimaryClip(ClipData.newPlainText("Address", oubliValue))
                                        android.widget.Toast.makeText(context, "Copied to clipboard", android.widget.Toast.LENGTH_SHORT).show()
                                    },
                                    modifier = Modifier.weight(1f),
                                ) {
                                    Icon(Icons.Filled.ContentCopy, contentDescription = null, modifier = Modifier.size(16.dp))
                                    Spacer(modifier = Modifier.width(4.dp))
                                    Text("Copy")
                                }
                                OutlinedButton(
                                    onClick = {
                                        shareText(
                                            context = context,
                                            chooserTitle = "Share Oubli request",
                                            subject = "Receive with Oubli",
                                            text = staticReceiveShareText(
                                                title = "Oubli",
                                                subtitle = "For receiving from Oubli wallets",
                                                value = oubliValue,
                                            ),
                                        )
                                    },
                                    modifier = Modifier.weight(1f),
                                ) {
                                    Icon(Icons.Filled.Share, contentDescription = null, modifier = Modifier.size(16.dp))
                                    Spacer(modifier = Modifier.width(4.dp))
                                    Text("Share")
                                }
                            }
                        }
                    }
                    1 -> {
                        // Starknet tab (static)
                        val value = address
                        Column(
                            modifier = Modifier
                                .fillMaxSize()
                                .padding(24.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.Center,
                        ) {
                            val qrBitmap = remember(value) { generateQrBitmap(value, 400) }
                            if (qrBitmap != null) {
                                Image(
                                    bitmap = qrBitmap.asImageBitmap(),
                                    contentDescription = "QR Code",
                                    modifier = Modifier.size(220.dp),
                                )
                            }
                            Spacer(modifier = Modifier.height(12.dp))
                            Text(
                                text = "For receiving from any Starknet wallet",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            Spacer(modifier = Modifier.height(8.dp))
                            Text(
                                text = value,
                                style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                                textAlign = TextAlign.Center,
                            )
                            Spacer(modifier = Modifier.height(16.dp))
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.spacedBy(12.dp),
                            ) {
                                Button(
                                    onClick = {
                                        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                                        clipboard.setPrimaryClip(ClipData.newPlainText("Address", value))
                                        android.widget.Toast.makeText(context, "Copied to clipboard", android.widget.Toast.LENGTH_SHORT).show()
                                    },
                                    modifier = Modifier.weight(1f),
                                ) {
                                    Icon(Icons.Filled.ContentCopy, contentDescription = null, modifier = Modifier.size(16.dp))
                                    Spacer(modifier = Modifier.width(4.dp))
                                    Text("Copy")
                                }
                                OutlinedButton(
                                    onClick = {
                                        shareText(
                                            context = context,
                                            chooserTitle = "Share Starknet request",
                                            subject = "Receive with Oubli",
                                            text = staticReceiveShareText(
                                                title = "Starknet",
                                                subtitle = "For receiving from any Starknet wallet",
                                                value = value,
                                            ),
                                        )
                                    },
                                    modifier = Modifier.weight(1f),
                                ) {
                                    Icon(Icons.Filled.Share, contentDescription = null, modifier = Modifier.size(16.dp))
                                    Spacer(modifier = Modifier.width(4.dp))
                                    Text("Share")
                                }
                            }
                        }
                    }

                    2 -> {
                        Column(
                            modifier = Modifier
                                .fillMaxSize()
                                .verticalScroll(rememberScrollState())
                                .padding(24.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                        ) {
                            Spacer(modifier = Modifier.height(24.dp))
                            if (lnSuccess) {
                                Icon(
                                    Icons.Filled.CheckCircle,
                                    contentDescription = null,
                                    modifier = Modifier.size(56.dp),
                                    tint = androidx.compose.ui.graphics.Color(0xFF4CAF50),
                                )
                                Spacer(modifier = Modifier.height(12.dp))
                                Text("Payment Received!", style = MaterialTheme.typography.titleMedium)
                                Spacer(modifier = Modifier.height(8.dp))
                                Text(
                                    "WBTC will be auto-funded into your privacy pool.",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    textAlign = TextAlign.Center,
                                )
                            } else if (lnInvoice != null) {
                                val qrBitmap = remember(lnInvoice) { generateQrBitmap(lnInvoice!!, 400) }
                                if (qrBitmap != null) {
                                    Image(
                                        bitmap = qrBitmap.asImageBitmap(),
                                        contentDescription = "Lightning Invoice QR",
                                        modifier = Modifier.size(220.dp),
                                    )
                                }
                                Spacer(modifier = Modifier.height(8.dp))
                                Text(
                                    text = "Share this invoice with the sender.",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    textAlign = TextAlign.Center,
                                )
                                Spacer(modifier = Modifier.height(8.dp))
                                Text(
                                    text = lnInvoice!!.take(30) + "...",
                                    style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                                Spacer(modifier = Modifier.height(12.dp))
                                Row(
                                    modifier = Modifier.fillMaxWidth(),
                                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                                ) {
                                    Button(
                                        onClick = {
                                            val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                                            clipboard.setPrimaryClip(ClipData.newPlainText("Invoice", lnInvoice))
                                            android.widget.Toast.makeText(context, "Copied to clipboard", android.widget.Toast.LENGTH_SHORT).show()
                                        },
                                        modifier = Modifier.weight(1f),
                                    ) {
                                        Icon(Icons.Filled.ContentCopy, contentDescription = null, modifier = Modifier.size(16.dp))
                                        Spacer(modifier = Modifier.width(4.dp))
                                        Text("Copy Invoice")
                                    }
                                    OutlinedButton(
                                        onClick = {
                                            shareText(
                                                context = context,
                                                chooserTitle = "Share Lightning invoice",
                                                subject = "Pay me on Lightning with Oubli",
                                                text = lightningInvoiceShareText(
                                                    invoice = lnInvoice!!,
                                                    amountSats = parseBolt11AmountSats(lnInvoice!!),
                                                ),
                                            )
                                        },
                                        modifier = Modifier.weight(1f),
                                    ) {
                                        Icon(Icons.Filled.Share, contentDescription = null, modifier = Modifier.size(16.dp))
                                        Spacer(modifier = Modifier.width(4.dp))
                                        Text("Share Invoice")
                                    }
                                }
                                if (lnError != null) {
                                    Spacer(modifier = Modifier.height(12.dp))
                                    Text(
                                        text = lnError!!,
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.error,
                                        textAlign = TextAlign.Center,
                                    )
                                    if (lnSwapId != null) {
                                        Spacer(modifier = Modifier.height(12.dp))
                                        Button(onClick = { startLnWait(lnSwapId!!) }) {
                                            Text("Retry Payment Check")
                                        }
                                    }
                                }
                                if (lnWaiting) {
                                    Spacer(modifier = Modifier.height(12.dp))
                                    Row(verticalAlignment = Alignment.CenterVertically) {
                                        CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                                        Spacer(modifier = Modifier.width(8.dp))
                                        Text("Waiting for payment...", style = MaterialTheme.typography.bodySmall)
                                    }
                                }
                                if (lnFee != null) {
                                    Spacer(modifier = Modifier.height(8.dp))
                                    Text(
                                        "Fee: ${lnFee} sats",
                                        style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            } else {
                                if (lnError != null) {
                                    Text(
                                        lnError!!,
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.error,
                                        textAlign = TextAlign.Center,
                                    )
                                    Spacer(modifier = Modifier.height(12.dp))
                                }
                                Text(
                                    "Enter amount to receive via Lightning",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                                Spacer(modifier = Modifier.height(12.dp))
                                OutlinedTextField(
                                    value = lnAmountSats,
                                    onValueChange = { lnAmountSats = it },
                                    label = { Text("Amount (sats)") },
                                    modifier = Modifier.fillMaxWidth(),
                                    singleLine = true,
                                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                                )
                                Spacer(modifier = Modifier.height(12.dp))
                                Button(
                                    onClick = {
                                        val amount = lnAmountSats.toULongOrNull()
                                        if (amount != null && amount > 0u) {
                                            lnError = null
                                            onReceiveLightningCreate(amount) { result ->
                                                result
                                                    .onSuccess { quote ->
                                                        lnInvoice = quote.lnInvoice
                                                        lnSwapId = quote.swapId
                                                        lnFee = quote.fee
                                                        startLnWait(quote.swapId)
                                                    }
                                                    .onFailure { e -> lnError = e.message }
                                            }
                                        }
                                    },
                                    enabled = lnAmountSats.toULongOrNull()?.let { it > 0u } == true,
                                ) {
                                    Text("Create Invoice")
                                }
                            }
                            Spacer(modifier = Modifier.height(24.dp))
                        }
                    }
                }
            }
        }
    }
}

private fun generateQrBitmap(content: String, size: Int): Bitmap? {
    return try {
        val writer = QRCodeWriter()
        val bitMatrix = writer.encode(content, BarcodeFormat.QR_CODE, size, size)
        val bitmap = Bitmap.createBitmap(size, size, Bitmap.Config.RGB_565)
        for (x in 0 until size) {
            for (y in 0 until size) {
                bitmap.setPixel(x, y, if (bitMatrix[x, y]) Color.BLACK else Color.WHITE)
            }
        }
        bitmap
    } catch (_: Exception) {
        null
    }
}

@Composable
private fun DebugSettingsDialog(
    currentRpcUrl: String,
    onSave: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var rpcUrl by rememberSaveable { mutableStateOf(currentRpcUrl) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Debug Settings") },
        text = {
            Column {
                Text("Change the RPC endpoint used for Starknet calls.", style = MaterialTheme.typography.bodySmall)
                Spacer(modifier = Modifier.height(16.dp))
                OutlinedTextField(
                    value = rpcUrl,
                    onValueChange = { rpcUrl = it },
                    label = { Text("RPC URL") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                )
            }
        },
        confirmButton = { Button(onClick = { onSave(rpcUrl) }, enabled = rpcUrl.isNotBlank()) { Text("Save") } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

@Composable
private fun ShowSeedPhraseDialog(
    onGetMnemonic: (onResult: (Result<String>) -> Unit) -> Unit,
    onDismiss: () -> Unit,
) {
    var seedWords by remember { mutableStateOf<List<String>?>(null) }
    var error by rememberSaveable { mutableStateOf<String?>(null) }
    var isLoading by rememberSaveable { mutableStateOf(false) }
    val context = LocalContext.current

    val bottomBar: (@Composable () -> Unit)? = if (seedWords != null) {
        { TaskPrimaryButton(title = "Done", onClick = onDismiss) }
    } else {
        {
            TaskPrimaryButton(
                title = if (isLoading) "Loading..." else "Reveal",
                enabled = !isLoading,
            ) {
                isLoading = true
                error = null
                onGetMnemonic { result ->
                    isLoading = false
                    result
                        .onSuccess { mnemonic -> seedWords = mnemonic.split(" ") }
                        .onFailure { e -> error = e.message ?: "Failed to retrieve seed phrase" }
                }
            }
        }
    }

    FullScreenTaskDialog(
        title = if (seedWords != null) "Seed Phrase" else "Show Seed Phrase",
        onDismissRequest = onDismiss,
        dismissEnabled = !isLoading,
        bottomBar = bottomBar,
    ) {
        if (seedWords != null) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState())
                    .padding(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(
                    text = "Write down these words and store them safely. Anyone with these words can access your funds.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                    textAlign = TextAlign.Center,
                )
                Spacer(modifier = Modifier.height(16.dp))
                seedWords!!.forEachIndexed { index, word ->
                    Text(
                        text = "${index + 1}. $word",
                        style = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
                        modifier = Modifier.padding(vertical = 2.dp),
                    )
                }
                Spacer(modifier = Modifier.height(16.dp))
                Button(onClick = {
                    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                    clipboard.setPrimaryClip(ClipData.newPlainText("Seed phrase", seedWords!!.joinToString(" ")))
                    android.widget.Toast.makeText(context, "Copied to clipboard", android.widget.Toast.LENGTH_SHORT).show()
                }) {
                    Icon(Icons.Filled.ContentCopy, contentDescription = null, modifier = Modifier.size(16.dp))
                    Spacer(modifier = Modifier.width(4.dp))
                    Text("Copy to Clipboard")
                }
            }
        } else {
            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                Column(
                    modifier = Modifier.padding(24.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text("Tap Reveal to show your seed phrase.", style = MaterialTheme.typography.bodySmall)
                    if (error != null) {
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(error!!, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                    }
                }
            }
        }
    }
}

/**
 * Check if a BOLT11 invoice includes an amount.
 * Amount is digits + optional multiplier (m/u/n/p) before the '1' separator.
 */
private fun bolt11HasAmount(invoice: String): Boolean {
    val lower = invoice.lowercase()
    val rest = when {
        lower.startsWith("lnbcrt") -> lower.substring(6)
        lower.startsWith("lnbc") -> lower.substring(4)
        lower.startsWith("lntb") -> lower.substring(4)
        else -> return false
    }
    return rest.matches(Regex("^\\d+[munp]?1.*"))
}

private fun normalizeLightningInvoice(value: String): String? {
    val normalized = value
        .trim()
        .lowercase()
        .removePrefix("lightning:")
    return when {
        normalized.startsWith("lnbc") -> normalized
        normalized.startsWith("lntb") -> normalized
        normalized.startsWith("lnbcrt") -> normalized
        else -> null
    }
}

private fun parseBolt11AmountSats(invoice: String): String? {
    val lower = invoice.lowercase()
    val rest = when {
        lower.startsWith("lnbcrt") -> lower.substring(6)
        lower.startsWith("lnbc") -> lower.substring(4)
        lower.startsWith("lntb") -> lower.substring(4)
        else -> return null
    }

    val match = Regex("""^(\d+)([munp]?)1.*""").find(rest) ?: return null
    val base = match.groupValues[1].toBigDecimalOrNull() ?: return null
    val multiplier = match.groupValues[2]

    val sats = when (multiplier) {
        "m" -> base.multiply(BigDecimal("100000"))
        "u" -> base.multiply(BigDecimal("100"))
        "n" -> base.divide(BigDecimal.TEN)
        "p" -> base.divide(BigDecimal("10000"))
        else -> base.multiply(BigDecimal("100000000"))
    }
        .setScale(0, RoundingMode.HALF_UP)

    return sats.toPlainString().takeIf { it != "0" }
}

private fun shareText(
    context: Context,
    chooserTitle: String,
    subject: String,
    text: String,
) {
    val shareIntent = Intent(Intent.ACTION_SEND).apply {
        type = "text/plain"
        putExtra(Intent.EXTRA_SUBJECT, subject)
        putExtra(Intent.EXTRA_TEXT, text)
    }
    context.startActivity(Intent.createChooser(shareIntent, chooserTitle))
}

private fun parseOubliUri(code: String): Pair<String, String?>? {
    val trimmed = code.trim()
    if (!trimmed.lowercase().startsWith("oubli:")) return null
    val rest = trimmed.drop(6)
    val qIndex = rest.indexOf('?')
    if (qIndex < 0) return Pair(rest, null)
    val pubkey = rest.substring(0, qIndex)
    val query = rest.substring(qIndex + 1)
    var amount: String? = null
    for (param in query.split("&")) {
        val parts = param.split("=", limit = 2)
        if (parts.size == 2 && parts[0] == "amount") {
            amount = parts[1]
        }
    }
    return Pair(pubkey, amount)
}

private fun staticReceiveShareText(
    title: String,
    subtitle: String,
    value: String,
): String {
    val descriptor = if (title == "Starknet") "Address" else "Public key"
    return """
        Receive with Oubli
        Type: $title
        $subtitle
        $descriptor: $value
    """.trimIndent()
}

private fun lightningInvoiceShareText(
    invoice: String,
    amountSats: String?,
): String {
    val amountLine = amountSats?.let { "Amount: $it sats" } ?: "Amount: Custom amount"
    return """
        Pay me on Lightning with Oubli
        $amountLine
        Invoice: $invoice
    """.trimIndent()
}
