package com.oubli.wallet.viewmodel

import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.SavedStateHandle
import app.cash.turbine.test
import com.oubli.wallet.data.WalletRepository
import kotlinx.coroutines.delay
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.oubli.ActivityEventFfi
import uniffi.oubli.ContactAddressFfi
import uniffi.oubli.ContactFfi
import uniffi.oubli.OubliException
import uniffi.oubli.AddressTypeFfi
import uniffi.oubli.SeedBackupStateFfi
import uniffi.oubli.SwapQuoteFfi
import uniffi.oubli.VerificationPromptFfi
import uniffi.oubli.WalletStateFfi
import uniffi.oubli.WalletStateInfo

@OptIn(ExperimentalCoroutinesApi::class)
class WalletViewModelTest {

    private val testDispatcher = UnconfinedTestDispatcher()
    private lateinit var fakeRepository: FakeWalletRepository
    private lateinit var viewModel: WalletViewModel

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
        fakeRepository = FakeWalletRepository()
        viewModel = WalletViewModel(fakeRepository, testDispatcher, SavedStateHandle())
        viewModel.activityPollingEnabled = false
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `initial state is Loading`() {
        val state = viewModel.uiState.value
        assertEquals(ScreenState.Loading, state.screenState)
        assertNull(state.userMessage)
    }

    @Test
    fun `toggleBalanceHidden toggles hidden state in Ready`() {
        setScreenState(
            ScreenState.Ready(
                address = "0xabc",
                publicKey = "0xdef",
                balanceSats = "123",
                pendingSats = "0",
            ),
        )

        val ready = viewModel.uiState.value.screenState as ScreenState.Ready
        assertEquals(false, ready.isBalanceHidden)

        viewModel.toggleBalanceHidden()
        val toggled = viewModel.uiState.value.screenState as ScreenState.Ready
        assertEquals(true, toggled.isBalanceHidden)

        viewModel.toggleBalanceHidden()
        val toggledBack = viewModel.uiState.value.screenState as ScreenState.Ready
        assertEquals(false, toggledBack.isBalanceHidden)
    }

    @Test
    fun `toggleCurrency toggles USD in Ready`() {
        setScreenState(
            ScreenState.Ready(
                address = "0xabc",
                publicKey = "0xdef",
                balanceSats = "123",
                pendingSats = "0",
            ),
        )

        val ready = viewModel.uiState.value.screenState as ScreenState.Ready
        assertEquals(false, ready.showFiat)
        viewModel.toggleCurrency()
        val toggled = viewModel.uiState.value.screenState as ScreenState.Ready
        assertEquals(true, toggled.showFiat)
    }

