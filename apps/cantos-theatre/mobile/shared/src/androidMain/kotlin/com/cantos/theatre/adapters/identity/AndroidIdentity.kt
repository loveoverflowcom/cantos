package com.cantos.theatre.adapters.identity

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.AtomicFile
import android.util.Base64
import com.cantos.theatre.domain.identity.LoginSecret
import com.cantos.theatre.domain.identity.NativeCallback
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.CredentialRead
import com.cantos.theatre.ports.CredentialStore
import com.cantos.theatre.ports.IdentityCrypto
import com.cantos.theatre.ports.SystemBrowser
import java.io.File
import java.io.IOException
import java.security.GeneralSecurityException
import java.security.KeyStore
import java.security.MessageDigest
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** One process owner. Only ciphertext enters noBackupFilesDir; the AES key is non-exportable. */
class AndroidCredentialStore(context: Context) : CredentialStore {
    private val file = AtomicFile(File(context.noBackupFilesDir, "theatre-identity-v1"))
    private val alias = "com.cantos.theatre.identity.v1"
    private val aad = "cantos-theatre-native-v1".toByteArray(Charsets.UTF_8)
    private fun key(create: Boolean): SecretKey? {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey(alias, null) as? SecretKey)?.let { return it }
        if (!create) return null
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").apply {
            init(KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setRandomizedEncryptionRequired(true).build())
        }.generateKey()
    }
    @Synchronized override fun read(): CredentialRead {
        return try {
            val bytes = file.openRead().use { stream ->
                if (stream.channel.size() !in 30..32798) return CredentialRead.Failed(CredentialFailure.Corrupt)
                stream.readBytes()
            }
            if (bytes.size !in 30..32798 || bytes[0] != 1.toByte()) return CredentialRead.Failed(CredentialFailure.Corrupt)
            val key = key(false) ?: return CredentialRead.Failed(CredentialFailure.Corrupt)
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, bytes.copyOfRange(1, 13)))
            cipher.updateAAD(aad)
            val plain = cipher.doFinal(bytes.copyOfRange(13, bytes.size))
            val value = plain.toString(Charsets.UTF_8)
            if (!value.toByteArray(Charsets.UTF_8).contentEquals(plain)) CredentialRead.Failed(CredentialFailure.Corrupt)
            else CredentialRead.Found(value)
        } catch (_: java.io.FileNotFoundException) { CredentialRead.Missing }
          catch (_: GeneralSecurityException) { CredentialRead.Failed(CredentialFailure.Corrupt) }
          catch (_: IOException) { CredentialRead.Failed(CredentialFailure.Unavailable) }
          catch (_: SecurityException) { CredentialRead.Failed(CredentialFailure.Denied) }
    }
    @Synchronized override fun write(value: String): CredentialFailure? {
        val plain = value.toByteArray(Charsets.UTF_8)
        if (plain.size !in 1..32768) return CredentialFailure.Corrupt
        return try {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, key(true))
            cipher.updateAAD(aad)
            val encoded = byteArrayOf(1) + cipher.iv + cipher.doFinal(plain)
            val output = file.startWrite()
            try { output.write(encoded); file.finishWrite(output) }
            catch (failure: IOException) { file.failWrite(output); throw failure }
            null
        } catch (_: GeneralSecurityException) { CredentialFailure.Unavailable }
          catch (_: IOException) { CredentialFailure.Unavailable }
          catch (_: SecurityException) { CredentialFailure.Denied }
    }
    @Synchronized override fun clear(): CredentialFailure? = try {
        file.delete()
        if (file.baseFile.exists()) CredentialFailure.Unavailable else null
    } catch (_: SecurityException) { CredentialFailure.Denied }
}

class AndroidIdentityCrypto : IdentityCrypto {
    override fun secret(): LoginSecret? = LoginSecret.parse(Base64.encodeToString(ByteArray(32).also { SecureRandom().nextBytes(it) }, Base64.URL_SAFE or Base64.NO_PADDING or Base64.NO_WRAP))
    override fun challenge(verifier: LoginSecret): String = Base64.encodeToString(MessageDigest.getInstance("SHA-256").digest(verifier.value.toByteArray(Charsets.US_ASCII)), Base64.URL_SAFE or Base64.NO_PADDING or Base64.NO_WRAP)
}
class AndroidSystemBrowser(private val context: Context) : SystemBrowser {
    override fun open(url: String): Boolean {
        val uri = Uri.parse(url)
        if (uri.scheme != "https") return false
        return try { context.startActivity(Intent(Intent.ACTION_VIEW, uri).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)); true }
        catch (_: android.content.ActivityNotFoundException) { false }
        catch (_: SecurityException) { false }
    }
    // ACTION_VIEW has no close API. Cancelling the protocol owner fences its eventual callback.
    override fun cancel() = Unit
}
fun parseAndroidCallback(uri: Uri): NativeCallback? {
    if (uri.fragment != null || uri.toString().substringBefore('?') != "com.cantos.theatre:/oauth2/callback") return null
    if (uri.queryParameterNames.any { it !in setOf("state", "code", "error", "error_description") || uri.getQueryParameters(it).size != 1 }) return null
    return NativeCallback(uri.toString().substringBefore('?'), uri.getQueryParameter("state") ?: return null, uri.getQueryParameter("code") ?: "", uri.getQueryParameter("error") != null)
}
