package com.cantos.theatre.domain.identity

enum class NativeConfigurationField { ApiOrigin, Issuer }
enum class NativeConfigurationFields { ApiOrigin, Issuer, Both }

/** Configuration diagnostics contain key names only, never untrusted build-input values. */
sealed interface NativeIdentityAvailability {
    data class Available(val configuration: NativeClientConfiguration) : NativeIdentityAvailability
    data class Missing(val fields: NativeConfigurationFields) : NativeIdentityAvailability
    data class Invalid(val field: NativeConfigurationField) : NativeIdentityAvailability
}

class NativeClientConfiguration private constructor(val apiOrigin: String, val identity: NativeConfiguration) {
    companion object {
        /** Empty defaults opt out. Supplied invalid values cannot silently become guest absence. */
        fun resolve(apiOrigin: String?, issuer: String?): NativeIdentityAvailability {
            if (apiOrigin.isNullOrEmpty()) {
                val fields = if (issuer.isNullOrEmpty()) NativeConfigurationFields.Both else NativeConfigurationFields.ApiOrigin
                return NativeIdentityAvailability.Missing(fields)
            }
            if (issuer.isNullOrEmpty()) {
                return NativeIdentityAvailability.Missing(NativeConfigurationFields.Issuer)
            }
            if (!validApiOrigin(apiOrigin)) {
                return NativeIdentityAvailability.Invalid(NativeConfigurationField.ApiOrigin)
            }
            val identity = NativeConfiguration.create(issuer, "com.cantos.theatre:/oauth2/callback")
                ?: return NativeIdentityAvailability.Invalid(NativeConfigurationField.Issuer)
            return NativeIdentityAvailability.Available(NativeClientConfiguration(apiOrigin, identity))
        }

        /** Legacy refusal-only factory; host activation uses [resolve] to retain the diagnosis. */
        fun create(apiOrigin: String, issuer: String): NativeClientConfiguration? =
            when (val availability = resolve(apiOrigin, issuer)) {
                is NativeIdentityAvailability.Available -> availability.configuration
                is NativeIdentityAvailability.Missing -> null
                is NativeIdentityAvailability.Invalid -> null
            }

        private fun validApiOrigin(apiOrigin: String): Boolean {
            val match = Regex("https://([a-z0-9.-]+)(?::([0-9]{1,5}))?").matchEntire(apiOrigin) ?: return false
            val host = match.groupValues[1]
            val port = match.groupValues[2]
            return !host.startsWith('.') && !host.endsWith('.') && !host.contains("..") &&
                (port.isEmpty() || port.toInt() in 1..65535)
        }
    }
}
