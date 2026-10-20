package com.cantos.theatre.domain.identity

enum class NativeConfigurationField { ApiOrigin, Issuer, Callback }
enum class NativeConfigurationFields { ApiOrigin, Issuer, Both }

/** Configuration diagnostics contain key names only, never untrusted build-input values. */
sealed interface NativeIdentityAvailability {
    data class Available(val configuration: NativeClientConfiguration) : NativeIdentityAvailability
    data class Missing(val fields: NativeConfigurationFields) : NativeIdentityAvailability
    data class Invalid(val field: NativeConfigurationField) : NativeIdentityAvailability
}

class NativeClientConfiguration private constructor(val apiOrigin: String, val identity: NativeConfiguration) {
    companion object {
        /** Missing keys take precedence; with both supplied, validate before any activation. */
        fun resolve(
            apiOrigin: String?,
            issuer: String?,
            profile: NativeCallbackProfile = NativeCallbackProfile.Production,
        ): NativeIdentityAvailability {
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
            val identity = NativeConfiguration.create(issuer, profile)
                ?: return NativeIdentityAvailability.Invalid(NativeConfigurationField.Issuer)
            return NativeIdentityAvailability.Available(NativeClientConfiguration(apiOrigin, identity))
        }

        /**
         * Host entry point: [applicationId] is the final Android application ID. An ID without a
         * profile is a callback refusal, never a fallback to the production scheme; a missing
         * issuer still only disables identity and is reported before the callback.
         */
        fun resolveForApplication(apiOrigin: String?, issuer: String?, applicationId: String): NativeIdentityAvailability {
            val profile = NativeCallbackProfile.forApplicationId(applicationId)
            val resolved = resolve(apiOrigin, issuer, profile ?: NativeCallbackProfile.Production)
            return if (profile == null && resolved is NativeIdentityAvailability.Available) {
                NativeIdentityAvailability.Invalid(NativeConfigurationField.Callback)
            } else {
                resolved
            }
        }

        /** Legacy refusal-only factory; host activation uses [resolve] to retain the diagnosis. */
        fun create(apiOrigin: String, issuer: String): NativeClientConfiguration? =
            when (val availability = resolve(apiOrigin, issuer)) {
                is NativeIdentityAvailability.Available -> availability.configuration
                is NativeIdentityAvailability.Missing -> null
                is NativeIdentityAvailability.Invalid -> null
            }

        internal fun validApiOrigin(apiOrigin: String): Boolean {
            val match = Regex("https://([a-z0-9.-]+)(?::([0-9]{1,5}))?").matchEntire(apiOrigin) ?: return false
            val host = match.groupValues[1]
            val port = match.groupValues[2]
            return !host.startsWith('.') && !host.endsWith('.') && !host.contains("..") &&
                (port.isEmpty() || port.toInt() in 1..65535)
        }
    }
}
