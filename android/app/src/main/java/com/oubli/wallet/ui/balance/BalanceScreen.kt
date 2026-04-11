package com.oubli.wallet.ui.balance

import android.view.HapticFeedbackConstants
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.foundation.background
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.filled.ArrowDownward
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.CameraAlt
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.CurrencyExchange
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.oubli.wallet.ui.QRScannerScreen
import com.oubli.wallet.viewmodel.WalletViewModel
import com.oubli.wallet.viewmodel.LightningReceiveUiState
import com.oubli.wallet.viewmodel.LightningSendUiState
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.sp
import com.oubli.wallet.ui.util.SupportIssue
import uniffi.oubli.ActivityEventFfi
import uniffi.oubli.ContactFfi

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BalanceScreen(
    address: String,
    publicKey: String,
    balanceSats: String,
    pendingSats: String,
    isBalanceHidden: Boolean,
    showFiat: Boolean = false,
    fiatCurrency: String = "usd",
    onToggleCurrency: () -> Unit = {},
    onSetFiatCurrency: (String) -> Unit = {},
    satsToFiat: (String) -> String? = { null },
    activity: List<ActivityEventFfi> = emptyList(),
    onToggleBalanceHidden: () -> Unit,
    onRefresh: () -> Unit,
    onSend: (amountSats: String, recipient: String) -> Unit,
    onCalculateSendFee: (amountSats: String, recipient: String) -> String = { _, _ -> "0" },
    onStartLightningPayment: (bolt11: String) -> Unit = {},
    lightningOperation: String? = null,
    lightningSendState: LightningSendUiState = LightningSendUiState(),
    onClearLightningSendState: () -> Unit = {},
    onCreateLightningReceiveInvoice: (amountSats: ULong) -> Unit = {},
    lightningReceiveState: LightningReceiveUiState = LightningReceiveUiState(),
    onRetryLightningReceiveWait: () -> Unit = {},
    onClearLightningReceiveState: () -> Unit = {},
    onMarkLightningReceiveExpired: () -> Unit = {},
    onGetMnemonic: (onResult: (Result<String>) -> Unit) -> Unit = { _ -> },
    contacts: List<ContactFfi> = emptyList(),
    onSaveContact: (ContactFfi) -> Unit = {},
    onDeleteContact: (String) -> Unit = {},
    satsToFiatRaw: (String) -> String? = { null },
    fiatToSats: (String) -> String? = { null },
    autoFundIssue: SupportIssue? = null,
    isRefreshing: Boolean = false,
    activityContactNames: Map<String, String> = emptyMap(),
    onShowMessage: (String) -> Unit = {},
) {
    var showDialog by rememberSaveable { mutableStateOf<ActionDialog?>(null) }
    var showMenu by remember { mutableStateOf(false) }
    var fiatPriceTimedOut by remember { mutableStateOf(false) }
    var selectedActivity by remember { mutableStateOf<ActivityEventFfi?>(null) }

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
        // Hamburger menu
        Row(
            modifier = Modifier.fillMaxWidth(),
        ) {
            Box {
                IconButton(onClick = { showMenu = true }) {
                    Icon(Icons.Filled.Menu, contentDescription = "Menu")
                }
                DropdownMenu(
                    expanded = showMenu,
                    onDismissRequest = { showMenu = false },
                ) {
                    DropdownMenuItem(
                        text = { Text("Fiat Currency (${fiatCurrency.uppercase()})") },
                        onClick = { showMenu = false; showDialog = ActionDialog.FiatCurrency },
                        leadingIcon = { Icon(Icons.Filled.CurrencyExchange, contentDescription = null) },
                    )
                    DropdownMenuItem(
                        text = { Text("Contacts") },
                        onClick = { showMenu = false; showDialog = ActionDialog.Contacts },
                        leadingIcon = { Icon(Icons.Filled.Person, contentDescription = null) },
                    )
                    DropdownMenuItem(
                        text = { Text("Show Seed Phrase") },
                        onClick = { showMenu = false; showDialog = ActionDialog.ShowSeedPhrase },
                        leadingIcon = { Icon(Icons.Filled.Visibility, contentDescription = null) },
                    )
                }
            }
        }

        Spacer(modifier = Modifier.height(8.dp))

        // Balance display — responsive font size
        val balanceText = when {
            isBalanceHidden -> "\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022"
            showFiat -> satsToFiat(balanceSats) ?: ""
            else -> balanceSats
        }
        val balanceFontSize = when {
            balanceText.length >= 10 -> 28.sp
            balanceText.length >= 7 -> 36.sp
            else -> 48.sp
        }
        val showLoading = showFiat && !isBalanceHidden && satsToFiat(balanceSats) == null && !fiatPriceTimedOut

        LaunchedEffect(showFiat, satsToFiat(balanceSats)) {
            if (showFiat && satsToFiat(balanceSats) == null) {
                fiatPriceTimedOut = false
                kotlinx.coroutines.delay(8000)
                if (satsToFiat(balanceSats) == null) {
                    fiatPriceTimedOut = true
                }
            }
        }

        Box(
            modifier = Modifier.height(56.dp),
            contentAlignment = Alignment.Center,
        ) {
            AnimatedContent(
                targetState = balanceText,
                transitionSpec = { fadeIn() togetherWith fadeOut() },
                label = "balance",
            ) { text ->
                if (showLoading) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(40.dp).padding(4.dp),
                        strokeWidth = 3.dp,
                    )
                } else {
                    Text(
                        text = text,
                        fontSize = balanceFontSize,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.onSurface,
                        textAlign = TextAlign.Center,
                        modifier = Modifier
                            .clickable(
                                onClickLabel = if (isBalanceHidden) "Show balance" else "Hide balance",
                            ) { onToggleBalanceHidden() }
                            .semantics {
                                if (isBalanceHidden) {
                                    contentDescription = "Balance hidden. Tap to show."
                                }
                            },
                    )
                }
            }
        }
        if (fiatPriceTimedOut && showFiat) {
            Text(
                text = "Price unavailable",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = if (showFiat && !fiatPriceTimedOut) fiatCurrency.uppercase() else "sats",
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.5f),
            modifier = Modifier.clickable(
                onClickLabel = "Switch currency",
            ) { onToggleCurrency() },
        )
        if (!isBalanceHidden) {
            val pendingVal = pendingSats.toLongOrNull() ?: 0L
            if (pendingVal > 0L) {
                Spacer(modifier = Modifier.height(4.dp))
                val pendingDisplay = if (showFiat) {
                    "+${satsToFiat(pendingSats) ?: pendingSats} incoming"
                } else {
                    "+$pendingSats sats incoming"
                }
                Text(
                    text = pendingDisplay,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.5f),
                    modifier = Modifier.semantics {
                        contentDescription = "Pending: $pendingSats sats incoming"
                    },
                )
            }
        }

        Spacer(modifier = Modifier.height(32.dp))

        // Action buttons — Send, Scan, Receive
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceEvenly,
        ) {
            ActionCircleButton(
                icon = Icons.Filled.ArrowUpward,
                label = "Send",
                onClick = { showDialog = ActionDialog.Send },
            )
            ActionCircleButton(
                icon = Icons.Filled.CameraAlt,
                label = "Scan",
                onClick = { showDialog = ActionDialog.Scan },
            )
            ActionCircleButton(
                icon = Icons.Filled.ArrowDownward,
                label = "Receive",
                onClick = { showDialog = ActionDialog.Receive },
            )
        }

        Spacer(modifier = Modifier.height(32.dp))

        // Activity section
        ActivityList(
            activity = activity,
            autoFundIssue = autoFundIssue,
            contactNames = activityContactNames,
            balanceSats = balanceSats,
            onShowMessage = onShowMessage,
            onOpenDetails = { selectedActivity = it },
        )
    }
    } // PullToRefreshBox

    // ---- Dialogs ----

    when (showDialog) {
        ActionDialog.Send -> {
            SendDialog(
                balanceSats = balanceSats,
                calculateSendFee = onCalculateSendFee,
                onConfirm = { amount, recipient ->
                    onSend(amount, recipient)
                    showDialog = null
                },
                onStartLightningPayment = onStartLightningPayment,
                lightningOperation = lightningOperation,
                lightningSendState = lightningSendState,
                onClearLightningSendState = onClearLightningSendState,
                onDismiss = { showDialog = null },
                contacts = contacts,
                satsToFiatRaw = satsToFiatRaw,
                fiatToSats = fiatToSats,
                fiatCurrency = fiatCurrency,
                fiatSymbol = WalletViewModel.fiatSymbol(fiatCurrency),
            )
        }
        ActionDialog.Scan -> {
            var scannedCode by remember { mutableStateOf<String?>(null) }
            if (scannedCode != null) {
                // Route scanned code to Send dialog
                SendDialog(
                    balanceSats = balanceSats,
                    calculateSendFee = onCalculateSendFee,
                    onConfirm = { amount, recipient ->
                        onSend(amount, recipient)
                        showDialog = null
                    },
                    onStartLightningPayment = onStartLightningPayment,
                    lightningOperation = lightningOperation,
                    lightningSendState = lightningSendState,
                    onClearLightningSendState = onClearLightningSendState,
                    onDismiss = { showDialog = null },
                    initialRecipient = scannedCode!!,
                    satsToFiatRaw = satsToFiatRaw,
                    fiatToSats = fiatToSats,
                    fiatCurrency = fiatCurrency,
                    fiatSymbol = WalletViewModel.fiatSymbol(fiatCurrency),
                )
            } else {
                Dialog(
                    onDismissRequest = { showDialog = null },
                    properties = DialogProperties(usePlatformDefaultWidth = false),
                ) {
                    QRScannerScreen(
                        onCodeScanned = { code -> scannedCode = code },
                        onClose = { showDialog = null },
                    )
                }
            }
        }
        ActionDialog.Receive -> {
            ReceiveDialog(
                address = address,
                publicKey = publicKey,
                onCreateLightningReceiveInvoice = onCreateLightningReceiveInvoice,
                lightningReceiveState = lightningReceiveState,
                onRetryLightningReceiveWait = onRetryLightningReceiveWait,
                onClearLightningReceiveState = onClearLightningReceiveState,
                onMarkLightningReceiveExpired = onMarkLightningReceiveExpired,
                onDismiss = { showDialog = null },
                onShowMessage = onShowMessage,
                satsToFiatRaw = satsToFiatRaw,
                fiatToSats = fiatToSats,
                fiatCurrency = fiatCurrency,
                fiatSymbol = WalletViewModel.fiatSymbol(fiatCurrency),
            )
        }
ActionDialog.ShowSeedPhrase -> {
            ShowSeedPhraseDialog(
                onGetMnemonic = onGetMnemonic,
                onDismiss = { showDialog = null },
                onShowMessage = onShowMessage,
            )
        }
        ActionDialog.FiatCurrency -> {
            FiatCurrencyDialog(
                currentCurrency = fiatCurrency,
                onSelect = { code -> onSetFiatCurrency(code); showDialog = null },
                onDismiss = { showDialog = null },
            )
        }
        ActionDialog.Contacts -> {
            ContactListDialog(
                contacts = contacts,
                onSave = onSaveContact,
                onDelete = onDeleteContact,
                onDismiss = { showDialog = null },
            )
        }
        null -> {}
    }

    selectedActivity?.let { event ->
        TransactionDetailsDialog(
            event = event,
            title = when (event.eventType) {
                "Fund", "TransferIn" -> "Received"
                "TransferOut", "Withdraw" -> "Sent"
                "Rollover" -> "Settled"
                "Ragequit" -> "Emergency Exit"
                else -> event.eventType
            },
            contactName = activityContactNames[event.txHash],
            onDismiss = { selectedActivity = null },
            onShowMessage = onShowMessage,
        )
    }
}

