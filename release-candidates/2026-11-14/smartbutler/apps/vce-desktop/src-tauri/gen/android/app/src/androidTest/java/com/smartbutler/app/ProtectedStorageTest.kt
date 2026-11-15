package com.smartbutler.app

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ProtectedStorageTest {
    @Test fun roundTripUsesFreshNonceAndRejectsTampering() {
        val original = "Private profile: नमस्ते Butler".toByteArray(Charsets.UTF_8)
        val first = ProtectedStorage.encrypt(original)
        val second = ProtectedStorage.encrypt(original)
        assertFalse(first.contentEquals(second))
        assertArrayEquals(original, ProtectedStorage.decrypt(first))
        assertArrayEquals(original, ProtectedStorage.decrypt(second))
        first[first.lastIndex] = (first.last().toInt() xor 1).toByte()
        try {
            ProtectedStorage.decrypt(first)
            fail("Tampered ciphertext must never decrypt")
        } catch (_: javax.crypto.AEADBadTagException) { }
    }
    @Test fun emptyPersonalDataStillAuthenticates() {
        assertArrayEquals(byteArrayOf(), ProtectedStorage.decrypt(ProtectedStorage.encrypt(byteArrayOf())))
        try {
            ProtectedStorage.decrypt(byteArrayOf(1, 2))
            fail("Truncated ciphertext must be rejected")
        } catch (_: IllegalArgumentException) { }
    }
}
