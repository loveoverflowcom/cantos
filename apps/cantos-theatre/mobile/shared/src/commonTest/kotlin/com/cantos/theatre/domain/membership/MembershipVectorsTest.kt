package com.cantos.theatre.domain.membership

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue

/** Independent hand-authored expectations, consumed unchanged by Rust and Kotlin. */
class MembershipVectorsTest {
    @Test
    fun all_twelve_shared_validation_vectors_match_exact_independent_findings() {
        val corpus = corpus()
        val vectors = corpus.getValue("validation").jsonArray
        assertEquals(12, vectors.size, "The selected corpus must not silently shrink")
        vectors.forEach { raw ->
            val vector = raw.jsonObject
            assertEquals(setOf("id", "name", "email", "errors"), vector.keys)
            assertEquals(vector.getValue("errors").jsonArray.map { it.jsonPrimitive.content },
                validateMembershipDraft(MembershipDraft(vector.text("name"), vector.text("email"))).map { it.name },
                vector.text("id"))
        }
    }

    @Test
    fun all_seven_shared_transition_scenarios_match_outcomes_fences_and_projections() {
        val scenarios = corpus().getValue("scenarios").jsonArray
        assertEquals(7, scenarios.size, "The selected corpus must not silently shrink")
        scenarios.forEach { raw ->
            val scenario = raw.jsonObject
            assertEquals(setOf("id", "mode", "actions"), scenario.keys)
            var state = when (scenario.text("mode")) {
                "fixture" -> MembershipState.fixture()
                "unavailable" -> MembershipState.unavailable()
                else -> error("Unknown preview mode")
            }
            val attempts = mutableMapOf<String, SubmissionAttempt>()
            val actions = scenario.getValue("actions").jsonArray
            assertTrue(actions.isNotEmpty())
            actions.forEachIndexed { index, rawAction ->
                val action = rawAction.jsonObject
                assertTrue(action.keys.all { it in actionKeys }, "Unknown action field")
                val label = "${scenario.text("id")} action $index: ${action.text("type") }"
                val event = action.event(attempts)
                val transition = state.reduce(event)
                when (event) {
                    is MembershipEvent.SetName, is MembershipEvent.SetEmail -> {
                        val refusal = when (val result = transition.result) {
                            MembershipResult.Changed -> null
                            is MembershipResult.EditRefused -> result.reason.name
                            else -> error("Expected a typed edit outcome")
                        }
                        assertEquals(action.optionalText("expectedRefusal"), refusal, label)
                    }
                    MembershipEvent.Submit -> when (val result = transition.result) {
                        is MembershipResult.Started -> {
                            assertEquals(null, action.optionalText("expectedRefusal"), label)
                            action["operationId"]?.let { assertEquals(it.jsonPrimitive.content.toULong(), result.attempt.operationId, label) }
                            action["requestId"]?.let { assertEquals(it.jsonPrimitive.content.toULong(), result.attempt.requestId, label) }
                            action.optionalText("payloadName")?.let { assertEquals(it, result.attempt.payload.name, label) }
                            action.optionalText("payloadEmail")?.let { assertEquals(it, result.attempt.payload.email, label) }
                            attempts[action.text("saveAs")] = result.attempt
                        }
                        is MembershipResult.SubmitRefused -> assertEquals(action.text("expectedRefusal"), result.reason.code(), label)
                        else -> error("Expected a typed submission outcome")
                    }
                    else -> Unit
                }
                action["accepted"]?.let {
                    assertEquals(it.jsonPrimitive.boolean, transition.result == MembershipResult.Changed, label)
                }
                state = transition.state
                action["expect"]?.jsonObject?.forEach { (key, expected) ->
                    val actual = when (key) {
                        "screen" -> JsonPrimitive(state.screen.code())
                        "submission" -> JsonPrimitive(state.submission.name.lowercase())
                        "receipt" -> JsonPrimitive(state.receipt != null)
                        "name" -> JsonPrimitive(state.draft.name)
                        "email" -> JsonPrimitive(state.draft.email)
                        "plansStatus" -> JsonPrimitive(state.plansStatus.name.lowercase())
                        else -> error("Unknown projection")
                    }
                    assertEquals(expected, actual, "$label: $key")
                }
            }
        }
    }

