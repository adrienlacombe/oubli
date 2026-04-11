package com.oubli.wallet.ui.util

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Handler
import android.os.Looper

private const val SENSITIVE_CLIPBOARD_CLEAR_DELAY_MS = 15_000L

fun shareText(
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

/** Truncates an address or hash for display: shows first 16 + "..." + last 8 chars. */
fun truncateAddress(value: String): String {
    if (value.length <= 28) return value
    return "${value.take(16)}...${value.takeLast(8)}"
}

fun copySensitiveTextToClipboard(
    context: Context,
    label: String,
    text: String,
) {
    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    clipboard.setPrimaryClip(ClipData.newPlainText(label, text))

    Handler(Looper.getMainLooper()).postDelayed({
        val current = clipboard.primaryClip
            ?.getItemAt(0)
            ?.coerceToText(context)
            ?.toString()
        if (current == text) {
            clipboard.clearPrimaryClip()
        }
    }, SENSITIVE_CLIPBOARD_CLEAR_DELAY_MS)
}

fun staticReceiveShareText(
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

fun lightningInvoiceShareText(
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
