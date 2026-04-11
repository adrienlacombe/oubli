package com.oubli.wallet.ui.balance

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
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
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Public
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import com.oubli.wallet.ui.theme.OubliReceived
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.oubli.wallet.ui.components.FullScreenTaskDialog
import com.oubli.wallet.ui.theme.OubliSuccessBg
import com.oubli.wallet.ui.util.generateQrBitmap
import com.oubli.wallet.ui.util.lightningInvoiceShareText
import com.oubli.wallet.ui.util.parseBolt11AmountSats
import com.oubli.wallet.ui.util.shareText
import com.oubli.wallet.ui.util.staticReceiveShareText
import com.oubli.wallet.viewmodel.LightningReceiveUiState
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.launch
import uniffi.oubli.ActivityEventFfi
import java.util.Locale

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ReceiveDialog(
    address: String,
    publicKey: String,
    onCreateLightningReceiveInvoice: (amountSats: ULong) -> Unit,
    lightningReceiveState: LightningReceiveUiState,
    onRetryLightningReceiveWait: () -> Unit,
    onClearLightningReceiveState: () -> Unit,
    onMarkLightningReceiveExpired: () -> Unit,
    onDismiss: () -> Unit,
    onShowMessage: (String) -> Unit = {},
    satsToFiatRaw: (String) -> String? = { null },
    fiatToSats: (String) -> String? = { null },
    fiatCurrency: String = "usd",
    fiatSymbol: String = "$",
    incomingPaymentFlow: SharedFlow<ActivityEventFfi>? = null,
) {
    val context = LocalContext.current
    val pagerState = rememberPagerState(pageCount = { 3 })
    val scope = rememberCoroutineScope()

    // Incoming payment success overlay
    var receivedPayment by remember { mutableStateOf<ActivityEventFfi?>(null) }
    LaunchedEffect(Unit) {
        incomingPaymentFlow?.collect { event ->
            receivedPayment = event
        }
    }
    // Auto-dismiss success overlay after 4 seconds
    LaunchedEffect(receivedPayment) {
        if (receivedPayment != null) {
            kotlinx.coroutines.delay(4000)
            receivedPayment = null
        }
    }

    // Oubli receive amount (optional)
    var oubliAmountSats by rememberSaveable { mutableStateOf("") }
    var starknetAmountSats by rememberSaveable { mutableStateOf("") }

    var lnAmountSats by rememberSaveable { mutableStateOf("") }
    var lnExpiryRemaining by remember { mutableIntStateOf(0) }

    // Expiry countdown
    LaunchedEffect(lightningReceiveState.expiryEpochSeconds, lightningReceiveState.isSuccess) {
        val exp = lightningReceiveState.expiryEpochSeconds ?: return@LaunchedEffect
        while (true) {
            val now = System.currentTimeMillis() / 1000
            val remaining = (exp - now).toInt()
            if (remaining <= 0) {
                lnExpiryRemaining = 0
                if (!lightningReceiveState.isSuccess) {
                    onMarkLightningReceiveExpired()
                }
                break
            }
            lnExpiryRemaining = remaining
            kotlinx.coroutines.delay(1000)
        }
    }

    val dismissReceiveDialog = {
        val shouldPreserveState = lightningReceiveState.swapId != null && !lightningReceiveState.isSuccess
        if (!shouldPreserveState) {
            onClearLightningReceiveState()
        }
        onDismiss()
    }

    FullScreenTaskDialog(
        title = if (receivedPayment != null) "Payment Received" else "Receive",
        onDismissRequest = dismissReceiveDialog,
    ) {
        // Success overlay when an incoming payment is detected
        if (receivedPayment != null) {
            androidx.compose.foundation.layout.Box(
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
                        contentDescription = "Payment received",
                        modifier = Modifier.size(72.dp),
                        tint = OubliReceived,
                    )
                    Spacer(modifier = Modifier.height(16.dp))
                    Text(
                        text = "${receivedPayment!!.amountSats ?: ""} sats received",
                        style = MaterialTheme.typography.headlineSmall,
                        textAlign = TextAlign.Center,
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = "New ${receivedPayment!!.eventType.lowercase()} detected",
                        style = MaterialTheme.typography.bodyMedium,
                        textAlign = TextAlign.Center,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            return@FullScreenTaskDialog
        }
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
                            com.oubli.wallet.ui.components.DualAmountInput(
                                satsAmount = oubliAmountSats,
                                onSatsChange = { oubliAmountSats = it },
                                satsToFiatRaw = satsToFiatRaw,
                                fiatToSats = fiatToSats,
                                fiatCurrency = fiatCurrency,
                                fiatSymbol = fiatSymbol,
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
                                        onShowMessage("Copied to clipboard")
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
                        // Starknet tab with optional amount
                        val value = address
                        val shareValue = if (starknetAmountSats.isNotEmpty()) {
                            "$value (requesting $starknetAmountSats sats)"
                        } else {
                            value
                        }
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
                            Spacer(modifier = Modifier.height(8.dp))
                            com.oubli.wallet.ui.components.DualAmountInput(
                                satsAmount = starknetAmountSats,
                                onSatsChange = { starknetAmountSats = it },
                                satsToFiatRaw = satsToFiatRaw,
                                fiatToSats = fiatToSats,
                                fiatCurrency = fiatCurrency,
                                fiatSymbol = fiatSymbol,
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
                                        onShowMessage("Copied to clipboard")
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
                                                value = shareValue,
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
                        val lightningInvoice = lightningReceiveState.invoice
                        val lightningError = lightningReceiveState.errorMessage

                        Column(
                            modifier = Modifier
                                .fillMaxSize()
                                .verticalScroll(rememberScrollState())
                                .padding(24.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                        ) {
                            Spacer(modifier = Modifier.height(24.dp))
                            if (lightningReceiveState.isSuccess) {
                                Icon(
                                    Icons.Filled.CheckCircle,
                                    contentDescription = null,
                                    modifier = Modifier.size(56.dp),
                                    tint = OubliReceived,
                                )
                                Spacer(modifier = Modifier.height(12.dp))
                                Text("Payment Received!", style = MaterialTheme.typography.titleMedium)
                            } else if (lightningInvoice != null) {
                                val qrBitmap = remember(lightningInvoice) { generateQrBitmap(lightningInvoice, 400) }
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
                                    text = if (lightningInvoice.length > 30) "${lightningInvoice.take(30)}..." else lightningInvoice,
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
                                            clipboard.setPrimaryClip(ClipData.newPlainText("Invoice", lightningInvoice))
                                            onShowMessage("Copied to clipboard")
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
                                                    invoice = lightningInvoice,
                                                    amountSats = parseBolt11AmountSats(lightningInvoice),
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
                                if (lightningError != null) {
                                    Spacer(modifier = Modifier.height(12.dp))
                                    Text(
                                        text = lightningError,
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.error,
                                        textAlign = TextAlign.Center,
                                    )
                                    if (lightningReceiveState.swapId != null) {
                                        Spacer(modifier = Modifier.height(12.dp))
                                        Button(onClick = onRetryLightningReceiveWait) {
                                            Text("Retry Payment Check")
                                        }
                                    }
                                }
                                if (lightningReceiveState.isWaiting) {
                                    Spacer(modifier = Modifier.height(12.dp))
                                    Row(verticalAlignment = Alignment.CenterVertically) {
                                        CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                                        Spacer(modifier = Modifier.width(8.dp))
                                        Text("Waiting for payment...", style = MaterialTheme.typography.bodySmall)
                                    }
                                    if (lnExpiryRemaining > 0) {
                                        val minutes = lnExpiryRemaining / 60
                                        val seconds = lnExpiryRemaining % 60
                                        Spacer(modifier = Modifier.height(4.dp))
                                        Text(
                                            text = "Expires in $minutes:${String.format(Locale.ROOT, "%02d", seconds)}",
                                            style = MaterialTheme.typography.labelSmall,
                                            color = if (lnExpiryRemaining < 60) MaterialTheme.colorScheme.error
                                                else MaterialTheme.colorScheme.onSurfaceVariant,
                                        )
                                    }
                                }
                                if (lightningReceiveState.feeSats != null) {
                                    Spacer(modifier = Modifier.height(8.dp))
                                    Text(
                                        "Fee: ${lightningReceiveState.feeSats} sats",
                                        style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            } else if (lightningReceiveState.isCreating) {
                                Spacer(modifier = Modifier.height(48.dp))
                                CircularProgressIndicator(modifier = Modifier.size(40.dp))
                                Spacer(modifier = Modifier.height(16.dp))
                                Text(
                                    "Preparing wallet...",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    textAlign = TextAlign.Center,
                                )
                            } else {
                                if (lightningError != null) {
                                    Text(
                                        lightningError,
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
                                com.oubli.wallet.ui.components.DualAmountInput(
                                    satsAmount = lnAmountSats,
                                    onSatsChange = { lnAmountSats = it },
                                    satsToFiatRaw = satsToFiatRaw,
                                    fiatToSats = fiatToSats,
                                    fiatCurrency = fiatCurrency,
                                    fiatSymbol = fiatSymbol,
                                )
                                Spacer(modifier = Modifier.height(12.dp))
                                Button(
                                    onClick = {
                                        val amount = lnAmountSats.toULongOrNull()
                                        if (amount != null && amount > 0u) {
                                            onCreateLightningReceiveInvoice(amount)
                                        }
                                    },
                                    enabled = !lightningReceiveState.isCreating &&
                                        !lightningReceiveState.isWaiting &&
                                        lnAmountSats.toULongOrNull()?.let { it > 0u } == true,
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