@Composable
private fun ActionCircleButton(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    label: String,
    onClick: () -> Unit,
) {
    val view = LocalView.current
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        IconButton(
            onClick = {
                view.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
                onClick()
            },
            modifier = Modifier
                .size(64.dp)
                .background(
                    color = MaterialTheme.colorScheme.primary,
                    shape = CircleShape,
                ),
        ) {
            Icon(
                imageVector = icon,
                contentDescription = label,
                tint = MaterialTheme.colorScheme.onPrimary,
                modifier = Modifier.size(28.dp),
            )
        }
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = label,
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

private enum class ActionDialog {
    Send, Receive, Scan, ShowSeedPhrase, FiatCurrency, Contacts
}

@Composable
private fun FiatCurrencyDialog(
    currentCurrency: String,
    onSelect: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Fiat Currency") },
        text = {
            Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                WalletViewModel.supportedFiatCurrencies.forEach { (code, name) ->
                    val symbol = WalletViewModel.fiatSymbol(code)
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { onSelect(code) }
                            .padding(vertical = 12.dp, horizontal = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = "$symbol $name (${code.uppercase()})",
                            modifier = Modifier.weight(1f),
                        )
                        if (code == currentCurrency) {
                            Icon(
                                imageVector = Icons.Filled.Check,
                                contentDescription = "Selected",
                                tint = MaterialTheme.colorScheme.primary,
                            )
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        },
    )
}
