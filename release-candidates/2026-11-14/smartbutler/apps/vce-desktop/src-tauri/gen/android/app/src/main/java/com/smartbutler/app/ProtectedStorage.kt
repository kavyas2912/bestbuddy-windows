package com.smartbutler.app

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import androidx.annotation.Keep
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** App-private authenticated encryption; key material never leaves Android Keystore. */
@Keep
object ProtectedStorage {
    private const val ALIAS = "smart.butler.personal.v1"
    private val aad = "SMARTBUTLER-KEYSTORE-1".toByteArray(Charsets.UTF_8)

    @Synchronized
    private fun key(create: Boolean): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        check(create) { "Personal-data key is unavailable" }
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").apply {
            init(KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256).setRandomizedEncryptionRequired(true).build())
        }.generateKey()
    }

    @JvmStatic
    @Synchronized
    fun eraseKey(): Int {
        KeyStore.getInstance("AndroidKeyStore").apply { load(null); deleteEntry(ALIAS) }
        return 1
    }

    @JvmStatic
    fun encrypt(plain: ByteArray): ByteArray {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key(true))
        cipher.updateAAD(aad)
        check(cipher.iv.size == 12)
        return cipher.iv + cipher.doFinal(plain)
    }

    @JvmStatic
    fun decrypt(protected: ByteArray): ByteArray {
        require(protected.size >= 28) { "Invalid protected data" }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key(false), GCMParameterSpec(128, protected.copyOfRange(0, 12)))
        cipher.updateAAD(aad)
        return cipher.doFinal(protected, 12, protected.size - 12)
    }
}
