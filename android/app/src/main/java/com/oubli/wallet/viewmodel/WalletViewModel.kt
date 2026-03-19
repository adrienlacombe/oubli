package com.oubli.wallet.viewmodel

import android.app.Application
import android.content.Context
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.oubli.wallet.platform.KeystoreStorage
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uniffi.oubli.ActivityEventFfi
import uniffi.oubli.OubliException
import uniffi.oubli.OubliWallet
import uniffi.oubli.SeedBackupStateFfi
import uniffi.oubli.WalletStateFfi
import uniffi.oubli.WalletStateInfo
import java.lang.ref.WeakReference

class WalletViewModel(application: Application) : AndroidViewModel(application) {

    private val _uiState = MutableStateFlow(
        WalletStateInfo(
            state = WalletStateFfi.ONBOARDING,
            address = null,
            publicKey = null,
            balanceSats = null,
            pendingSats = null,
            operation = null,
            errorMessage = null,
            autoFundError = null,
        )
    )
    val uiState: StateFlow<WalletStateInfo> = _uiState.asStateFlow()

    private val _isBalanceHidden = MutableStateFlow(false)
    val isBalanceHidden: StateFlow<Boolean> = _isBalanceHidden.asStateFlow()

    private val _showUsd = MutableStateFlow(false)
    val showUsd: StateFlow<Boolean> = _showUsd.asStateFlow()

    private val _btcPriceUsd = MutableStateFlow<Double?>(null)
    val btcPriceUsd: StateFlow<Double?> = _btcPriceUsd.asStateFlow()

    private val _isRefreshing = MutableStateFlow(false)
    val isRefreshing: StateFlow<Boolean> = _isRefreshing.asStateFlow()

    private val _seedBackupState = MutableStateFlow<SeedBackupStateFfi?>(null)
    val seedBackupState: StateFlow<SeedBackupStateFfi?> = _seedBackupState.asStateFlow()

    private val _activity = MutableStateFlow<List<ActivityEventFfi>>(emptyList())
    val activity: StateFlow<List<ActivityEventFfi>> = _activity.asStateFlow()

    private val _errorEvents = MutableSharedFlow<String>(extraBufferCapacity = 1)
    val errorEvents: SharedFlow<String> = _errorEvents.asSharedFlow()

    private val _successEvents = MutableSharedFlow<String>(extraBufferCapacity = 1)
    val successEvents: SharedFlow<String> = _successEvents.asSharedFlow()

    private val _lightningOperation = MutableStateFlow<String?>(null)
    val lightningOperation: StateFlow<String?> = _lightningOperation.asStateFlow()

    private val _biometricUnlockError = MutableStateFlow<String?>(null)
    val biometricUnlockError: StateFlow<String?> = _biometricUnlockError.asStateFlow()

    private var wallet: OubliWallet? = null
    private var activityPollingJob: kotlinx.coroutines.Job? = null