    private fun corpus(): JsonObject = Json.parseToJsonElement(membershipVectorJson).jsonObject.also {
        assertEquals(setOf("schemaVersion", "boundary", "validation", "scenarios"), it.keys)
        assertEquals(1, it.getValue("schemaVersion").jsonPrimitive.int)
        assertTrue(it.text("boundary").contains("not an accepted backend"))
    }

    private fun JsonObject.event(attempts: Map<String, SubmissionAttempt>): MembershipEvent = when (text("type")) {
        "open-form" -> MembershipEvent.OpenForm
        "open-intro" -> MembershipEvent.OpenIntro
        "open-account" -> MembershipEvent.OpenAccount
        "show-received" -> MembershipEvent.ShowReceived
        "account-registered" -> MembershipEvent.ShowAccountRegisteredFixture
        "open-plans" -> MembershipEvent.OpenPlans
        "deep-plans" -> MembershipEvent.OpenPlansDeepLink
        "back" -> MembershipEvent.Back
        "clear-session" -> MembershipEvent.ClearSession
        "edit-name" -> MembershipEvent.SetName(text("value"))
        "edit-email" -> MembershipEvent.SetEmail(text("value"))
        "submit" -> MembershipEvent.Submit
        "plans-status" -> MembershipEvent.SetPlansStatus(when (text("value")) {
            "unpublished" -> PlansStatus.Unpublished
            "loading" -> PlansStatus.Loading
            "empty" -> PlansStatus.Empty
            "unavailable" -> PlansStatus.Unavailable
            "failed" -> PlansStatus.Failed
            else -> error("Unknown plans status")
        })
        "finish" -> MembershipEvent.Finish(attempts.getValue(text("attempt")), when (text("outcome")) {
            "received" -> MembershipFinishOutcome.Received
            "failed" -> MembershipFinishOutcome.Failed
            "unknown" -> MembershipFinishOutcome.Unknown
            else -> error("Unknown outcome")
        })
        else -> error("Unknown fixture action")
    }

    private fun MembershipScreen.code(): String = when (this) {
        MembershipScreen.Intro -> "intro"
        MembershipScreen.Form -> "form"
        MembershipScreen.Received -> "received"
        MembershipScreen.Account -> "account"
        MembershipScreen.AccountRegistered -> "account-registered"
        is MembershipScreen.Plans -> "plans:" + when (origin) {
            PlansOrigin.Intro -> "intro"
            PlansOrigin.Account -> "account"
            PlansOrigin.AccountRegistered -> "account-registered"
            PlansOrigin.Received -> "received"
        }
    }

    private fun MembershipSubmitRefusal.code(): String = when (this) {
        MembershipSubmitRefusal.NotOnForm -> "NotOnForm"
        is MembershipSubmitRefusal.InvalidFields -> "InvalidFields([${errors.joinToString { it.name }}])"
        MembershipSubmitRefusal.Unavailable -> "Unavailable"
        MembershipSubmitRefusal.AlreadyPending -> "AlreadyPending"
        MembershipSubmitRefusal.AlreadyReceived -> "AlreadyReceived"
        MembershipSubmitRefusal.SequenceExhausted -> "SequenceExhausted"
    }

    private fun JsonObject.text(key: String): String = assertIs<JsonPrimitive>(getValue(key)).also { assertTrue(it.isString) }.content
    private fun JsonObject.optionalText(key: String): String? = get(key)?.jsonPrimitive?.contentOrNull
    private val actionKeys = setOf("type", "value", "saveAs", "attempt", "outcome", "expectedRefusal", "accepted",
        "operationId", "requestId", "payloadName", "payloadEmail", "expect")
}
