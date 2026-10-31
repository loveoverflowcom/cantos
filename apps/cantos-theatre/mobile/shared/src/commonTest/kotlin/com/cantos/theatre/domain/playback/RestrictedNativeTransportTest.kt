package com.cantos.theatre.domain.playback

import com.cantos.theatre.ports.PlaybackLocation
import kotlin.test.Test
import kotlin.test.assertEquals

class RestrictedNativeTransportTest {
    @Test fun url_only_decoders_refuse_shared_grants_even_if_the_caller_omits_the_marker() {
        for (scheme in listOf("http", "https")) {
            assertEquals(false, PlaybackLocation("$scheme://api.example.org/api/v1/theatre/shared/media/sentinel-grant").supportedByUrlOnlyDecoder)
        }
        assertEquals(false, PlaybackLocation("https://api.example.org/other", requiresSession = true).supportedByUrlOnlyDecoder)
        assertEquals(true, PlaybackLocation("https://api.example.org/api/v1/theatre/media/public-grant").supportedByUrlOnlyDecoder)
        assertEquals("PlaybackLocation([redacted])", PlaybackLocation("sentinel-private-url", true).toString())
    }
}
