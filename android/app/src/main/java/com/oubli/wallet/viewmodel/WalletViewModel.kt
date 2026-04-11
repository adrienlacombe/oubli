package com.oubli.wallet.viewmodel

import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.annotation.VisibleForTesting
import com.oubli.wallet.data.WalletRepository
import com.oubli.wallet.ui.util.ErrorPresentation
import com.oubli.wallet.ui.util.SupportContext
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.oubli.ContactFfi
import uniffi.oubli.OubliException
import uniffi.oubli.WalletStateFfi
import javax.inject.Inject
import javax.inject.Qualifier

@Qualifier
@Retention(AnnotationRetention.BINARY)
annotation class IoDispatcher

@HiltViewModel
class WalletViewModel @Inject constructor(
    private val repository: WalletRepository,
    @IoDispatcher private val ioDispatcher: CoroutineDispatcher,
) : ViewModel() {

    @VisibleForTesting
    internal var activityPollingEnabled: Boolean = true

    private val _uiState = MutableStateFlow(WalletUiState())
    val uiState: StateFlow<WalletUiState> = _uiState.asStateFlow()

    /** Dialog-scoped flow for lightning operation progress messages. */
    private val _lightningOperation = MutableStateFlow<String?>(null)
    val lightningOperation: StateFlow<String?> = _lightningOperation.asStateFlow()

    private var activityPollingJob: Job? = null

    // ---- Initialization ----

    fun attach(activity: FragmentActivity) {
        if (repository.isInitialized) return
        viewModelScope.launch(ioDispatcher) {
            try {
                repository.initialize(activity)
                refreshState()
            } catch (e: OubliException) {
                emitError(e, context = SupportContext.General, source = "initialize")
            }
        }
    }

    // ---- State refresh ----

    private suspend fun refreshState() {
        try {
            val state = repository.getState()
            _uiState.update { current ->
                current.copy(screenState = mapFfiStateToScreen(state, current.screenState))
            }
            updateActivityPolling(state.state)
        } catch (e: Exception) {
            emitError(e, context = SupportContext.General, source = "refresh_state")
        }
    }

    private fun mapFfiStateToScreen(
        state: uniffi.oubli.WalletStateInfo,
        currentScreen: ScreenState = _uiState.value.screenState,
    ): ScreenState {
        return when (state.state) {
            WalletStateFfi.ONBOARDING -> ScreenState.Onboarding
            WalletStateFfi.LOCKED -> ScreenState.Locked()
            WalletStateFfi.READY -> {
                val autoFundIssue = ErrorPresentation.fromRawMessage(
                    rawMessage = state.autoFundError,
                    context = SupportContext.AutoFund,
                    walletState = state.state.name,
                    source = "ffi_auto_fund",
                )
                val currentReady = currentScreen as? ScreenState.Ready
                ScreenState.Ready(
                    address = state.address.orEmpty(),
                    publicKey = state.publicKey.orEmpty(),
                    balanceSats = state.balanceSats ?: "0",
                    pendingSats = state.pendingSats ?: "0",
                    isBalanceHidden = currentReady?.isBalanceHidden ?: false,
                    showFiat = currentReady?.showFiat ?: false,
                    fiatCurrency = currentReady?.fiatCurrency ?: repository.getSavedFiatCurrency(),
                    btcFiatPrice = currentReady?.btcFiatPrice,
                    activity = currentReady?.activity ?: emptyList(),
                    contacts = currentReady?.contacts ?: emptyList(),
                    isRefreshing = false,
                    autoFundIssue = autoFundIssue,
                    activityContactNames = currentReady?.activityContactNames ?: emptyMap(),
                )
            }
            WalletStateFfi.PROCESSING -> ScreenState.Processing(
                address = state.address.orEmpty(),
                operation = state.operation ?: "Processing...",
            )
            WalletStateFfi.ERROR -> ScreenState.Error(
                message = ErrorPresentation
                    .fromRawMessage(
                        rawMessage = state.errorMessage,
                        context = SupportContext.General,
                        walletState = state.state.name,
                        source = "ffi_state_error",
                    )
                    ?.message
                    ?: "Something went wrong. Try again.",
                diagnostics = ErrorPresentation
                    .fromRawMessage(
                        rawMessage = state.errorMessage,
                        context = SupportContext.General,
                        walletState = state.state.name,
                        source = "ffi_state_error",
                    )
                    ?.diagnostics,
            )
            WalletStateFfi.SEED_BACKUP -> {
                // Seed backup state is set separately via startSeedBackup
                val current = _uiState.value.screenState
                if (current is ScreenState.SeedBackup) current
                else ScreenState.Loading
            }
            WalletStateFfi.WIPED -> ScreenState.Wiped
        }
    }

    private fun updateActivityPolling(walletState: WalletStateFfi) {
        if (!activityPollingEnabled) {
            activityPollingJob?.cancel()
            activityPollingJob = null
            return
        }
        val shouldPoll = walletState == WalletStateFfi.READY || walletState == WalletStateFfi.PROCESSING
        if (shouldPoll && activityPollingJob == null) {
            refreshBtcPrice()
            loadContacts()
            activityPollingJob = viewModelScope.launch(ioDispatcher) {
                while (true) {
                    delay(2000)
                    try {
                        val state = repository.getState()
                        val events = try {
                            repository.getActivity()
                        } catch (_: Exception) {
                            repository.getCachedActivity()
                        }
                        val contacts = (_uiState.value.screenState as? ScreenState.Ready)?.contacts ?: emptyList()
                        val nameMap = buildContactNameMap(events, contacts)
                        _uiState.update { current ->
                            val refreshedScreen = mapFfiStateToScreen(state, current.screenState)
                            if (refreshedScreen is ScreenState.Ready) {
                                current.copy(
                                    screenState = refreshedScreen.copy(
                                        activity = events,
                                        activityContactNames = nameMap,
                                    ),
                                )
                            } else {
                                current.copy(screenState = refreshedScreen)
                            }
                        }
                    } catch (_: Exception) {
                        // Polling failure is non-fatal
                    }
                }
            }
        } else if (!shouldPoll && activityPollingJob != null) {
            activityPollingJob?.cancel()
            activityPollingJob = null
        }
    }

    // ---- Onboarding ----

    fun generateMnemonic(onResult: (String) -> Unit) {
        viewModelScope.launch(ioDispatcher) {
            try {
                val mnemonic = repository.generateMnemonic()
                launch(Dispatchers.Main) { onResult(mnemonic) }
            } catch (e: Exception) {
                emitError(e, context = SupportContext.General, source = "generate_mnemonic")
            }
        }
    }

    fun validateMnemonic(phrase: String, onResult: (Boolean) -> Unit) {
        viewModelScope.launch(ioDispatcher) {
            val valid = try {
                repository.validateMnemonic(phrase)
                true
            } catch (_: OubliException) {
                false
            }
            launch(Dispatchers.Main) { onResult(valid) }
        }
    }

    fun completeOnboarding(mnemonic: String) {
        viewModelScope.launch(ioDispatcher) {
            try {
                repository.completeOnboarding(mnemonic)
                refreshState()
                loadActivity()
            } catch (e: Exception) {
                emitError(e, context = SupportContext.General, source = "complete_onboarding")
            }
        }
    }

    // ---- Unlock ----

    fun unlockBiometric() {
        if (!repository.isInitialized) {
            _uiState.update { current ->
                val screen = current.screenState
                if (screen is ScreenState.Locked) {
                    current.copy(screenState = screen.copy(unlockError = "Wallet is unavailable. Restart the app and try again."))
                } else {
                    current
                }
            }
            return
        }

        _uiState.update { current ->
            val screen = current.screenState
            if (screen is ScreenState.Locked) {
                current.copy(screenState = screen.copy(unlockError = null))
            } else {
                current
            }
        }

        viewModelScope.launch(ioDispatcher) {
            try {
                repository.unlockBiometric()
                refreshState()
                // Load cached activity immediately for instant display
                val cachedEvents = repository.getCachedActivity()
                _uiState.update { current ->
                    val screen = current.screenState
                    if (screen is ScreenState.Ready) {
                        current.copy(screenState = screen.copy(activity = cachedEvents))
                    } else {
                        current
                    }
                }
                // Then fetch fresh activity from network
                loadActivity()
            } catch (e: Exception) {
                _uiState.update { current ->
                    val screen = current.screenState
                    if (screen is ScreenState.Locked) {
                        current.copy(screenState = screen.copy(unlockError = biometricUnlockErrorMessage(e)))
                    } else {
                        current
                    }
                }
            }
        }
    }

    // ---- Balance Privacy ----

    fun toggleBalanceHidden() {
        _uiState.update { current ->
            val screen = current.screenState
            if (screen is ScreenState.Ready) {
                current.copy(screenState = screen.copy(isBalanceHidden = !screen.isBalanceHidden))
            } else {
                current
            }
        }
    }

    fun toggleCurrency() {
        _uiState.update { current ->
            val screen = current.screenState
            if (screen is ScreenState.Ready) {
                current.copy(screenState = screen.copy(showFiat = !screen.showFiat))
            } else {
                current
            }
        }
        val shouldRefreshBtcPrice =
            (_uiState.value.screenState as? ScreenState.Ready)?.let { screen ->
                screen.showFiat && screen.btcFiatPrice == null
            } == true
        if (shouldRefreshBtcPrice) {
            refreshBtcPrice()
        }
    }

    fun refreshBtcPrice() {
        val currency = getFiatCurrency()
        viewModelScope.launch(ioDispatcher) {
            val price = repository.getBtcPrice(currency)
            if (price != null) {
                _uiState.update { current ->
                    val screen = current.screenState
                    if (screen is ScreenState.Ready) {
                        current.copy(screenState = screen.copy(btcFiatPrice = price))
                    } else {
                        current
                    }
                }
            }
        }
    }

    fun setFiatCurrency(code: String) {
        val lower = code.lowercase()
        repository.saveFiatCurrency(lower)
        _uiState.update { current ->
            val screen = current.screenState
            if (screen is ScreenState.Ready) {
                current.copy(screenState = screen.copy(fiatCurrency = lower, btcFiatPrice = null))
            } else {
                current
            }
        }
        refreshBtcPrice()
    }

    private fun getFiatCurrency(): String {
        val screen = _uiState.value.screenState
        return (screen as? ScreenState.Ready)?.fiatCurrency ?: repository.getSavedFiatCurrency()
    }

    fun satsToFiat(sats: String): String? {
        val screen = _uiState.value.screenState as? ScreenState.Ready ?: return null
        val price = screen.btcFiatPrice ?: return null
        val satsVal = sats.toDoubleOrNull() ?: return null
        val fiat = satsVal * price / 100_000_000.0
        val symbol = fiatSymbol(screen.fiatCurrency)
        return if (fiat < 0.01) String.format("${symbol}%.4f", fiat)
        else String.format("${symbol}%.2f", fiat)
    }

    /** Raw numeric fiat value (no symbol) for a given sats amount. */
    fun satsToFiatRaw(sats: String): String? {
        val screen = _uiState.value.screenState as? ScreenState.Ready ?: return null
        val price = screen.btcFiatPrice ?: return null
        val satsVal = sats.toDoubleOrNull()?.takeIf { it > 0 } ?: return null
        val fiat = satsVal * price / 100_000_000.0
        return if (fiat < 0.01) String.format("%.4f", fiat)
        else String.format("%.2f", fiat)
    }

    /** Convert a fiat amount string to sats (rounded to nearest integer). */
    fun fiatToSats(fiat: String): String? {
        val screen = _uiState.value.screenState as? ScreenState.Ready ?: return null
        val price = screen.btcFiatPrice?.takeIf { it > 0 } ?: return null
        val fiatVal = fiat.toDoubleOrNull()?.takeIf { it > 0 } ?: return null
        val sats = fiatVal / price * 100_000_000.0
        return sats.toLong().toString()
    }

    companion object {
        val supportedFiatCurrencies = listOf(
            "usd" to "US Dollar", "eur" to "Euro", "gbp" to "British Pound",
            "jpy" to "Japanese Yen", "cad" to "Canadian Dollar", "aud" to "Australian Dollar",
            "chf" to "Swiss Franc", "cny" to "Chinese Yuan", "inr" to "Indian Rupee",
            "brl" to "Brazilian Real", "krw" to "Korean Won", "mxn" to "Mexican Peso",
            "try" to "Turkish Lira", "sek" to "Swedish Krona", "nok" to "Norwegian Krone",
            "dkk" to "Danish Krone", "pln" to "Polish Zloty", "zar" to "South African Rand",
            "thb" to "Thai Baht", "sgd" to "Singapore Dollar", "hkd" to "Hong Kong Dollar",
            "nzd" to "New Zealand Dollar",
        )

        fun fiatSymbol(code: String): String = when (code.lowercase()) {
            "usd", "cad", "aud", "nzd", "sgd", "hkd", "mxn" -> "$"
            "eur" -> "€"
            "gbp" -> "£"
            "jpy", "cny" -> "¥"
            "inr" -> "₹"
            "brl" -> "R$"
            "krw" -> "₩"
            "try" -> "₺"
            "sek", "nok", "dkk" -> "kr "
            "pln" -> "zł "
            "zar" -> "R "
            "thb" -> "฿"
            "chf" -> "CHF "
            else -> "${code.uppercase()} "
        }
    }

    fun calculateSendFee(amountSats: String, recipient: String): String {
        return repository.calculateSendFee(amountSats, recipient)
    }

    // ---- Operations ----

    fun send(amountSats: String, recipient: String) {
        viewModelScope.launch(ioDispatcher) {
            try {
                val txHash = repository.send(amountSats, recipient)
                refreshState()
                emitSuccess("Sent: ${txHash?.take(16)}...")
            } catch (e: Exception) {
                emitError(e, context = SupportContext.Send, source = "send")
            }
        }
    }

    fun payLightningWithCallback(bolt11: String, onResult: (Result<String?>) -> Unit) {
        viewModelScope.launch(ioDispatcher) {
            val pollJob = launch {
                while (true) {
                    delay(1000)
                    try {
                        _lightningOperation.value = repository.getState().operation
                    } catch (_: Exception) {}
                }
            }

            val result = runCatching<String?> { repository.payLightning(bolt11) }
            pollJob.cancel()
            _lightningOperation.value = null

            if (result.isSuccess) {
                refreshState()
                loadActivity()
            }

            launch(Dispatchers.Main) { onResult(result) }
        }
    }

    fun receiveLightningCreateInvoice(amountSats: ULong, onResult: (Result<uniffi.oubli.SwapQuoteFfi>) -> Unit) {
        viewModelScope.launch(ioDispatcher) {
            val result = runCatching { repository.swapLnToWbtc(amountSats, false) }
            launch(Dispatchers.Main) { onResult(result) }
        }
    }

    fun receiveLightningWait(swapId: String, onResult: (Result<Unit>) -> Unit) {
        viewModelScope.launch(ioDispatcher) {
            val result = runCatching { repository.receiveLightningWait(swapId) }
            if (result.isSuccess) {
                refreshState()
                loadActivity()
            }
            launch(Dispatchers.Main) { onResult(result) }
        }
    }

    fun refreshBalance() {
        viewModelScope.launch(ioDispatcher) {
            _uiState.update { current ->
                val screen = current.screenState
                if (screen is ScreenState.Ready) {
                    current.copy(screenState = screen.copy(isRefreshing = true))
                } else {
                    current
                }
            }
            try {
                repository.refreshBalance()
                refreshState()
                loadActivity()
            } catch (e: Exception) {
                emitError(e, context = SupportContext.General, source = "refresh_balance")
            }
            _uiState.update { current ->
                val screen = current.screenState
                if (screen is ScreenState.Ready) {
                    current.copy(screenState = screen.copy(isRefreshing = false))
                } else {
                    current
                }
            }
        }
    }

    private fun loadActivity() {
        viewModelScope.launch(ioDispatcher) {
            try {
                val events = repository.getActivity()
                val contacts = (_uiState.value.screenState as? ScreenState.Ready)?.contacts ?: emptyList()
                val nameMap = buildContactNameMap(events, contacts)
                _uiState.update { current ->
                    val screen = current.screenState
                    if (screen is ScreenState.Ready) {
                        current.copy(screenState = screen.copy(activity = events, activityContactNames = nameMap))
                    } else {
                        current
                    }
                }
            } catch (e: Exception) {
                emitError(e, context = SupportContext.General, source = "load_activity")
            }
        }
    }

    private fun buildContactNameMap(
        events: List<uniffi.oubli.ActivityEventFfi>,
        contacts: List<ContactFfi>,
    ): Map<String, String> {
        if (contacts.isEmpty()) return emptyMap()
        val map = mutableMapOf<String, String>()
        for (event in events) {
            val recipient = repository.getTransferRecipientSync(event.txHash) ?: continue
            val contact = contacts.firstOrNull { c ->
                c.addresses.any { it.address.equals(recipient, ignoreCase = true) }
            } ?: continue
            map[event.txHash] = contact.name
        }
        return map
    }

    // ---- Contacts ----

    fun loadContacts() {
        viewModelScope.launch(ioDispatcher) {
            try {
                val contacts = repository.getContacts()
                _uiState.update { current ->
                    val screen = current.screenState
                    if (screen is ScreenState.Ready) {
                        current.copy(screenState = screen.copy(contacts = contacts))
                    } else {
                        current
                    }
                }
            } catch (_: Exception) {}
        }
    }

    fun saveContact(contact: ContactFfi) {
        viewModelScope.launch(ioDispatcher) {
            try {
                repository.saveContact(contact)
                loadContacts()
            } catch (e: Exception) {
                emitError(e, context = SupportContext.General, source = "save_contact")
            }
        }
    }

    fun deleteContact(contactId: String) {
        viewModelScope.launch(ioDispatcher) {
            try {
                repository.deleteContact(contactId)
                loadContacts()
            } catch (e: Exception) {
                emitError(e, context = SupportContext.General, source = "delete_contact")
            }
        }
    }

    fun updateContactLastUsed(contactId: String) {
        viewModelScope.launch(ioDispatcher) {
            try {
                repository.updateContactLastUsed(contactId)
                loadContacts()
            } catch (_: Exception) {}
        }
    }

    // ---- Seed Backup ----

    fun startSeedBackup(mnemonic: String) {
        viewModelScope.launch(ioDispatcher) {
            try {
                val backupState = repository.startSeedBackup(mnemonic)
                if (backupState != null) {
                    _uiState.update { it.copy(screenState = ScreenState.SeedBackup(backupState)) }
                }
                refreshState()
            } catch (e: Exception) {
                emitError(e, context = SupportContext.General, source = "start_seed_backup")
            }
        }
    }

    fun verifySeedWord(promptIndex: UInt, answer: String, onResult: (Boolean) -> Unit) {
        viewModelScope.launch(ioDispatcher) {
            try {
                val correct = repository.verifySeedWord(promptIndex, answer) ?: false
                launch(Dispatchers.Main) { onResult(correct) }
            } catch (e: Exception) {
                emitError(e, context = SupportContext.SeedReveal, source = "verify_seed_word")
                launch(Dispatchers.Main) { onResult(false) }
            }
        }
    }

    // ---- Seed Phrase Retrieval ----

    fun getMnemonic(onResult: (Result<String>) -> Unit) {
        viewModelScope.launch(ioDispatcher) {
            val result = runCatching { repository.getMnemonic() }
            launch(Dispatchers.Main) { onResult(result) }
        }
    }

    // ---- Message handling ----

    fun onMessageShown(id: Long) {
        _uiState.update { current ->
            if (current.userMessage?.id == id) {
                current.copy(userMessage = null)
            } else {
                current
            }
        }
    }

    // ---- Helpers ----

    private fun emitError(
        throwable: Throwable,
        context: SupportContext = SupportContext.General,
        source: String? = null,
    ) {
        val issue = ErrorPresentation.fromThrowable(
            throwable = throwable,
            context = context,
            walletState = _uiState.value.screenState::class.simpleName,
            source = source,
        )
        _uiState.update {
            it.copy(
                userMessage = UserMessage(
                    text = issue.message,
                    isError = true,
                    diagnostics = issue.diagnostics,
                )
            )
        }
    }

    fun showMessage(message: String) {
        _uiState.update { it.copy(userMessage = UserMessage(text = message, isError = false)) }
    }

    private fun emitSuccess(message: String) {
        _uiState.update { it.copy(userMessage = UserMessage(text = message, isError = false)) }
    }

    private fun biometricUnlockErrorMessage(throwable: Throwable): String {
        val rawMessage = when (throwable) {
            is OubliException.Auth -> throwable.message
            is OubliException.InvalidState -> throwable.message
            else -> throwable.message
        }?.trim().orEmpty()

        val lowercased = rawMessage.lowercase()

        return when {
            rawMessage.isBlank() -> "Authentication failed. Try again."
            "biometric authentication failed" in lowercased || "authentication failed" in lowercased ->
                "Authentication failed. Try again."
            "cancel" in lowercased ->
                "Authentication was canceled. Try again."
            "locked out" in lowercased ->
                "Biometric authentication is temporarily locked. Wait a moment, then try again."
            "not available" in lowercased ->
                "Biometric authentication is unavailable right now. Try again."
            else -> rawMessage
        }
    }
}
