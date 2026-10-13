// Optional JVM measurement primitive for the existing verification flow. No Native claim.
import com.cantos.theatre.adapters.protobuf.ProtobufResult
import com.cantos.theatre.adapters.protobuf.StudioPilotCodec
import com.cantos.theatre.adapters.protobuf.StudioPilotKind
import com.cantos.theatre.adapters.protobuf.ValidateScriptRequest
import java.lang.management.ManagementFactory
import com.sun.management.ThreadMXBean

private fun <T> value(result: ProtobufResult<T>): T = (result as ProtobufResult.Value<T>).value
fun main() {
    val bean = ManagementFactory.getThreadMXBean() as ThreadMXBean
    val allocationAvailable = bean.isThreadAllocatedMemorySupported && bean.isThreadAllocatedMemoryEnabled
    println("JVM-only Kotlin 2.2.20 / kotlinx protobuf 1.9.0; no transport/compression/device")
    println("script_utf8_bytes,protobuf_bytes,iterations,encode_ns_per_op,decode_ns_per_op,encode_alloc_bytes_per_op,decode_alloc_bytes_per_op")
    for (kib in listOf(1, 8, 64, 512)) {
        val input = ValidateScriptRequest("{\"text\":\"" + "a".repeat(kib * 1024 - 11) + "\"}")
        val bytes = value(StudioPilotCodec.encode(input))
        val iterations = if (kib <= 8) 2000 else if (kib <= 64) 400 else 100
        repeat(100) {
            check(value(StudioPilotCodec.decode(StudioPilotKind.ValidateScriptRequest, value(StudioPilotCodec.encode(input)))) == input)
        }
        fun measure(operation: () -> Unit): Pair<Long, Long> {
            val allocatedBefore = if (allocationAvailable) bean.getThreadAllocatedBytes(Thread.currentThread().threadId()) else 0L
            val started = System.nanoTime()
            repeat(iterations) { operation() }
            val elapsed = System.nanoTime() - started
            val allocatedAfter = if (allocationAvailable) bean.getThreadAllocatedBytes(Thread.currentThread().threadId()) else 0L
            return elapsed / iterations to if (allocationAvailable) (allocatedAfter - allocatedBefore) / iterations else -1L
        }
        val encode = measure { check(value(StudioPilotCodec.encode(input)).size == bytes.size) }
        val decode = measure { check(value(StudioPilotCodec.decode(StudioPilotKind.ValidateScriptRequest, bytes)) == input) }
        println("${input.scriptJson.toByteArray().size},${bytes.size},$iterations,${encode.first},${decode.first},${encode.second},${decode.second}")
    }
}