    fun attach(activity: FragmentActivity) {
        if (wallet != null) return
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val storage = KeystoreStorage(
                    context = getApplication(),
                    activityRef = WeakReference(activity),
                )
                wallet = OubliWallet(storage)
                val savedUrl = getApplication<Application>()
                    .getSharedPreferences("oubli_debug", Context.MODE_PRIVATE)
                    .getString("rpc_url", null)
                if (savedUrl != null) wallet?.updateRpcUrl(savedUrl)
                refreshState()
            } catch (e: OubliException) {
                _errorEvents.tryEmit("Failed to initialize wallet: ${e.message}")
            }
        }
    }

    private fun refreshState() {
        val w = wallet ?: return
        val state = w.getState()
        _uiState.value = state
        updateActivityPolling()
    }

    private fun updateActivityPolling() {
        val shouldPoll = _uiState.value.state == WalletStateFfi.READY ||
                _uiState.value.state == WalletStateFfi.PROCESSING
        if (shouldPoll && activityPollingJob == null) {
            refreshBtcPrice()
            activityPollingJob = viewModelScope.launch(Dispatchers.IO) {
                while (true) {
                    delay(2000)
                    runCatching { wallet?.getActivity() }
                        .onSuccess { events ->
                            if (events != null) _activity.value = events
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
        viewModelScope.launch(Dispatchers.IO) {
            runCatching { wallet?.generateMnemonic() }
                .onSuccess { mnemonic ->
                    if (mnemonic != null) {
                        launch(Dispatchers.Main) { onResult(mnemonic) }
                    }
                }
                .onFailure { emitError(it) }
        }
    }

    fun validateMnemonic(phrase: String, onResult: (Boolean) -> Unit) {
        viewModelScope.launch(Dispatchers.IO) {
            val valid = try {
                wallet?.validateMnemonic(phrase)
                true
            } catch (_: OubliException) {
                false
            }
            launch(Dispatchers.Main) { onResult(valid) }
        }
    }

    fun completeOnboarding(mnemonic: String) {
        viewModelScope.launch(Dispatchers.IO) {
            runCatching { wallet?.handleCompleteOnboarding(mnemonic) }
                .onSuccess {
                    refreshState()
                    loadActivity()
                }
                .onFailure { emitError(it) }
        }
    }

    // ---- Unlock ----

    fun unlockBiometric() {
        val currentWallet = wallet
        if (currentWallet == null) {
            _biometricUnlockError.value = "Wallet is unavailable. Restart the app and try again."
            return
        }
        _biometricUnlockError.value = null

        viewModelScope.launch(Dispatchers.IO) {
            runCatching { currentWallet.handleUnlockBiometric() }
                .onSuccess {
                    _biometricUnlockError.value = null
                    refreshState()
                    // Load cached activity immediately for instant display
                    currentWallet.getCachedActivity().let { _activity.value = it }
                    // Then fetch fresh activity from network
                    loadActivity()
                }
                .onFailure { _biometricUnlockError.value = biometricUnlockErrorMessage(it) }
        }
    }

    // ---- Balance Privacy ----

    fun toggleBalanceHidden() {
        _isBalanceHidden.value = !_isBalanceHidden.value
    }

    fun toggleCurrency() {
        _showUsd.value = !_showUsd.value
        if (_showUsd.value && _btcPriceUsd.value == null) {
            refreshBtcPrice()
        }
    }

    fun refreshBtcPrice() {
        viewModelScope.launch(Dispatchers.IO) {
            val price = wallet?.getBtcPriceUsd()
            if (price != null) _btcPriceUsd.value = price
        }
    }

    fun satsToUsd(sats: String): String? {
        val price = _btcPriceUsd.value ?: return null
        val satsVal = sats.toDoubleOrNull() ?: return null
        val usd = satsVal * price / 100_000_000.0
        return if (usd < 0.01) String.format("$%.4f", usd)
        else String.format("$%.2f", usd)
    }

    // ---- Operations ----

    fun send(amountSats: String, recipient: String) {
        viewModelScope.launch(Dispatchers.IO) {
            runCatching<String?> { wallet?.handleSend(amountSats, recipient) }
                .onSuccess { txHash ->
                    refreshState()
                    _successEvents.tryEmit("Sent: ${txHash?.take(16)}...")
                }
                .onFailure { emitError(it) }
        }
    }

    fun payLightning(bolt11: String) {
        viewModelScope.launch(Dispatchers.IO) {
            runCatching<String?> { wallet?.payLightning(bolt11) }
                .onSuccess { result ->
                    refreshState()
                    loadActivity()
                    _successEvents.tryEmit("Lightning payment sent")
                }
                .onFailure { emitError(it) }
        }
    }

    fun payLightningWithCallback(bolt11: String, onResult: (Result<String?>) -> Unit) {
        viewModelScope.launch(Dispatchers.IO) {
            val pollJob = launch {
                while (true) {
                    delay(1000)
                    _lightningOperation.value = wallet?.getState()?.operation
                }
            }

            val result = runCatching<String?> { wallet?.payLightning(bolt11) }
            pollJob.cancel()
            _lightningOperation.value = null

            result.onSuccess {
                refreshState()
                loadActivity()
            }

            launch(Dispatchers.Main) { onResult(result) }
        }
    }

    fun receiveLightningCreateInvoice(amountSats: ULong, onResult: (Result<uniffi.oubli.SwapQuoteFfi>) -> Unit) {
        viewModelScope.launch(Dispatchers.IO) {
            val result = runCatching { wallet!!.swapLnToWbtc(amountSats, false) }
            launch(Dispatchers.Main) { onResult(result) }
        }
    }

    fun receiveLightningWait(swapId: String, onResult: (Result<Unit>) -> Unit) {
        viewModelScope.launch(Dispatchers.IO) {
            val result = runCatching { wallet!!.receiveLightningWait(swapId) }
            result.onSuccess {
                refreshState()
                loadActivity()
            }
            launch(Dispatchers.Main) { onResult(result) }
        }
    }

    fun refreshBalance() {
        viewModelScope.launch(Dispatchers.IO) {
            _isRefreshing.value = true
            runCatching { wallet?.handleRefreshBalance() }
                .onSuccess {
                    refreshState()
                    loadActivity()
                }
                .onFailure { emitError(it) }
            _isRefreshing.value = false
        }
    }

    fun loadActivity() {
        viewModelScope.launch(Dispatchers.IO) {
            runCatching { wallet?.getActivity() }
                .onSuccess { events ->
                    if (events != null) _activity.value = events
                }
                .onFailure { emitError(it) }
        }
    }

    // ---- Seed Backup ----

    fun startSeedBackup(mnemonic: String) {
        viewModelScope.launch(Dispatchers.IO) {
            runCatching { wallet?.handleStartSeedBackup(mnemonic) }
                .onSuccess { backupState ->
                    _seedBackupState.value = backupState
                    refreshState()
                }
                .onFailure { emitError(it) }
        }
    }

    fun verifySeedWord(promptIndex: UInt, answer: String, onResult: (Boolean) -> Unit) {
        viewModelScope.launch(Dispatchers.IO) {
            runCatching { wallet?.handleVerifySeedWord(promptIndex, answer) }
                .onSuccess { correct ->
                    launch(Dispatchers.Main) { onResult(correct ?: false) }
                }
                .onFailure {
                    emitError(it)
                    launch(Dispatchers.Main) { onResult(false) }
                }
        }
    }

    // ---- Seed Phrase Retrieval ----

    fun getMnemonic(onResult: (Result<String>) -> Unit) {
        viewModelScope.launch(Dispatchers.IO) {
            val result = runCatching { wallet?.getMnemonic() ?: throw IllegalStateException("Wallet not initialized") }
            launch(Dispatchers.Main) { onResult(result) }
        }
    }

    // ---- Debug Settings ----

    fun updateRpcUrl(url: String) {
        viewModelScope.launch(Dispatchers.IO) {
            runCatching { wallet?.updateRpcUrl(url) }
                .onSuccess {
                    getApplication<Application>()
                        .getSharedPreferences("oubli_debug", Context.MODE_PRIVATE)
                        .edit().putString("rpc_url", url).apply()
                    _successEvents.tryEmit("RPC endpoint updated")
                }
                .onFailure { emitError(it) }
        }
    }

    fun getRpcUrl(): String {
        return wallet?.getRpcUrl() ?: ""
    }

    // ---- Helpers ----

    private fun emitError(throwable: Throwable) {
        val message = when (throwable) {
            is OubliException -> throwable.message ?: "Unknown error"
            else -> throwable.message ?: "Unexpected error"
        }
        _errorEvents.tryEmit(message)
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
                "Biometric authentication is temporarily locked. Use your device credential, then try again."
            "not available" in lowercased ->
                "Biometric authentication is unavailable right now. Try again."
            else -> rawMessage
        }
    }
}
