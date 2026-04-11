package com.oubli.wallet.ui.util

import java.math.BigDecimal
import java.math.RoundingMode

/**
 * Normalize a string into a BOLT11 Lightning invoice if it looks like one.
 * Returns null if the input is not a recognizable Lightning invoice.
 */
fun normalizeLightningInvoice(value: String): String? {
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

/**
 * Parse the amount from a BOLT11 invoice, returning the amount in sats as a String.
 * Returns null if the invoice does not contain an amount or is not a valid invoice.
 */
fun parseBolt11AmountSats(invoice: String): String? {
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

/**
 * Parse an oubli: URI returning the public key and optional amount.
 */
fun parseOubliUri(code: String): Pair<String, String?>? {
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
