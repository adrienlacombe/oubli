package com.oubli.wallet.platform

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_STRONG
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import uniffi.oubli.OubliException
import uniffi.oubli.PlatformStorageCallback
import java.lang.ref.WeakReference
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import kotlin.coroutines.resume
import kotlin.coroutines.suspendCoroutine

/**
 * Android implementation of [PlatformStorageCallback].
 *
 * Uses the Android Keystore system to store and retrieve encrypted data.
 * Biometric authentication uses [BiometricPrompt] with [BIOMETRIC_STRONG].
 * Hardware salt is generated via [SecureRandom].
 */
class KeystoreStorage(
    private val context: Context,
    private val activityRef: WeakReference<FragmentActivity>,
) : PlatformStorageCallback {

    companion object {
        private const val KEYSTORE_PROVIDER = "AndroidKeyStore"
        private const val MASTER_KEY_ALIAS = "oubli_master_kek"
        private const val AES_GCM_TRANSFORMATION = "AES/GCM/NoPadding"
        private const val GCM_TAG_LENGTH = 128
        private const val GCM_IV_LENGTH = 12
        private const val PREFS_NAME = "oubli_secure_store"
    }

    private val keyStore: KeyStore = KeyStore.getInstance(KEYSTORE_PROVIDER).apply { load(null) }

    // ---- PlatformStorageCallback implementation ----

    override fun secureStore(key: String, value: List<UByte>) {
        try {
            val secretKey = getOrCreateMasterKey()
            val cipher = Cipher.getInstance(AES_GCM_TRANSFORMATION)
            cipher.init(Cipher.ENCRYPT_MODE, secretKey)

            val plaintext = value.map { it.toByte() }.toByteArray()
            val ciphertext = cipher.doFinal(plaintext)
            val iv = cipher.iv

            // Store IV + ciphertext together. IV is always GCM_IV_LENGTH bytes.
            val combined = ByteArray(iv.size + ciphertext.size)
            System.arraycopy(iv, 0, combined, 0, iv.size)
            System.arraycopy(ciphertext, 0, combined, iv.size, ciphertext.size)

            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            prefs.edit()
                .putString(key, android.util.Base64.encodeToString(combined, android.util.Base64.NO_WRAP))
                .apply()
        } catch (e: Exception) {
            throw OubliException.Store(e.message ?: "secureStore failed")
        }
    }

    override fun secureLoad(key: String): List<UByte>? {
        try {
            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            val encoded = prefs.getString(key, null) ?: return null

            val combined = android.util.Base64.decode(encoded, android.util.Base64.NO_WRAP)
            if (combined.size < GCM_IV_LENGTH) {
                throw IllegalStateException("stored data too short")
            }

            val iv = combined.copyOfRange(0, GCM_IV_LENGTH)
            val ciphertext = combined.copyOfRange(GCM_IV_LENGTH, combined.size)

            val secretKey = getOrCreateMasterKey()
            val cipher = Cipher.getInstance(AES_GCM_TRANSFORMATION)
            cipher.init(Cipher.DECRYPT_MODE, secretKey, GCMParameterSpec(GCM_TAG_LENGTH, iv))

            val plaintext = cipher.doFinal(ciphertext)
            return plaintext.map { it.toUByte() }
        } catch (e: Exception) {
            throw OubliException.Store(e.message ?: "secureLoad failed")
        }
    }

    override fun secureDelete(key: String) {
        try {
            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            prefs.edit().remove(key).apply()
        } catch (e: Exception) {
            throw OubliException.Store(e.message ?: "secureDelete failed")
        }
    }

    override fun requestBiometric(reason: String): Boolean {
        val activity = activityRef.get()
            ?: throw OubliException.Auth("Activity not available for biometric prompt")

        // BiometricPrompt must run on the main thread. Since UniFFI calls this from a
        // background thread, we use a blocking coroutine bridge via CountDownLatch.
        val latch = java.util.concurrent.CountDownLatch(1)
        var authenticated = false
        var errorMsg: String? = null

        val executor = ContextCompat.getMainExecutor(activity)

        val callback = object : BiometricPrompt.AuthenticationCallback() {
            override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                authenticated = true
                latch.countDown()
            }

            override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                errorMsg = errString.toString()
                latch.countDown()
            }

            override fun onAuthenticationFailed() {
                // Individual attempt failed, prompt stays open. Do nothing here;
                // the system will call onAuthenticationError if max attempts exceeded.
            }
        }

        activity.runOnUiThread {
            val prompt = BiometricPrompt(activity, executor, callback)
            val promptInfo = BiometricPrompt.PromptInfo.Builder()
                .setTitle("Oubli")
                .setSubtitle(reason)
                .setNegativeButtonText("Cancel")
                .setAllowedAuthenticators(BIOMETRIC_STRONG)
                .build()
            prompt.authenticate(promptInfo)
        }

        latch.await()

        if (errorMsg != null) {
            throw OubliException.Auth(errorMsg!!)
        }
        return authenticated
    }

    override fun biometricAvailable(): Boolean {
        val biometricManager = BiometricManager.from(context)
        return biometricManager.canAuthenticate(BIOMETRIC_STRONG) ==
            BiometricManager.BIOMETRIC_SUCCESS
    }

    override fun generateHardwareSalt(): List<UByte> {
        try {
            val salt = ByteArray(32)
            SecureRandom().nextBytes(salt)
            return salt.map { it.toUByte() }
        } catch (e: Exception) {
            throw OubliException.Store(e.message ?: "generateHardwareSalt failed")
        }
    }

    // ---- Internal helpers ----

    private fun getOrCreateMasterKey(): SecretKey {
        val existing = keyStore.getEntry(MASTER_KEY_ALIAS, null)
        if (existing is KeyStore.SecretKeyEntry) {
            return existing.secretKey
        }

        val keyGen = KeyGenerator.getInstance(
            KeyProperties.KEY_ALGORITHM_AES,
            KEYSTORE_PROVIDER
        )
        val spec = KeyGenParameterSpec.Builder(
            MASTER_KEY_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .setIsStrongBoxBacked(isStrongBoxAvailable())
            .build()

        keyGen.init(spec)
        return keyGen.generateKey()
    }

    private fun isStrongBoxAvailable(): Boolean {
        return context.packageManager.hasSystemFeature(
            android.content.pm.PackageManager.FEATURE_STRONGBOX_KEYSTORE
        )
    }
}
