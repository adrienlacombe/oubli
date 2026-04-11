package com.oubli.wallet.data

import androidx.fragment.app.FragmentActivity
import kotlinx.coroutines.flow.SharedFlow
import uniffi.oubli.ActivityEventFfi
import uniffi.oubli.ContactFfi
import uniffi.oubli.SeedBackupStateFfi
import uniffi.oubli.SwapQuoteFfi
import uniffi.oubli.WalletStateInfo

/**
 * Repository interface defining all wallet operations as suspend functions.
 * Abstracts the Rust FFI layer (OubliWallet) behind a clean Kotlin-coroutine API.
 */
interface WalletRepository {

    /** One-time initialization requiring a FragmentActivity for biometric prompts. */
    suspend fun initialize(activity: FragmentActivity)

    /** Whether the wallet has been initialized. */
    val isInitialized: Boolean

    /** Reactive stream of newly detected incoming payments. */
    val incomingPayments: SharedFlow<ActivityEventFfi>

    // ---- State ----

    suspend fun getState(): WalletStateInfo

    // ---- Onboarding ----

    suspend fun generateMnemonic(): String

    suspend fun validateMnemonic(phrase: String)

    suspend fun completeOnboarding(mnemonic: String)

    // ---- Unlock ----

    suspend fun unlockBiometric()

    // ---- Balance & Activity ----

    suspend fun refreshBalance()

    suspend fun getActivity(): List<ActivityEventFfi>

    suspend fun getCachedActivity(): List<ActivityEventFfi>

    suspend fun getBtcPriceUsd(): Double?

    suspend fun getBtcPrice(currency: String): Double?

    fun getSavedFiatCurrency(): String

    fun saveFiatCurrency(code: String)

    fun calculateSendFee(amountSats: String, recipient: String): String

    // ---- Operations ----

    suspend fun send(amountSats: String, recipient: String): String?

    suspend fun payLightning(bolt11: String): String?

    suspend fun swapLnToWbtc(amountSats: ULong, testnet: Boolean): SwapQuoteFfi

    suspend fun receiveLightningWait(swapId: String)

    // ---- Seed Backup ----

    suspend fun startSeedBackup(mnemonic: String): SeedBackupStateFfi?

    suspend fun verifySeedWord(promptIndex: UInt, answer: String): Boolean?

    suspend fun getMnemonic(): String

    // ---- Contacts ----

    suspend fun getContacts(): List<ContactFfi>

    suspend fun saveContact(contact: ContactFfi): String

    suspend fun deleteContact(contactId: String)

    suspend fun findContactByAddress(address: String): ContactFfi?

    suspend fun updateContactLastUsed(contactId: String)

    suspend fun getTransferRecipient(txHash: String): String?

    /** Blocking variant for use in composable remember blocks. */
    fun getTransferRecipientSync(txHash: String): String?

}
