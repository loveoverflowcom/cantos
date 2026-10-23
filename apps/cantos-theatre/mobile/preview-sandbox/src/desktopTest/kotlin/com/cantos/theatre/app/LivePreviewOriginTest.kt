package com.cantos.theatre.app

import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

/** The opt-in live mode admits only HTTPS or an explicit loopback `http` origin; anything else mounts nothing. */
class LivePreviewOriginTest {
    @Test fun only_https_and_explicit_loopback_http_are_admitted() {
        for (accepted in listOf("https://api.example", "https://api.example:8443", "http://127.0.0.1:8080", "http://localhost:8080")) {
            val ready = assertIs<HostOrigin.Ready>(selectLivePreviewOrigin(accepted), accepted)
            assertEquals(accepted.removeSuffix("/"), ready.origin.toString())
        }
    }

    @Test fun every_other_value_is_a_typed_refusal_that_selects_no_origin() {
        for (refused in listOf("", "   ", "http://example.com", "http://192.168.0.5:8080", "ftp://127.0.0.1",
            "api.example", "http://user@127.0.0.1:8080", "http://127.0.0.1:8080/path?x=1", "javascript:alert(1)")) {
            assertIs<HostOrigin.Refused>(selectLivePreviewOrigin(refused), refused)
        }
    }
}
