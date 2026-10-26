package com.cantos.theatre.adapters.http

import kotlin.test.Test
import kotlin.test.assertEquals

class Sha256Test {
    @Test
    fun matches_the_fips_180_4_vectors() {
        assertEquals("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", sha256Hex(byteArrayOf()))
        assertEquals("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad", sha256Hex("abc".encodeToByteArray()))
        assertEquals(
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            sha256Hex("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".encodeToByteArray()),
        )
        assertEquals(
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
            sha256Hex(ByteArray(1_000_000) { 'a'.code.toByte() }),
        )
    }

    /** Lengths around the 55/56 and 64-byte padding boundaries, against a reference digest. */
    @Test
    fun matches_a_reference_digest_across_padding_boundaries() {
        val expected = mapOf(
        0 to "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        1 to "084fed08b978af4d7d196a7446a86b58009e636b611db16211b65a9aadff29c5",
        54 to "160bbf14b458c877b7049e7cb5771dd653930f97d20bdd8ee795c16062906233",
        55 to "e7313d333c272e639f790978283f9eb392e843d0f29b7016828bb1daa4aac70b",
        56 to "4324d65f3c103567f5589c710bc08f8523f929a9272e3af36fc968e52abc6c27",
        57 to "35df609437dcfea3279283ab79fd554e2bf78f8f7ae2de532d8ee300b09e8f73",
        63 to "81c80242132f230c3bd41b3e63bbcff16107339549214a99614ff26664625055",
        64 to "39e3d7b6b5d075d37d053ad89b24b41bef4f3c29760c84447cab3f3be1882241",
        65 to "aacca6ff74fdbb296d165a45cecfa04e5127bc008770fbbdd48006f2d2fae95e",
        119 to "9ce7368e4daf32341631b492e80359dc9f594b48453cd0dd5bf0b19279cc177e",
        120 to "7836b787757e95e58b3ca5aec90b1b004e8deba1e50e9675af9cabf1a13a04b5",
        128 to "d2742f1f4ac6bb7ca2b239ee18402ba8b3f9f8e652d2a72973c2b9ba11c08cf6",
        1000 to "1e9bc38cbf860b9ec31918b065f9b52476c549a782e0e7990bed8ce3868d2371",
        )
        expected.forEach { (length, digest) ->
            assertEquals(digest, sha256Hex(ByteArray(length) { i -> ((i * 7 + 3) and 0xff).toByte() }), "length $length")
        }
    }
}
