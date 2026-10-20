package com.cantos.theatre.adapters.http

import com.cantos.theatre.adapters.http.registrationwire.Accepted
import com.cantos.theatre.adapters.http.registrationwire.EmailRequest
import com.cantos.theatre.adapters.http.registrationwire.FirebaseConfirmRequest
import com.cantos.theatre.adapters.http.registrationwire.FirebaseConfirmation
import com.cantos.theatre.adapters.http.registrationwire.NativeConfiguration
import com.cantos.theatre.adapters.http.registrationwire.ProofRequest
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals

class RegistrationWireTest {
    @Test fun generated_types_consume_every_independent_registration_fixture() {
        val json = Json { classDiscriminator = "status" }
        val cases = json.parseToJsonElement(registrationFixtureJson) as JsonArray
        for (case in cases) {
            val fields = case.jsonObject
            val value = fields.getValue("value").toString()
            val accepted = try {
                when (fields.getValue("kind").jsonPrimitive.content) {
                    "email_request" -> json.decodeFromString<EmailRequest>(value)
                    "proof_request" -> json.decodeFromString<ProofRequest>(value)
                    "firebase_confirm_request" -> json.decodeFromString<FirebaseConfirmRequest>(value)
                    "firebase_confirmation" -> json.decodeFromString<FirebaseConfirmation>(value)
                    "native_configuration" -> json.decodeFromString<NativeConfiguration>(value)
                    "accepted" -> json.decodeFromString<Accepted>(value)
                    else -> error("Uncovered canonical registration fixture kind")
                }
                true
            } catch (_: SerializationException) { false }
            assertEquals(fields.getValue("valid").jsonPrimitive.boolean, accepted, fields.getValue("kind").toString())
        }
    }
}
