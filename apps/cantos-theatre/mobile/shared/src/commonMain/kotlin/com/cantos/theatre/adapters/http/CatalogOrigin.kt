package com.cantos.theatre.adapters.http

/**
 * The one owner of the production API origin for every native host (#298). A host supplies only
 * an optional explicit override; it never hardcodes another origin and a build type never selects
 * a different backend.
 */
const val DefaultProductionApiOrigin: String = "https://api.cantos.cc"

/** Why an explicit override is not admissible. Fail closed; never fall back to another environment. */
enum class ApiOriginRefusal {
    /** An override was supplied but is empty or blank. */
    Empty,
    /** Not a bare `scheme://host[:port]` origin (path, query, credentials, bad port, …). */
    Malformed,
    /** `http://` outside the loopback exception, or any other scheme. */
    NotHttps,
    /** `http://` on a debug build, but not on the exact loopback hosts the network config permits. */
    CleartextHostNotAllowed,
}

/** Where a configured origin came from; it is recorded for diagnostics, not for behavior. */
enum class OriginSource { Default, Override }

/** What a host's optional API origin override means for composing the live public catalog. */
sealed interface CatalogOriginSelection {
    data class Configured(val origin: ApiOrigin, val source: OriginSource = OriginSource.Override) : CatalogOriginSelection
    /** An override was supplied but is not admissible for this build: a typed refusal before any request. */
    data class Rejected(val refusal: ApiOriginRefusal) : CatalogOriginSelection
}

/**
 * Resolution order: explicit [override] → [DefaultProductionApiOrigin]. Only an **absent** (`null`)
 * override selects the default; an empty or invalid one is refused and never silently changes the
 * environment. Release builds admit only `https://` origins. [allowLoopbackHttp] is true only for a
 * debug build, where `http://` is further limited to `127.0.0.1` and `localhost` (for example
 * `adb reverse tcp:8080 tcp:8080`): exactly the hosts the debug network security config and
 * `cargo xtask theatre-host android-debug` permit. [ApiOrigin] also parses `[::1]`, but Android
 * would block that cleartext, so it is refused here. The value is used exactly as configured.
 */
fun selectCatalogOrigin(override: String?, allowLoopbackHttp: Boolean): CatalogOriginSelection {
    if (override == null) return CatalogOriginSelection.Configured(ApiOrigin.parse(DefaultProductionApiOrigin), OriginSource.Default)
    if (override.isBlank()) return CatalogOriginSelection.Rejected(ApiOriginRefusal.Empty)
    val origin = try {
        ApiOrigin.parse(override)
    } catch (_: IllegalArgumentException) {
        if (override.startsWith("http://") && runCatching { ApiOrigin.parse("https://" + override.removePrefix("http://")) }.isSuccess) {
            return CatalogOriginSelection.Rejected(
                if (allowLoopbackHttp) ApiOriginRefusal.CleartextHostNotAllowed else ApiOriginRefusal.NotHttps,
            )
        }
        // Another well-formed scheme (ftp://…) is a scheme refusal; everything else is not an origin at all.
        val scheme = override.substringBefore("://", missingDelimiterValue = "")
        val otherScheme = scheme.isNotEmpty() && scheme.all { it.isLetterOrDigit() } && scheme != "http" && scheme != "https"
        return CatalogOriginSelection.Rejected(if (otherScheme) ApiOriginRefusal.NotHttps else ApiOriginRefusal.Malformed)
    }
    if (!origin.isHttps) {
        val host = override.removePrefix("http://").substringBefore(':').lowercase()
        if (!allowLoopbackHttp) return CatalogOriginSelection.Rejected(ApiOriginRefusal.NotHttps)
        if (host !in CleartextLoopbackHosts) return CatalogOriginSelection.Rejected(ApiOriginRefusal.CleartextHostNotAllowed)
    }
    return CatalogOriginSelection.Configured(origin)
}

/**
 * Entry for a value read from native packaging (Android manifest placeholder, iOS `Info.plist`
 * build setting). Packaging cannot omit a key, so the build tooling's own empty value means
 * "no override was supplied". An explicit empty override is refused at the build/CLI boundary
 * (for example xtask's `--api-origin`), so an empty packaged value is never a user override.
 */
fun selectPackagedApiOrigin(packaged: String?, allowLoopbackHttp: Boolean): CatalogOriginSelection =
    selectCatalogOrigin(packaged?.takeIf { it.isNotEmpty() }, allowLoopbackHttp)

/** Must equal the debug network security config's cleartext domains. */
internal val CleartextLoopbackHosts = setOf("127.0.0.1", "localhost")
