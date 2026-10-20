package com.cantos.theatre.adapters.identity

import com.cantos.theatre.domain.identity.LoginSecret
import com.cantos.theatre.ports.IdentityCrypto
import java.security.MessageDigest
import java.security.GeneralSecurityException
import java.security.ProviderException
import java.security.SecureRandom
import java.util.Base64

/** Real entropy/S256 for the explicit JVM integration host; no credential persistence. */
class DesktopIdentityCrypto : IdentityCrypto {
    override fun secret(): LoginSecret? = try {
        LoginSecret.parse(Base64.getUrlEncoder().withoutPadding()
            .encodeToString(ByteArray(32).also { SecureRandom().nextBytes(it) }))
    } catch (_: ProviderException) { null }
      catch (_: SecurityException) { null }
    override fun challenge(verifier: LoginSecret): String? = try {
        Base64.getUrlEncoder().withoutPadding()
            .encodeToString(MessageDigest.getInstance("SHA-256").digest(verifier.value.toByteArray(Charsets.US_ASCII)))
    } catch (_: GeneralSecurityException) { null }
      catch (_: ProviderException) { null }
      catch (_: SecurityException) { null }
}
