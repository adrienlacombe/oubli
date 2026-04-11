package com.oubli.wallet.data

import android.content.Context
import androidx.fragment.app.FragmentActivity
import com.oubli.wallet.platform.KeystoreStorage
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.oubli.ActivityEventFfi
import uniffi.oubli.ContactFfi
import uniffi.oubli.OubliWallet
import uniffi.oubli.SeedBackupStateFfi
import uniffi.oubli.SwapQuoteFfi
import uniffi.oubli.WalletStateInfo
import java.lang.ref.WeakReference
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class WalletRepositoryImpl @Inject constructor(
    @ApplicationContext private val context: Context,
) : WalletRepository {

    companion object {
        private const val DEBUG_PREFS = "oubli_debug"
    }

    private var wallet: OubliWallet? = null

    override val isInitialized: Boolean
        get() = wallet != null

    override suspend fun initialize(activity: FragmentActivity) {
        if (wallet != null) return
        withContext(Dispatchers.IO) {
            val storage = KeystoreStorage(
                context = context,
                activityRef = WeakReference(activity),
            )
            val w = OubliWallet(storage, null, null)
            wallet = w
        }
    }

    override suspend fun getState(): WalletStateInfo = withContext(Dispatchers.IO) {
        wallet!!.getState()
    }

    // ---- Onboarding ----

    override suspend fun generateMnemonic(): String = withContext(Dispatchers.IO) {
        wallet!!.generateMnemonic()
    }

    override suspend fun validateMnemonic(phrase: String) = withContext(Dispatchers.IO) {
        wallet!!.validateMnemonic(phrase)
    }

    override suspend fun completeOnboarding(mnemonic: String) = withContext(Dispatchers.IO) {
        wallet!!.handleCompleteOnboarding(mnemonic)
    }

    // ---- Unlock ----

    override suspend fun unlockBiometric() = withContext(Dispatchers.IO) {
        wallet!!.handleUnlockBiometric()
    }

    // ---- Balance & Activity ----

    override suspend fun refreshBalance() = withContext(Dispatchers.IO) {
        wallet!!.handleRefreshBalance()
    }

    override suspend fun getActivity(): List<ActivityEventFfi> = withContext(Dispatchers.IO) {
        wallet!!.getActivity()
    }

    override suspend fun getCachedActivity(): List<ActivityEventFfi> = withContext(Dispatchers.IO) {
        wallet!!.getCachedActivity()
    }

    override suspend fun getBtcPriceUsd(): Double? = withContext(Dispatchers.IO) {
        wallet!!.getBtcPriceUsd()
    }

    override suspend fun getBtcPrice(currency: String): Double? = withContext(Dispatchers.IO) {
        wallet!!.getBtcPrice(currency)
    }

    override fun calculateSendFee(amountSats: String, recipient: String): String {
        return wallet?.calculateSendFee(amountSats, recipient) ?: "0"
    }

    // ---- Operations ----

    override suspend fun send(amountSats: String, recipient: String): String? = withContext(Dispatchers.IO) {
        wallet!!.handleSend(amountSats, recipient)
    }

    override suspend fun payLightning(bolt11: String): String? = withContext(Dispatchers.IO) {
        wallet!!.payLightning(bolt11)
    }

    override suspend fun swapLnToWbtc(amountSats: ULong, testnet: Boolean): SwapQuoteFfi = withContext(Dispatchers.IO) {
        wallet!!.swapLnToWbtc(amountSats, testnet)
    }

    override suspend fun receiveLightningWait(swapId: String) = withContext(Dispatchers.IO) {
        wallet!!.receiveLightningWait(swapId)
    }

    // ---- Seed Backup ----

    override suspend fun startSeedBackup(mnemonic: String): SeedBackupStateFfi? = withContext(Dispatchers.IO) {
        wallet!!.handleStartSeedBackup(mnemonic)
    }

    override suspend fun verifySeedWord(promptIndex: UInt, answer: String): Boolean? = withContext(Dispatchers.IO) {
        wallet!!.handleVerifySeedWord(promptIndex, answer)
    }

    override suspend fun getMnemonic(): String = withContext(Dispatchers.IO) {
        wallet!!.getMnemonic()
    }

    // ---- Contacts ----

    override suspend fun getContacts(): List<ContactFfi> = withContext(Dispatchers.IO) {
        wallet!!.getContacts()
    }

    override suspend fun saveContact(contact: ContactFfi): String = withContext(Dispatchers.IO) {
        wallet!!.saveContact(contact)
    }

    override suspend fun deleteContact(contactId: String) = withContext(Dispatchers.IO) {
        wallet!!.deleteContact(contactId)
    }

    override suspend fun findContactByAddress(address: String): ContactFfi? = withContext(Dispatchers.IO) {
        wallet!!.findContactByAddress(address)
    }

    override suspend fun updateContactLastUsed(contactId: String) = withContext(Dispatchers.IO) {
        wallet!!.updateContactLastUsed(contactId)
    }

    override suspend fun getTransferRecipient(txHash: String): String? = withContext(Dispatchers.IO) {
        wallet!!.getTransferRecipient(txHash)
    }

    override fun getTransferRecipientSync(txHash: String): String? {
        return try { wallet?.getTransferRecipient(txHash) } catch (_: Exception) { null }
    }

    override fun getSavedFiatCurrency(): String {
        val prefs = context.getSharedPreferences(DEBUG_PREFS, Context.MODE_PRIVATE)
        return prefs.getString("oubli_fiat_currency", "usd") ?: "usd"
    }

    override fun saveFiatCurrency(code: String) {
        val prefs = context.getSharedPreferences(DEBUG_PREFS, Context.MODE_PRIVATE)
        prefs.edit().putString("oubli_fiat_currency", code.lowercase()).apply()
    }

}
