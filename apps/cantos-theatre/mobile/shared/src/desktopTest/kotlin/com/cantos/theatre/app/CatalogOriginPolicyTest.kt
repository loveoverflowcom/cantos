package com.cantos.theatre.app

import com.cantos.theatre.adapters.http.CleartextLoopbackHosts
import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals

/** The runtime origin policy and the debug network security config must name the same hosts. */
class CatalogOriginPolicyTest {
    @Test
    fun cleartext_hosts_equal_the_debug_network_security_config_domains() {
        val config = File("../androidApp/src/debug/res/xml/cantos_loopback_network_security.xml").readText()
        val cleartext = Regex("""<domain-config cleartextTrafficPermitted="true">(.*?)</domain-config>""", RegexOption.DOT_MATCHES_ALL)
            .find(config)?.groupValues?.get(1) ?: error("no cleartext domain-config")
        val domains = Regex("""<domain[^>]*>([^<]+)</domain>""").findAll(cleartext).map { it.groupValues[1].trim() }.toSet()
        assertEquals(CleartextLoopbackHosts, domains)
        assertEquals(true, config.contains("""<base-config cleartextTrafficPermitted="false" />"""))
    }
}
