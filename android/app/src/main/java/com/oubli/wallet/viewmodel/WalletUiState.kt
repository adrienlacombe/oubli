package com.oubli.wallet.viewmodel

import uniffi.oubli.ActivityEventFfi
import uniffi.oubli.ContactFfi
import uniffi.oubli.SeedBackupStateFfi

/**
 * A user-facing message shown via Snackbar.
 * Each message has a unique [id] so the UI can report consumption via [WalletViewModel.onMessageShown].
 */
data class UserMessage(
    val id: Long = System.nanoTime(),
    val text: String,
    val isError: Boolean = false,
)

/**
 * Unified UI state for the entire wallet app.
 */
data class WalletUiState(
    val screenState: ScreenState = ScreenState.Loading,
    val userMessage: UserMessage? = null,
)

/**
 * Sealed hierarchy representing the current screen the user should see.
 */
sealed interface ScreenState {

    data object Loading : ScreenState

    data object Onboarding : ScreenState

    data class Locked(val unlockError: String? = null) : ScreenState

    data class Ready(
        val address: String,
        val publicKey: String,
        val balanceSats: String,
        val pendingSats: String,
        val isBalanceHidden: Boolean = false,
        val showFiat: Boolean = false,
        val fiatCurrency: String = "usd",
        val btcFiatPrice: Double? = null,
        val activity: List<ActivityEventFfi> = emptyList(),
        val contacts: List<ContactFfi> = emptyList(),
        val isRefreshing: Boolean = false,
        val autoFundError: String? = null,
        val activityContactNames: Map<String, String> = emptyMap(),
    ) : ScreenState

    data class Processing(val address: String, val operation: String) : ScreenState

    data class Error(val message: String) : ScreenState

    data class SeedBackup(val backupState: SeedBackupStateFfi) : ScreenState

    data object Wiped : ScreenState
}
