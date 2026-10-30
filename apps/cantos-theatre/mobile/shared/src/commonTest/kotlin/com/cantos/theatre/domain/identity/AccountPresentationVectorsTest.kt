package com.cantos.theatre.domain.identity

import com.cantos.ui.AvatarInitials
import com.cantos.ui.avatarInitials
import com.cantos.ui.avatarTone
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals

class AccountPresentationVectorsTest {
    @Test
    fun canonical_server_vectors_keep_unicode_initials_and_tone_across_locales() {
        for (vector in Json.parseToJsonElement(accountPresentationVectorJson).jsonArray) {
            val row = vector.jsonObject
            val view = row.getValue("presentation").jsonObject
            val alias = view.getValue("alias").jsonPrimitive.content
            val seed = view.getValue("avatar_seed").jsonPrimitive.content
            val p = AccountPresentation(alias, seed)
            assertEquals(row.getValue("initials").jsonPrimitive.content, avatarInitials(p.alias, AvatarInitials.Pair))
            assertEquals(row.getValue("tone").jsonPrimitive.content, avatarTone(p.avatarSeed).name.lowercase())
        }
        assertEquals("ML", avatarInitials("Mây Lam", AvatarInitials.Pair))
        assertEquals("TN", avatarInitials("Trăng Non", AvatarInitials.Pair))
        assertEquals("A\u0301D", avatarInitials("A\u0301nh Dương", AvatarInitials.Pair))
    }
}
