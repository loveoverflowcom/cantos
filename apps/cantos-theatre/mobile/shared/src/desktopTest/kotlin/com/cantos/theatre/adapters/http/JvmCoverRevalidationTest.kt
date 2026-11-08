package com.cantos.theatre.adapters.http

import com.cantos.theatre.adapters.image.CoverDecoder
import com.cantos.theatre.adapters.image.SkiaCoverDecoder
import com.cantos.theatre.app.CoverImages
import com.cantos.theatre.features.shell.presentation.CoverLoad
import com.cantos.theatre.presentation.model.ShellCover
import com.cantos.theatre.presentation.model.ShellCoverVariant
import com.sun.net.httpserver.HttpServer
import kotlinx.coroutines.test.runTest
import java.net.InetSocketAddress
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertSame

/** Real loopback traffic through the JVM/Android transport with a synthetic authorization peer. */
class JvmCoverRevalidationTest {
    @Test
    fun conditional_get_reuses_only_admitted_pixels_and_transfers_no_image_bytes() = runTest {
        val bytes = thumbnailWebp()
        val sha = sha256Hex(bytes)
        val etag = "\"$sha\""
        val revalidations = AtomicInteger()
        val bodyBytes = AtomicInteger()
        val refused = AtomicBoolean()
        val decodes = AtomicInteger()
        val requests = AtomicInteger()
        val validHeaders = AtomicBoolean(true)
        val peer = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        peer.createContext("/cover") { exchange ->
            try {
                requests.incrementAndGet()
                if (exchange.requestHeaders.getFirst("Accept") != "image/webp" ||
                    exchange.requestHeaders.getFirst("Authorization") != null
                ) validHeaders.set(false)
                val supplied = exchange.requestHeaders.getFirst("If-None-Match")
                if (supplied == etag) revalidations.incrementAndGet()
                when {
                    refused.get() -> exchange.sendResponseHeaders(404, -1)
                    supplied == etag -> {
                        exchange.responseHeaders.set("ETag", etag)
                        exchange.sendResponseHeaders(304, -1)
                    }
                    else -> {
                        exchange.responseHeaders.set("Content-Type", "image/webp")
                        exchange.responseHeaders.set("ETag", etag)
                        exchange.sendResponseHeaders(200, bytes.size.toLong())
                        exchange.responseBody.write(bytes)
                        bodyBytes.addAndGet(bytes.size)
                    }
                }
            } finally {
                exchange.close()
            }
        }
        peer.start()
        try {
            val cover = ShellCover(
                "http://127.0.0.1:${peer.address.port}/cover", sha, bytes.size, "image/webp", 2, 3,
                "Bìa thử nghiệm", ShellCoverVariant.Thumbnail,
            )
            val loader = CoverImages(this, PublicCoverHttp(JvmCatalogTransport()), CoverDecoder { body, edge ->
                decodes.incrementAndGet()
                SkiaCoverDecoder.decode(body, edge)
            })
            val first = assertIs<CoverLoad.Ready>(loader.load(cover, 20, 30))
            val second = assertIs<CoverLoad.Ready>(loader.load(cover, 20, 30))
            assertSame(first.bitmap, second.bitmap)
            assertEquals(bytes.size, bodyBytes.get())
            assertEquals(1, decodes.get())
            refused.set(true)
            assertEquals(CoverLoad.Unavailable, loader.load(cover, 20, 30))
            refused.set(false)
            assertIs<CoverLoad.Ready>(loader.load(cover, 20, 30))
            assertEquals(4, requests.get())
            assertEquals(2, revalidations.get())
            assertEquals(bytes.size * 2, bodyBytes.get())
            assertEquals(2, decodes.get())
            assertEquals(true, validHeaders.get())
        } finally {
            peer.stop(0)
        }
    }
}
