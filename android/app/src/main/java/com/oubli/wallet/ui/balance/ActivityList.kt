package com.oubli.wallet.ui.balance

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowDownward
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.Autorenew
import androidx.compose.material.icons.filled.Bolt
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.oubli.wallet.ui.theme.OubliError
import com.oubli.wallet.ui.theme.OubliPending
import com.oubli.wallet.ui.theme.OubliReceived
import com.oubli.wallet.ui.theme.OubliSent
import com.oubli.wallet.ui.util.truncateAddress
import uniffi.oubli.ActivityEventFfi
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle

@Composable
fun ActivityList(
    activity: List<ActivityEventFfi>,
    autoFundError: String?,
    contactNames: Map<String, String> = emptyMap(),
    balanceSats: String = "0",
    onShowMessage: (String) -> Unit = {},
    onOpenDetails: (ActivityEventFfi) -> Unit = {},
) {
    Text(
        text = "Activity",
        style = MaterialTheme.typography.titleSmall,
        modifier = Modifier
            .fillMaxWidth()
            .padding(bottom = 8.dp)
            .semantics { heading() },
    )

    if (activity.isEmpty()) {
        Card(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
            shape = RoundedCornerShape(16.dp),
        ) {
            Column(
                modifier = Modifier.padding(32.dp).fillMaxWidth(),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Icon(
                    Icons.Filled.ArrowDownward,
                    contentDescription = null,
                    modifier = Modifier.size(40.dp),
                    tint = MaterialTheme.colorScheme.primary,
                )
                Spacer(modifier = Modifier.height(12.dp))
                Text(
                    text = "No transactions yet",
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = FontWeight.Medium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Spacer(modifier = Modifier.height(4.dp))
                if (balanceSats == "0") {
                    Text(
                        text = "Tap Receive to get your first payment",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.primary,
                    )
                } else {
                    Text(
                        text = "No transactions yet",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    } else {
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            activity.forEach { event ->
                ActivityItem(
                    event = event,
                    contactName = contactNames[event.txHash],
                    onOpenDetails = { onOpenDetails(event) },
                )
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
                    onShowMessage("Copied to clipboard")
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

@Composable
private fun ActivityItem(
    event: ActivityEventFfi,
    contactName: String? = null,
    onOpenDetails: () -> Unit,
) {
    val info = activityInfo(event.eventType)
    val isIncoming = event.eventType in listOf("Fund", "TransferIn", "Rollover")
    val status = normalizedStatus(event)
    val secondaryMeta = formattedTimestamp(event) ?: if (event.blockNumber > 0UL) {
        "Block ${event.blockNumber}"
    } else {
        "Awaiting confirmation"
    }

    val amountDesc = event.amountSats?.let { sats ->
        val sign = if (isIncoming) "plus" else "minus"
        "$sign $sats sats"
    } ?: ""
    val rowDescription = "${info.label}: $amountDesc, $status"

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp, horizontal = 4.dp)
            .clickable(onClick = onOpenDetails)
            .semantics(mergeDescendants = true) {
                contentDescription = rowDescription
            },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Leading icon
        Box(
            modifier = Modifier
                .size(40.dp)
                .background(
                    color = info.iconBg,
                    shape = CircleShape,
                ),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                imageVector = info.icon,
                contentDescription = null,
                modifier = Modifier.size(20.dp),
                tint = info.iconTint,
            )
        }

        Spacer(modifier = Modifier.width(12.dp))

        // Label + contact name or block
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = info.label,
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.Medium,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = contactName ?: truncateAddress(event.txHash),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
            )
            Spacer(modifier = Modifier.height(4.dp))
            Row(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                StatusChip(status = status)
                Text(
                    text = secondaryMeta,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        // Amount
        if (event.amountSats != null) {
            val sign = if (isIncoming) "+" else "-"
            val color = if (isIncoming) OubliReceived else OubliSent
            Text(
                text = "$sign${event.amountSats} sats",
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.SemiBold,
                color = color,
            )
        }
    }
}

@Composable
private fun StatusChip(status: String) {
    val normalized = status.ifBlank { "Unknown" }
    val (container, content) = when (normalized) {
        "Confirmed" -> Pair(OubliReceived.copy(alpha = 0.18f), OubliReceived)
        "Pending" -> Pair(OubliPending.copy(alpha = 0.2f), OubliPending)
        "Failed" -> Pair(OubliError.copy(alpha = 0.18f), OubliError)
        else -> Pair(MaterialTheme.colorScheme.surfaceContainerHigh, MaterialTheme.colorScheme.onSurfaceVariant)
    }

    Surface(
        color = container,
        shape = RoundedCornerShape(999.dp),
    ) {
        Text(
            text = normalized,
            style = MaterialTheme.typography.labelSmall,
            color = content,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
        )
    }
}

@Composable
fun TransactionDetailsDialog(
    event: ActivityEventFfi,
    title: String,
    contactName: String?,
    onDismiss: () -> Unit,
    onShowMessage: (String) -> Unit = {},
) {
    val context = LocalContext.current
    val status = normalizedStatus(event)

    androidx.compose.material3.AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Transaction") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                StatusChip(status = status)
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                )
                contactName?.takeIf { it.isNotBlank() }?.let { name ->
                    Text(
                        text = name,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                event.amountSats?.let { amount ->
                    val sign = if (event.eventType in listOf("Fund", "TransferIn", "Rollover")) "+" else "-"
                    Text(
                        text = "$sign$amount sats",
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                    )
                }
                DetailLine("Status", status)
                formattedTimestamp(event)?.let { timestamp ->
                    DetailLine("Time", timestamp)
                }
                if (event.blockNumber > 0UL) {
                    DetailLine("Block", event.blockNumber.toString())
                }
                DetailLine("Hash", event.txHash, monospaced = true)
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Button(
                        onClick = {
                            val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                            clipboard.setPrimaryClip(ClipData.newPlainText("Transaction hash", event.txHash))
                            onShowMessage("Copied transaction hash")
                        },
                    ) {
                        Text("Copy Hash")
                    }
                    event.explorerUrl?.takeIf { it.isNotBlank() }?.let { explorerUrl ->
                        TextButton(
                            onClick = { openExplorer(context, explorerUrl) },
                        ) {
                            Text("View Explorer")
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("Done") }
        },
    )
}

@Composable
private fun DetailLine(label: String, value: String, monospaced: Boolean = false) {
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = value,
            style = if (monospaced) {
                MaterialTheme.typography.bodySmall.copy(fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace)
            } else {
                MaterialTheme.typography.bodyMedium
            },
        )
    }
}

private fun normalizedStatus(event: ActivityEventFfi): String {
    return event.status.ifBlank {
        if (event.blockNumber == 0UL) "Pending" else "Confirmed"
    }
}

private fun formattedTimestamp(event: ActivityEventFfi): String? {
    val timestamp = event.timestampSecs ?: return null
    return DateTimeFormatter
        .ofLocalizedDateTime(FormatStyle.MEDIUM, FormatStyle.SHORT)
        .withZone(ZoneId.systemDefault())
        .format(Instant.ofEpochSecond(timestamp.toLong()))
}

private fun openExplorer(context: Context, explorerUrl: String) {
    val intent = Intent(Intent.ACTION_VIEW, Uri.parse(explorerUrl))
    context.startActivity(intent)
}

private data class ActivityInfo(
    val label: String,
    val icon: ImageVector,
    val iconBg: Color,
    val iconTint: Color,
)

private fun activityInfo(type: String): ActivityInfo = when (type) {
    "Fund" -> ActivityInfo("Received", Icons.Filled.ArrowDownward, Color(0xFF1B3A1B), OubliReceived)
    "TransferIn" -> ActivityInfo("Received", Icons.Filled.ArrowDownward, Color(0xFF1B3A1B), OubliReceived)
    "TransferOut" -> ActivityInfo("Sent", Icons.Filled.ArrowUpward, Color(0xFF3A1B1B), OubliSent)
    "Withdraw" -> ActivityInfo("Sent", Icons.Filled.ArrowUpward, Color(0xFF3A1B1B), OubliSent)
    "Rollover" -> ActivityInfo("Settled", Icons.Filled.Autorenew, Color(0xFF2A2A1B), OubliPending)
    "Ragequit" -> ActivityInfo("Emergency Exit", Icons.Filled.ErrorOutline, Color(0xFF3A1B1B), OubliError)
    else -> ActivityInfo(type, Icons.Filled.Bolt, Color(0xFF1B2A3A), Color(0xFFADC6FF))
}