    @Test
    fun `toggleCurrency refreshes BTC price when showing fiat without cached price`() = runTest(testDispatcher) {
        fakeRepository.stateToReturn = readyState()
        fakeRepository.initialized = true
        fakeRepository.btcPricesByCurrency["usd"] = 65432.1

        viewModel.uiState.test {
            assertEquals(ScreenState.Loading, awaitItem().screenState)

            viewModel.refreshBalance()
            awaitUntil { it.screenState is ScreenState.Ready }

            viewModel.toggleCurrency()
            val ready = awaitUntil { (it.screenState as? ScreenState.Ready)?.btcFiatPrice == 65432.1 }
                .screenState as ScreenState.Ready

            assertEquals(true, ready.showFiat)
            assertEquals(65432.1, ready.btcFiatPrice)
            assertEquals(listOf("usd"), fakeRepository.btcPriceRequests)

            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `send success emits success message`() = runTest(testDispatcher) {
        fakeRepository.stateToReturn = readyState()
        fakeRepository.initialized = true
        fakeRepository.sendResult = "0xdeadbeef12345678"

        viewModel.uiState.test {
            assertEquals(ScreenState.Loading, awaitItem().screenState)

            viewModel.refreshBalance()
            awaitUntil { it.screenState is ScreenState.Ready }

            viewModel.send("100", "0xrecipient")
            val withMsg = awaitUntil { it.userMessage != null }

            assertNotNull(withMsg.userMessage)
            assertEquals(false, withMsg.userMessage!!.isError)
            assertTrue(withMsg.userMessage!!.text.contains("Sent"))

            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `send failure emits error message`() = runTest(testDispatcher) {
        fakeRepository.stateToReturn = readyState()
        fakeRepository.initialized = true
        fakeRepository.sendShouldThrow = true

        viewModel.uiState.test {
            assertEquals(ScreenState.Loading, awaitItem().screenState)

            viewModel.refreshBalance()
            awaitUntil { it.screenState is ScreenState.Ready }

            viewModel.send("100", "0xrecipient")
            val withMsg = awaitUntil { it.userMessage != null }

            assertNotNull(withMsg.userMessage)
            assertEquals(true, withMsg.userMessage!!.isError)
            assertTrue(withMsg.userMessage!!.text.contains("Send failed"))

            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `onMessageShown clears the message`() = runTest(testDispatcher) {
        fakeRepository.stateToReturn = readyState()
        fakeRepository.initialized = true
        fakeRepository.sendResult = "0xdeadbeef12345678"

        viewModel.uiState.test {
            assertEquals(ScreenState.Loading, awaitItem().screenState)

            viewModel.refreshBalance()
            awaitUntil { it.screenState is ScreenState.Ready }

            viewModel.send("100", "0xrecipient")
            val withMsg = awaitUntil { it.userMessage != null }
            assertNotNull(withMsg.userMessage)

            viewModel.onMessageShown(withMsg.userMessage!!.id)
            val cleared = awaitUntil { it.userMessage == null }
            assertNull(cleared.userMessage)

            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `onMessageShown ignores mismatched id`() = runTest(testDispatcher) {
        fakeRepository.stateToReturn = readyState()
        fakeRepository.initialized = true
        fakeRepository.sendResult = "0xdeadbeef12345678"

        viewModel.uiState.test {
            assertEquals(ScreenState.Loading, awaitItem().screenState)

            viewModel.refreshBalance()
            awaitUntil { it.screenState is ScreenState.Ready }

            viewModel.send("100", "0xrecipient")
            val withMsg = awaitUntil { it.userMessage != null }
            assertNotNull(withMsg.userMessage)

            // Use a wrong id
            viewModel.onMessageShown(withMsg.userMessage!!.id + 999)

            // Message should still be there — no new emission expected
            // Just verify current state
            assertEquals(withMsg.userMessage, viewModel.uiState.value.userMessage)

            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `unlockBiometric when wallet unavailable shows locked error`() = runTest(testDispatcher) {
        fakeRepository.stateToReturn = lockedState()
        fakeRepository.initialized = true

        viewModel.uiState.test {
            assertEquals(ScreenState.Loading, awaitItem().screenState)

            viewModel.refreshBalance()
            awaitUntil { it.screenState is ScreenState.Locked }

            fakeRepository.initialized = false
            viewModel.unlockBiometric()

            val locked = awaitUntil { (it.screenState as? ScreenState.Locked)?.unlockError != null }
            assertEquals(
                "Wallet is unavailable. Restart the app and try again.",
                (locked.screenState as ScreenState.Locked).unlockError
            )

            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `unlockBiometric maps auth failure to friendly message`() = runTest(testDispatcher) {
        fakeRepository.stateToReturn = lockedState()
        fakeRepository.initialized = true
        fakeRepository.unlockBiometricError = OubliException.Auth("Biometric authentication failed")

        viewModel.uiState.test {
            assertEquals(ScreenState.Loading, awaitItem().screenState)

            viewModel.refreshBalance()
            awaitUntil { it.screenState is ScreenState.Locked }

            viewModel.unlockBiometric()
            val locked = awaitUntil { (it.screenState as? ScreenState.Locked)?.unlockError != null }

            assertEquals("Authentication failed. Try again.", (locked.screenState as ScreenState.Locked).unlockError)

            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `loadContacts populates contacts in ready state`() = runTest(testDispatcher) {
        fakeRepository.stateToReturn = readyState()
        fakeRepository.initialized = true
        fakeRepository.contactsToReturn = listOf(sampleContact())

        viewModel.uiState.test {
            assertEquals(ScreenState.Loading, awaitItem().screenState)

            viewModel.refreshBalance()
            awaitUntil { it.screenState is ScreenState.Ready }

            viewModel.loadContacts()
            val ready = awaitUntil { (it.screenState as? ScreenState.Ready)?.contacts?.isNotEmpty() == true }
                .screenState as ScreenState.Ready

            assertEquals(1, ready.contacts.size)
            assertEquals("Alice", ready.contacts.single().name)

            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `setFiatCurrency lowercases selection and refreshes price`() = runTest(testDispatcher) {
        fakeRepository.stateToReturn = readyState()
        fakeRepository.initialized = true
        fakeRepository.btcPricesByCurrency["eur"] = 43210.5

        viewModel.uiState.test {
            assertEquals(ScreenState.Loading, awaitItem().screenState)

            viewModel.refreshBalance()
            awaitUntil { it.screenState is ScreenState.Ready }

            viewModel.setFiatCurrency("EUR")
            val ready = awaitUntil { (it.screenState as? ScreenState.Ready)?.btcFiatPrice == 43210.5 }
                .screenState as ScreenState.Ready

            assertEquals("eur", ready.fiatCurrency)
            assertEquals(43210.5, ready.btcFiatPrice)
            assertEquals("eur", fakeRepository.savedFiatCurrencyValue)
            assertEquals("eur", fakeRepository.btcPriceRequests.last())

            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `refreshBalance failure clears refreshing state and emits error`() = runTest(testDispatcher) {
        fakeRepository.stateToReturn = readyState()
        fakeRepository.initialized = true

        viewModel.uiState.test {
            assertEquals(ScreenState.Loading, awaitItem().screenState)

            viewModel.refreshBalance()
            awaitUntil { it.screenState is ScreenState.Ready }

            fakeRepository.refreshBalanceError = RuntimeException("Refresh failed")
            viewModel.refreshBalance()

            val finalState = awaitUntil {
                val ready = it.screenState as? ScreenState.Ready
                it.userMessage != null && ready?.isRefreshing == false
            }

            assertTrue(finalState.userMessage!!.isError)
            assertEquals("Something went wrong. Try again.", finalState.userMessage!!.text)
            assertNotNull(finalState.userMessage!!.diagnostics)
            assertFalse((finalState.screenState as ScreenState.Ready).isRefreshing)

            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `completeOnboarding refreshes state and loads activity`() = runTest(testDispatcher) {
        fakeRepository.stateToReturn = readyState()
        fakeRepository.activityToReturn = listOf(sampleActivity())

        viewModel.uiState.test {
            assertEquals(ScreenState.Loading, awaitItem().screenState)

            viewModel.completeOnboarding("test mnemonic")
            val ready = awaitUntil { (it.screenState as? ScreenState.Ready)?.activity?.isNotEmpty() == true }
                .screenState as ScreenState.Ready

            assertEquals("test mnemonic", fakeRepository.completedOnboardingMnemonic)
            assertEquals(1, ready.activity.size)
            assertEquals("0xtx", ready.activity.single().txHash)

            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `validateMnemonic returns false on oubli exception`() = runTest(testDispatcher) {
        fakeRepository.validateMnemonicError = OubliException.Backup("invalid mnemonic")

        var result: Boolean? = null
        viewModel.validateMnemonic("bad phrase") { result = it }

        assertFalse(awaitValue { result })
    }

    @Test
    fun `startSeedBackup keeps seed backup screen after refresh`() = runTest(testDispatcher) {
        fakeRepository.stateToReturn = seedBackupWalletState()
        fakeRepository.startSeedBackupResult = sampleSeedBackupState()

        viewModel.uiState.test {
            assertEquals(ScreenState.Loading, awaitItem().screenState)

            viewModel.startSeedBackup("test mnemonic")
            val screen = awaitUntil { it.screenState is ScreenState.SeedBackup }.screenState as ScreenState.SeedBackup

            assertEquals(2, screen.backupState.prompts.size)

            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `verifySeedWord failure returns false and emits error`() = runTest(testDispatcher) {
        fakeRepository.verifySeedWordError = RuntimeException("Verification failed")

        var result: Boolean? = null
        viewModel.verifySeedWord(1u, "wrong") { result = it }

        assertFalse(awaitValue { result })
        assertTrue(viewModel.uiState.value.userMessage!!.isError)
        assertEquals(
            "Oubli could not reveal the seed phrase right now. Try again.",
            viewModel.uiState.value.userMessage!!.text,
        )
        assertNotNull(viewModel.uiState.value.userMessage!!.diagnostics)
    }

    @Test
    fun `getMnemonic failure returns result failure`() = runTest(testDispatcher) {
        fakeRepository.getMnemonicError = RuntimeException("No mnemonic available")

        var result: Result<String>? = null
        viewModel.getMnemonic { result = it }

        val callbackResult = awaitValue { result }
        assertTrue(callbackResult.isFailure)
        assertEquals("No mnemonic available", callbackResult.exceptionOrNull()!!.message)
    }

    @Test
    fun `payLightning success refreshes activity and clears progress`() = runTest(testDispatcher) {
        fakeRepository.stateToReturn = readyState()
        fakeRepository.payLightningResult = "0xlightning"
        fakeRepository.activityToReturn = listOf(sampleActivity(txHash = "0xln"))

        var result: Result<String?>? = null

        viewModel.uiState.test {
            assertEquals(ScreenState.Loading, awaitItem().screenState)

            viewModel.payLightningWithCallback("lnbc1example") { result = it }
            val ready = awaitUntil { (it.screenState as? ScreenState.Ready)?.activity?.singleOrNull()?.txHash == "0xln" }
                .screenState as ScreenState.Ready

            assertEquals("lnbc1example", fakeRepository.lastPaidBolt11)
            assertEquals("0xlightning", awaitValue { result }.getOrNull())
            assertEquals("0xln", ready.activity.single().txHash)
            assertNull(viewModel.lightningOperation.value)

            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `receiveLightningWait success refreshes activity and returns success`() = runTest(testDispatcher) {
        fakeRepository.stateToReturn = readyState()
        fakeRepository.activityToReturn = listOf(sampleActivity(txHash = "0xswap"))

        var result: Result<Unit>? = null

        viewModel.uiState.test {
            assertEquals(ScreenState.Loading, awaitItem().screenState)

            viewModel.receiveLightningWait("swap-123") { result = it }
            val ready = awaitUntil { (it.screenState as? ScreenState.Ready)?.activity?.singleOrNull()?.txHash == "0xswap" }
                .screenState as ScreenState.Ready

            assertEquals("swap-123", fakeRepository.lastReceiveLightningWaitSwapId)
            assertTrue(awaitValue { result }.isSuccess)
            assertEquals("0xswap", ready.activity.single().txHash)

            cancelAndIgnoreRemainingEvents()
        }
    }

    // ---- Helpers ----

    private fun readyState() = WalletStateInfo(
        state = WalletStateFfi.READY,
        address = "0x123",
        publicKey = "0xabc",
        balanceSats = "1000",
        pendingSats = "0",
        operation = null,
        errorMessage = null,
        autoFundError = null,
    )

    private fun lockedState() = WalletStateInfo(
        state = WalletStateFfi.LOCKED,
        address = null,
        publicKey = null,
        balanceSats = null,
        pendingSats = null,
        operation = null,
        errorMessage = null,
        autoFundError = null,
    )

    private fun seedBackupWalletState() = WalletStateInfo(
        state = WalletStateFfi.SEED_BACKUP,
        address = null,
        publicKey = null,
        balanceSats = null,
        pendingSats = null,
        operation = null,
        errorMessage = null,
        autoFundError = null,
    )

    private fun sampleActivity(txHash: String = "0xtx") = ActivityEventFfi(
        eventType = "transfer_out",
        amountSats = "100",
        txHash = txHash,
        blockNumber = 123uL,
        timestampSecs = 1_700_000_000uL,
        status = "confirmed",
        explorerUrl = "https://explorer.example/$txHash",
    )

    private fun sampleContact() = ContactFfi(
        id = "contact-1",
        name = "Alice",
        addresses = listOf(ContactAddressFfi("0xrecipient", AddressTypeFfi.OUBLI, null)),
        notes = null,
        createdAt = 1uL,
        lastUsedAt = 1uL,
    )

    private fun sampleSeedBackupState() = SeedBackupStateFfi(
        wordGroups = listOf(listOf("abandon", "ability", "able", "about")),
        prompts = listOf(VerificationPromptFfi(1u), VerificationPromptFfi(7u)),
    )

    /**
     * Consume items from the Turbine channel until [predicate] is satisfied,
     * returning the matching item. Throws on timeout.
     */
    private suspend fun <T> app.cash.turbine.ReceiveTurbine<T>.awaitUntil(predicate: (T) -> Boolean): T {
        while (true) {
            val item = awaitItem()
            if (predicate(item)) return item
        }
    }

    private suspend fun <T : Any> awaitValue(provider: () -> T?): T {
        var resolved: T? = null
        withTimeout(5_000) {
            while (resolved == null) {
                resolved = provider()
                if (resolved == null) {
                    delay(1)
                }
            }
        }
        return resolved!!
    }

    @Suppress("UNCHECKED_CAST")
    private fun setScreenState(screenState: ScreenState) {
        val field = WalletViewModel::class.java.getDeclaredField("_uiState")
        field.isAccessible = true
        val stateFlow = field.get(viewModel) as kotlinx.coroutines.flow.MutableStateFlow<WalletUiState>
        stateFlow.value = WalletUiState(screenState = screenState)
    }
}

/**
 * Fake implementation of [WalletRepository] for unit testing.
 */
class FakeWalletRepository : WalletRepository {

    var initialized = false
    var refreshBalanceError: Throwable? = null
    var unlockBiometricError: Throwable? = null
    var validateMnemonicError: OubliException? = null
    var completedOnboardingMnemonic: String? = null
    var stateToReturn = WalletStateInfo(
        state = WalletStateFfi.ONBOARDING,
        address = null,
        publicKey = null,
        balanceSats = null,
        pendingSats = null,
        operation = null,
        errorMessage = null,
        autoFundError = null,
    )
    var sendResult: String? = null
    var sendShouldThrow = false
    var activityToReturn: List<ActivityEventFfi> = emptyList()
    var cachedActivityToReturn: List<ActivityEventFfi> = emptyList()
    var contactsToReturn: List<ContactFfi> = emptyList()
    var btcPricesByCurrency: MutableMap<String, Double?> = mutableMapOf()
    var btcPriceRequests: MutableList<String> = mutableListOf()
    var savedFiatCurrencyValue: String = "usd"
    var payLightningResult: String? = null
    var payLightningError: Throwable? = null
    var lastPaidBolt11: String? = null
    var receiveLightningWaitError: Throwable? = null
    var lastReceiveLightningWaitSwapId: String? = null
    var startSeedBackupResult: SeedBackupStateFfi? = null
    var startSeedBackupError: Throwable? = null
    var verifySeedWordResult: Boolean? = true
    var verifySeedWordError: Throwable? = null
    var getMnemonicResult: String = "word ".repeat(12).trim()
    var getMnemonicError: Throwable? = null
    private val _incomingPayments = kotlinx.coroutines.flow.MutableSharedFlow<ActivityEventFfi>(extraBufferCapacity = 10)
    override val incomingPayments: kotlinx.coroutines.flow.SharedFlow<ActivityEventFfi> = _incomingPayments

    override val isInitialized: Boolean get() = initialized

    override suspend fun initialize(activity: FragmentActivity) {
        initialized = true
    }

    override suspend fun getState(): WalletStateInfo = stateToReturn

    override suspend fun generateMnemonic(): String = "word ".repeat(12).trim()

    override suspend fun validateMnemonic(phrase: String) {
        validateMnemonicError?.let { throw it }
    }

    override suspend fun completeOnboarding(mnemonic: String) {
        completedOnboardingMnemonic = mnemonic
    }

    override suspend fun unlockBiometric() {
        unlockBiometricError?.let { throw it }
    }

    override suspend fun refreshBalance() {
        refreshBalanceError?.let { throw it }
    }

    override suspend fun getActivity(): List<ActivityEventFfi> = activityToReturn

    override suspend fun getCachedActivity(): List<ActivityEventFfi> =
        if (cachedActivityToReturn.isNotEmpty()) cachedActivityToReturn else activityToReturn

    override suspend fun getBtcPriceUsd(): Double? = btcPricesByCurrency["usd"]

    override fun calculateSendFee(amountSats: String, recipient: String): String = "0"

    override suspend fun send(amountSats: String, recipient: String): String? {
        if (sendShouldThrow) throw RuntimeException("Send failed")
        return sendResult
    }

    override suspend fun payLightning(bolt11: String): String? {
        lastPaidBolt11 = bolt11
        payLightningError?.let { throw it }
        return payLightningResult
    }

    override suspend fun ensureDeployed() {}

    override suspend fun ensureSwapEngine() {}

    override suspend fun createLnInvoice(amountSats: ULong, exactIn: Boolean): SwapQuoteFfi {
        throw UnsupportedOperationException("Not implemented in fake")
    }

    override suspend fun swapLnToWbtc(amountSats: ULong, testnet: Boolean): SwapQuoteFfi {
        throw UnsupportedOperationException("Not implemented in fake")
    }

    override suspend fun receiveLightningWait(swapId: String) {
        lastReceiveLightningWaitSwapId = swapId
        receiveLightningWaitError?.let { throw it }
    }

    override suspend fun startSeedBackup(mnemonic: String): SeedBackupStateFfi? {
        startSeedBackupError?.let { throw it }
        return startSeedBackupResult
    }

    override suspend fun verifySeedWord(promptIndex: UInt, answer: String): Boolean? {
        verifySeedWordError?.let { throw it }
        return verifySeedWordResult
    }

    override suspend fun getMnemonic(): String {
        getMnemonicError?.let { throw it }
        return getMnemonicResult
    }

    override suspend fun getContacts(): List<ContactFfi> = contactsToReturn

    override suspend fun saveContact(contact: ContactFfi): String = ""

    override suspend fun deleteContact(contactId: String) {}

    override suspend fun findContactByAddress(address: String): ContactFfi? = null

    override suspend fun updateContactLastUsed(contactId: String) {}

    override suspend fun getTransferRecipient(txHash: String): String? = null

    override fun getTransferRecipientSync(txHash: String): String? = null

    override suspend fun getBtcPrice(currency: String): Double? {
        val normalized = currency.lowercase()
        btcPriceRequests += normalized
        return btcPricesByCurrency[normalized]
    }

    override fun getSavedFiatCurrency(): String = savedFiatCurrencyValue

    override fun saveFiatCurrency(code: String) {
        savedFiatCurrencyValue = code.lowercase()
    }

}
