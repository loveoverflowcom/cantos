package com.cantos.theatre.app

import com.cantos.theatre.resources.Res
import com.cantos.theatre.resources.allFontResources
import java.awt.Font
import java.io.ByteArrayInputStream
import java.security.MessageDigest
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class FontResourceTest {
    @Test
    fun font_provenance_records_only_the_exact_bundled_faces_and_their_license() = runTest {
        val provenance = Json.parseToJsonElement(Res.readBytes("files/provenance.json").decodeToString()).jsonObject
        assertEquals(setOf(
            "source", "source_package_sha256", "upstream", "version", "copyright", "license",
            "license_path", "license_sha256", "extraction", "files",
        ), provenance.keys)
        assertEquals("https://drive.google.com/file/d/157jbpHUTKZ-tDzNfXD878nvnF4thWmM4/view", provenance.getValue("source").jsonPrimitive.content)
        assertEquals("bf55c2651c3db82fb1b7d07a44658c930d27175b0ebadc083f58dff865871fc9", provenance.getValue("source_package_sha256").jsonPrimitive.content)
        assertEquals("https://github.com/notofonts/latin-greek-cyrillic", provenance.getValue("upstream").jsonPrimitive.content)
        assertEquals("2.015", provenance.getValue("version").jsonPrimitive.content)
        assertEquals("Copyright 2022 The Noto Project Authors", provenance.getValue("copyright").jsonPrimitive.content)
        assertEquals("SIL-OFL-1.1", provenance.getValue("license").jsonPrimitive.content)
        assertEquals("files/OFL.txt", provenance.getValue("license_path").jsonPrimitive.content)
        assertEquals("Unmodified complete Noto Sans and Noto Serif TrueType faces; no subsetting, conversion or local host lookup.",
            provenance.getValue("extraction").jsonPrimitive.content)

        val expected = mapOf(
            "font/noto_sans_regular.ttf" to Triple("Noto Sans", 400, "f5f552c8c5edb61fe6efb824baf4d4de47b1a8689ab4925ff43f7bd6a4ebece5"),
            "font/noto_sans_bold.ttf" to Triple("Noto Sans", 700, "3a08a47daa00cade516425c15c57615aef2fd418ec9811a7b9f465088f92cc05"),
            "font/noto_serif_regular.ttf" to Triple("Noto Serif", 400, "19e72cd8d595fae5bd74a5206f5d938512e1183d4fed7abb1ec1be1d7efa5f88"),
            "font/noto_serif_bold.ttf" to Triple("Noto Serif", 700, "96656aa5cec8f1d6fd0e804c1fad397e1a1cfa082e6642124e0bda68cd8363ce"),
        )
        val faces = provenance.getValue("files").jsonObject
        assertEquals(expected.keys, faces.keys)
        assertEquals(expected.keys.map { it.substringAfterLast('/').removeSuffix(".ttf") }.toSet(),
            Res.allFontResources.keys)
        expected.forEach { (path, face) ->
            val metadata = faces.getValue(path).jsonObject
            assertEquals(setOf("family", "weight", "sha256"), metadata.keys, path)
            assertEquals(face.first, metadata.getValue("family").jsonPrimitive.content, path)
            assertEquals(face.second, metadata.getValue("weight").jsonPrimitive.int, path)
            val recordedHash = metadata.getValue("sha256").jsonPrimitive.content
            assertEquals(face.third, recordedHash, path)
            assertEquals(recordedHash, sha256(Res.readBytes(path)), path)
        }
        val license = Res.readBytes(provenance.getValue("license_path").jsonPrimitive.content)
        val licenseText = license.decodeToString()
        assertTrue(licenseText.startsWith(provenance.getValue("copyright").jsonPrimitive.content))
        assertTrue(licenseText.contains("SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007"))
        val recordedLicenseHash = provenance.getValue("license_sha256").jsonPrimitive.content
        assertEquals("e2e177a32561584d4fc13aaa3cd8e53758a12910f013fe9ca125419111722029", recordedLicenseHash)
        assertEquals(recordedLicenseHash, sha256(license))
    }

    @Test
    fun all_bundled_font_weights_cover_vietnamese_including_stacked_and_combining_marks() = runTest {
        val required = (0x1EA0..0x1EF9).toSet() + setOf(0x110, 0x111, 0x1A0, 0x1A1, 0x1AF, 0x1B0,
            0x300, 0x301, 0x303, 0x309, 0x323)
        listOf("noto_sans_regular", "noto_sans_bold", "noto_serif_regular", "noto_serif_bold").forEach { name ->
            val bytes = Res.readBytes("font/$name.ttf")
            val font = Font.createFont(Font.TRUETYPE_FONT, ByteArrayInputStream(bytes))
            assertEquals(emptyList(), required.filterNot { font.canDisplay(it) }, name)
            assertTrue(font.family.startsWith("Noto "), name)
        }
    }

    private fun sha256(bytes: ByteArray): String = MessageDigest.getInstance("SHA-256")
        .digest(bytes).joinToString("") { "%02x".format(it.toInt() and 0xff) }

}
