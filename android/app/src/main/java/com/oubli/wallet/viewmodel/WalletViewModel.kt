package com.oubli.wallet.viewmodel

import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.SavedStateHandle
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
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.oubli.ActivityEventFfi
import uniffi.oubli.ContactFfi
import uniffi.oubli.OubliException
import uniffi.oubli.WalletStateFfi
import javax.inject.Inject
import javax.inject.Qualifier
import java.util.Locale

@Qualifier
@Retention(AnnotationRetention.BINARY)
annotation class IoDispatcher

@HiltViewModel
class WalletViewModel @Inject constructor(
    private val repository: WalletRepository,
    @IoDispatcher private val ioDispatcher: CoroutineDispatcher,
    private val savedStateHandle: SavedStateHandle,
) : ViewModel() {

    @VisibleForTesting
    internal var activityPollingEnabled: Boolean = true

    private val _uiState = MutableStateFlow(WalletUiState())
    val uiState: StateFlow<WalletUiState> = _uiState.asStateFlow()

    /** Dialog-scoped flow for lightning operation progress messages. */
    private val _lightningOperation = MutableStateFlow<String?>(null)
    val lightningOperation: StateFlow<String?> = _lightningOperation.asStateFlow()

    private val _lightningSendState = MutableStateFlow(loadLightningSendState())
    val lightningSendState: StateFlow<LightningSendUiState> = _lightningSendState.asStateFlow()

    private val _lightningReceiveState = MutableStateFlow(loadLightningReceiveState())
    val lightningReceiveState: StateFlow<LightningReceiveUiState> = _lightningReceiveState.asStateFlow()

    /** One-shot events emitted when the Rust poll detects a new incoming payment. */
    private val _incomingPaymentEvent = MutableSharedFlow<ActivityEventFfi>(extraBufferCapacity = 5)
    val incomingPaymentEvent: SharedFlow<ActivityEventFfi> = _incomingPaymentEvent.asSharedFlow()

    /** Tx hashes of recently received payments — used for transient UI highlights. */
    private val _highlightedTxHashes = MutableStateFlow<Set<String>>(emptySet())
    val highlightedTxHashes: StateFlow<Set<String>> = _highlightedTxHashes.asStateFlow()

    private var activityPollingJob: Job? = null
    private var paymentCallbackJob: Job? = null
    private var lightningReceiveWaitJob: Job? = null

    private fun loadLightningSendState(): LightningSendUiState {
        val restoredStatus = savedStateHandle.get<String>(KEY_LIGHTNING_SEND_STATUS)
            ?.let { runCatching { LightningSendStatus.valueOf(it) }.getOrNull() }
            ?: LightningSendStatus.Idle
        val restoredMessage = savedStateHandle.get<String>(KEY_LIGHTNING_SEND_MESSAGE)

        return when (restoredStatus) {
            LightningSendStatus.Processing -> LightningSendUiState(
                status = LightningSendStatus.Error,
                message = "Lightning payment was interrupted. Check activity before retrying.",
            )
            else -> LightningSendUiState(
                status = restoredStatus,
                message = restoredMessage,
            )
        }
    }

    private fun loadLightningReceiveState(): LightningReceiveUiState {
        val wasCreating = savedStateHandle.get<Boolean>(KEY_LIGHTNING_RECEIVE_CREATING) ?: false
        val wasWaiting = savedStateHandle.get<Boolean>(KEY_LIGHTNING_RECEIVE_WAITING) ?: false

        return LightningReceiveUiState(
            invoice = savedStateHandle.get<String>(KEY_LIGHTNING_RECEIVE_INVOICE),
            swapId = savedStateHandle.get<String>(KEY_LIGHTNING_RECEIVE_SWAP_ID),
            feeSats = savedStateHandle.get<String>(KEY_LIGHTNING_RECEIVE_FEE),
            expiryEpochSeconds = savedStateHandle.get<Long>(KEY_LIGHTNING_RECEIVE_EXPIRY),
            isCreating = false,
            isWaiting = false,
            isSuccess = savedStateHandle.get<Boolean>(KEY_LIGHTNING_RECEIVE_SUCCESS) ?: false,
            errorMessage = when {
                wasCreating -> "Invoice creation was interrupted. Create a new one."
                wasWaiting -> "Payment check was interrupted. Retry when you're ready."
                else -> savedStateHandle.get<String>(KEY_LIGHTNING_RECEIVE_ERROR)
            },
        )
    }

    private fun setLightningSendState(state: LightningSendUiState) {
        _lightningSendState.value = state
        savedStateHandle[KEY_LIGHTNING_SEND_STATUS] = state.status.name
        savedStateHandle[KEY_LIGHTNING_SEND_MESSAGE] = state.message
    }

    private fun setLightningReceiveState(state: LightningReceiveUiState) {
        _lightningReceiveState.value = state
        savedStateHandle[KEY_LIGHTNING_RECEIVE_INVOICE] = state.invoice
        savedStateHandle[KEY_LIGHTNING_RECEIVE_SWAP_ID] = state.swapId
        savedStateHandle[KEY_LIGHTNING_RECEIVE_FEE] = state.feeSats
        savedStateHandle[KEY_LIGHTNING_RECEIVE_EXPIRY] = state.expiryEpochSeconds
        savedStateHandle[KEY_LIGHTNING_RECEIVE_CREATING] = state.isCreating
        savedStateHandle[KEY_LIGHTNING_RECEIVE_WAITING] = state.isWaiting
        savedStateHandle[KEY_LIGHTNING_RECEIVE_SUCCESS] = state.isSuccess
        savedStateHandle[KEY_LIGHTNING_RECEIVE_ERROR] = state.errorMessage
    }

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
            paymentCallbackJob?.cancel()
            paymentCallbackJob = null
            return
        }
        val shouldPoll = walletState == WalletStateFfi.READY || walletState == WalletStateFfi.PROCESSING
        if (shouldPoll && activityPollingJob == null) {
            refreshBtcPrice()
            loadContacts()
            // Eagerly deploy account in background so Lightning receive
            // doesn't block on deployment when the user opens it.
            viewModelScope.launch(ioDispatcher) {
                try { repository.ensureDeployed() } catch (_: Exception) {}
            }
            activityPollingJob = viewModelScope.launch(ioDispatcher) {
                var knownTxHashes = emptySet<String>()
                var firstPoll = true
                while (true) {
                    delay(2000)
                    try {
                        val state = repository.getState()
                        val events = try {
                            repository.getActivity()
                        } catch (_: Exception) {
                            repository.getCachedActivity()
                        }

                        // Detect new incoming payments by diffing against previous poll
                        if (!firstPoll) {
                            val incomingTypes = setOf("TransferIn", "Fund")
                            events
                                .filter { it.txHash !in knownTxHashes && it.eventType in incomingTypes }
                                .forEach { event ->
                                    _incomingPaymentEvent.emit(event)
                                    _highlightedTxHashes.update { it + event.txHash }
                                    launch {
                                        delay(5000)
                                        _highlightedTxHashes.update { it - event.txHash }
                                    }
                                }
                        }
                        knownTxHashes = events.map { it.txHash }.toSet()
                        firstPoll = false

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
            paymentCallbackJob?.cancel()
            paymentCallbackJob = null
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
        return if (fiat < 0.01) String.format(Locale.ROOT, "${symbol}%.4f", fiat)
        else String.format(Locale.ROOT, "${symbol}%.2f", fiat)
    }

    /** Raw numeric fiat value (no symbol) for a given sats amount. */
    fun satsToFiatRaw(sats: String): String? {
        val screen = _uiState.value.screenState as? ScreenState.Ready ?: return null
        val price = screen.btcFiatPrice ?: return null
        val satsVal = sats.toDoubleOrNull()?.takeIf { it > 0 } ?: return null
        val fiat = satsVal * price / 100_000_000.0
        return if (fiat < 0.01) String.format(Locale.ROOT, "%.4f", fiat)
        else String.format(Locale.ROOT, "%.2f", fiat)
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
        private const val KEY_LIGHTNING_SEND_STATUS = "lightning_send_status"
        private const val KEY_LIGHTNING_SEND_MESSAGE = "lightning_send_message"
        private const val KEY_LIGHTNING_RECEIVE_INVOICE = "lightning_receive_invoice"
        private const val KEY_LIGHTNING_RECEIVE_SWAP_ID = "lightning_receive_swap_id"
        private const val KEY_LIGHTNING_RECEIVE_FEE = "lightning_receive_fee"
        private const val KEY_LIGHTNING_RECEIVE_EXPIRY = "lightning_receive_expiry"
        private const val KEY_LIGHTNING_RECEIVE_CREATING = "lightning_receive_creating"
        private const val KEY_LIGHTNING_RECEIVE_WAITING = "lightning_receive_waiting"
        private const val KEY_LIGHTNING_RECEIVE_SUCCESS = "lightning_receive_success"
        private const val KEY_LIGHTNING_RECEIVE_ERROR = "lightning_receive_error"

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

    fun startLightningPayment(bolt11: String) {
        if (_lightningSendState.value.status == LightningSendStatus.Processing) return

        setLightningSendState(
            LightningSendUiState(
                status = LightningSendStatus.Processing,
                message = null,
            )
        )

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
                setLightningSendState(
                    LightningSendUiState(
                        status = LightningSendStatus.Success,
                        message = lightningSuccessMessage(result.getOrNull()),
                    )
                )
            } else {
                setLightningSendState(
                    LightningSendUiState(
                        status = LightningSendStatus.Error,
                        message = result.exceptionOrNull()?.message ?: "Unknown error",
                    )
                )
            }
        }
    }

    fun clearLightningSendState() {
        setLightningSendState(LightningSendUiState())
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

    fun startLightningReceiveInvoice(amountSats: ULong) {
        setLightningReceiveState(
            LightningReceiveUiState(
                isCreating = true,
                creatingStep = "Deploying wallet…",
                errorMessage = null,
            )
        )

        viewModelScope.launch(ioDispatcher) {
            try {
                // Step 1: Deploy account if needed (no-op if eager prewarm finished)
                repository.ensureDeployed()

                // Step 2: Initialize swap engine (no-op if eager prewarm finished)
                setLightningReceiveState(
                    _lightningReceiveState.value.copy(creatingStep = "Connecting to Lightning…")
                )
                repository.ensureSwapEngine()

                // Step 3: Create invoice — this is the only LP call
                setLightningReceiveState(
                    _lightningReceiveState.value.copy(creatingStep = "Creating invoice…")
                )
                val quote = repository.createLnInvoice(amountSats, false)

                val now = System.currentTimeMillis() / 1000
                val expiryEpoch = quote.expiry.toLong()
                android.util.Log.d("OubliLN", "Invoice created: expiry=$expiryEpoch now=$now remaining=${expiryEpoch - now}s swapId=${quote.swapId}")

                setLightningReceiveState(
                    LightningReceiveUiState(
                        invoice = quote.lnInvoice,
                        swapId = quote.swapId,
                        feeSats = quote.fee,
                        expiryEpochSeconds = expiryEpoch,
                    )
                )
                waitForLightningReceive(quote.swapId)
            } catch (e: Exception) {
                setLightningReceiveState(
                    LightningReceiveUiState(
                        errorMessage = e.message ?: "Failed to create invoice",
                    )
                )
            }
        }
    }

    fun retryLightningReceiveWait() {
        val swapId = _lightningReceiveState.value.swapId ?: return
        waitForLightningReceive(swapId)
    }

    fun clearLightningReceiveState() {
        lightningReceiveWaitJob?.cancel()
        lightningReceiveWaitJob = null
        setLightningReceiveState(LightningReceiveUiState())
    }

    fun markLightningReceiveExpired() {
        val state = _lightningReceiveState.value
        if (state.isSuccess) return
        lightningReceiveWaitJob?.cancel()
        lightningReceiveWaitJob = null
        setLightningReceiveState(
            LightningReceiveUiState(
                errorMessage = "Invoice expired. Create a new one.",
            )
        )
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

    private fun waitForLightningReceive(swapId: String) {
        if (_lightningReceiveState.value.isWaiting) return

        lightningReceiveWaitJob?.cancel()
        lightningReceiveWaitJob = viewModelScope.launch(ioDispatcher) {
            setLightningReceiveState(
                _lightningReceiveState.value.copy(
                    isCreating = false,
                    isWaiting = true,
                    errorMessage = null,
                )
            )

            val result = runCatching { repository.receiveLightningWait(swapId) }
            val current = _lightningReceiveState.value

            if (result.isSuccess) {
                refreshState()
                loadActivity()
                setLightningReceiveState(
                    current.copy(
                        isCreating = false,
                        isWaiting = false,
                        isSuccess = true,
                        errorMessage = null,
                    )
                )
            } else {
                val errMsg = result.exceptionOrNull()?.message ?: ""
                val isExpiry = errMsg.contains("expired", ignoreCase = true)
                        || errMsg.contains("timeout", ignoreCase = true)
                // Suppress expiry errors — the UI countdown already handles this
                if (!isExpiry) {
                    setLightningReceiveState(
                        current.copy(
                            isCreating = false,
                            isWaiting = false,
                            errorMessage = errMsg.ifEmpty { "Failed to confirm Lightning payment" },
                        )
                    )
                }
            }
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

    private fun lightningSuccessMessage(txHash: String?): String {
        return if (txHash != null && txHash.length > 16) {
            "Tx: ${txHash.take(10)}...${txHash.takeLast(6)}"
        } else {
            txHash ?: "Payment complete"
        }
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
