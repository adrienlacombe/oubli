package com.oubli.wallet.ui.util

import com.oubli.wallet.BuildConfig
import java.security.MessageDigest

data class SupportIssue(
    val message: String,
    val diagnostics: String,
)

enum class SupportContext {
    General,
    AutoFund,
    Send,
    LightningReceive,
    SeedReveal,
}

private enum class ErrorCategory {
    Authentication,
    InsufficientBalance,
    Network,
    Paymaster,
    Busy,
    WalletUnavailable,
    SeedAccess,
    Generic,
}

object ErrorPresentation {
    fun fromThrowable(
        throwable: Throwable,
        context: SupportContext,
        walletState: String? = null,
        source: String? = null,
    ): SupportIssue {
        val rawMessage = throwable.message?.trim().orEmpty()
        val category = classify(rawMessage)
        val diagnostics = buildDiagnostics(
            rawMessage = rawMessage,
            category = category,
            context = context,
            walletState = walletState,
            source = source ?: throwable::class.simpleName,
        )
        return SupportIssue(
            message = userMessage(category, context),
            diagnostics = diagnostics,
        )
    }

    fun fromRawMessage(
        rawMessage: String?,
        context: SupportContext,
        walletState: String? = null,
        source: String? = null,
    ): SupportIssue? {
        val normalized = rawMessage?.trim().orEmpty()
        if (normalized.isEmpty()) {
            return null
        }
        val category = classify(normalized)
        val diagnostics = buildDiagnostics(
            rawMessage = normalized,
            category = category,
            context = context,
            walletState = walletState,
            source = source,
        )
        return SupportIssue(
            message = userMessage(category, context),
            diagnostics = diagnostics,
        )
    }

    private fun classify(rawMessage: String): ErrorCategory {
        val normalized = rawMessage.lowercase()
        return when {
            normalized.contains("biometric") || normalized.contains("auth") || normalized.contains("locked out") ->
                ErrorCategory.Authentication
            normalized.contains("insufficient") || normalized.contains("balance too low") ->
                ErrorCategory.InsufficientBalance
            normalized.contains("paymaster") || normalized.contains("fee sponsorship") ->
                ErrorCategory.Paymaster
            normalized.contains("no active account") || normalized.contains("wallet is unavailable") ->
                ErrorCategory.WalletUnavailable
            normalized.contains("seed") || normalized.contains("mnemonic") ->
                ErrorCategory.SeedAccess
            normalized.contains("operation in progress") || normalized.contains("invalid state") || normalized.contains("already") ->
                ErrorCategory.Busy
            normalized.contains("rpc") || normalized.contains("network") || normalized.contains("request") ||
                normalized.contains("timeout") || normalized.contains("connection") || normalized.contains("transport") ->
                ErrorCategory.Network
            else -> ErrorCategory.Generic
        }
    }

    private fun userMessage(category: ErrorCategory, context: SupportContext): String {
        if (context == SupportContext.AutoFund) {
            return "New funds arrived, but Oubli could not finish moving them into your private balance. Your funds are still safe. Refresh and try again shortly."
        }

        return when (category) {
            ErrorCategory.Authentication ->
                "Authentication failed. Try again."
            ErrorCategory.InsufficientBalance ->
                "Insufficient balance for this action."
            ErrorCategory.Network ->
                when (context) {
                    SupportContext.LightningReceive ->
                        "Oubli could not finish the Lightning receive flow. Check your connection and try again."
                    else ->
                        "Network request failed. Check your connection and try again."
                }
            ErrorCategory.Paymaster ->
                "Fee sponsorship is temporarily unavailable. Try again shortly."
            ErrorCategory.Busy ->
                "Oubli is finishing another action. Wait a moment and try again."
            ErrorCategory.WalletUnavailable ->
                "Wallet is unavailable. Restart the app and try again."
            ErrorCategory.SeedAccess ->
                when (context) {
                    SupportContext.SeedReveal ->
                        "Oubli could not reveal the seed phrase right now. Try again."
                    else ->
                        "Seed phrase access is unavailable right now. Try again."
                }
            ErrorCategory.Generic ->
                when (context) {
                    SupportContext.Send ->
                        "Send failed. Check the amount and recipient, then try again."
                    SupportContext.LightningReceive ->
                        "Oubli could not finish the Lightning receive flow. Try again."
                    SupportContext.SeedReveal ->
                        "Oubli could not reveal the seed phrase right now. Try again."
                    else ->
                        "Something went wrong. Try again."
                }
        }
    }

    private fun buildDiagnostics(
        rawMessage: String,
        category: ErrorCategory,
        context: SupportContext,
        walletState: String?,
        source: String?,
    ): String {
        val fingerprint = MessageDigest.getInstance("SHA-256")
            .digest(rawMessage.toByteArray())
            .joinToString(separator = "") { "%02x".format(it) }
            .take(12)

        return buildString {
            appendLine("Oubli diagnostics")
            appendLine("App: Android ${BuildConfig.VERSION_NAME} (${BuildConfig.VERSION_CODE})")
            appendLine("Context: ${context.name}")
            appendLine("Category: ${category.name}")
            appendLine("Fingerprint: $fingerprint")
            if (!walletState.isNullOrBlank()) {
                appendLine("Wallet state: $walletState")
            }
            if (!source.isNullOrBlank()) {
                appendLine("Source: $source")
            }
        }.trimEnd()
    }
}
