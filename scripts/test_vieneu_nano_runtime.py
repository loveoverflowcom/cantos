"""Dependency-free safety tests; no installed engine or downloaded model is used."""

import hashlib
from contextlib import nullcontext
import builtins
import importlib.metadata
import io
import json
import math
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import Mock, patch
import wave

import vieneu_nano_runtime as runtime


def request_bytes(**changes):
    request = {
        "provider_id": runtime.PROVIDER_ID,
        "model_id": runtime.MODEL_ID,
        "adapter_version": runtime.ADAPTER_VERSION,
        "voice_id": "duc-tri",
        "language": "vi-VN",
        "text": "Đức Trí hỏi: Mỹ Duyên đã đến Huế chưa?",
    }
    request.update(changes)
    return json.dumps(request, ensure_ascii=False).encode("utf-8")


class RequestValidationTests(unittest.TestCase):
    def assert_rejected(self, raw):
        with self.assertRaisesRegex(runtime.RuntimeFailure, "^invalid_request$"):
            runtime.parse_request(raw)

    def test_real_voices_and_vietnamese_are_accepted(self):
        for voice in runtime.VOICE_NAMES:
            for language in ("vi", "vi-VN"):
                with self.subTest(voice=voice, language=language):
                    parsed = runtime.parse_request(request_bytes(voice_id=voice, language=language))
                    self.assertEqual(parsed["voice_id"], voice)

    def test_exact_identity_and_no_ignored_controls(self):
        for changes in (
            {"provider_id": "other"}, {"model_id": "other"},
            {"adapter_version": "other"}, {"voice_id": "Adam"},
            {"voice_id": ["duc-tri"]}, {"language": "en"},
            {"speed": 1.0}, {"emotion": "happy"}, {"pitch": 0},
            {"reference_audio": "/private/voice.wav"},
        ):
            with self.subTest(changes=changes):
                self.assert_rejected(request_bytes(**changes))

    def test_empty_long_and_unusual_unicode_are_rejected(self):
        for text in ("", "  ", "a" * 2001, "ế" * 667, "Xin\nchào",
                     "Xin\tchào", "[happy] Xin chào", "Xin ① chào", "🙂",
                     "中文", "Xin\u202echào", "e\u0302\u0301", "\ud800"):
            with self.subTest(text=repr(text)):
                if "\ud800" in text:
                    raw = request_bytes(text="placeholder").replace(b"placeholder", b"\\ud800")
                else:
                    raw = request_bytes(text=text)
                self.assert_rejected(raw)
        self.assertTrue(runtime.valid_text("ế" * 666))
        self.assertTrue(runtime.valid_text("a" * 2000))

    def test_malformed_duplicate_and_bounded_request(self):
        duplicates = request_bytes().replace(b'"voice_id": "duc-tri"',
                                             b'"voice_id": "duc-tri", "voice_id": "my-duyen"')
        for raw in (b"", b"\xff", b"[]", b"null", b"{", duplicates,
                    b'{"x": NaN}', b"[" * 2000 + b"]" * 2000,
                    b" " * (runtime.MAX_REQUEST_BYTES + 1)):
            with self.subTest(raw=raw[:30]):
                self.assert_rejected(raw)


class ArtifactValidationTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.path = Path(self.directory.name).resolve()
        self.contents = b"pinned miniature test graph"
        self.identity = (len(self.contents), hashlib.sha256(self.contents).hexdigest())
        (self.path / "graph.onnx").write_bytes(self.contents)

    def tearDown(self):
        self.directory.cleanup()

    def load_bundle(self):
        with patch.object(runtime, "MODEL_FILES", {"graph.onnx": self.identity}):
            return runtime.read_model_bundle(self.path)

    def test_exact_regular_bytes_returned(self):
        self.assertEqual(self.load_bundle(), {"graph.onnx": self.contents})

    def test_checksum_size_extra_and_missing_files(self):
        for changed in (b"x" * len(self.contents), self.contents[:-1], self.contents + b"x"):
            (self.path / "graph.onnx").write_bytes(changed)
            with self.assertRaisesRegex(runtime.RuntimeFailure, "^model_identity_mismatch$"):
                self.load_bundle()
        (self.path / "graph.onnx").write_bytes(self.contents)
        (self.path / "extra.py").write_text("untrusted")
        with self.assertRaisesRegex(runtime.RuntimeFailure, "^model_identity_mismatch$"):
            self.load_bundle()
        (self.path / "extra.py").unlink()
        (self.path / "graph.onnx").unlink()
        with self.assertRaisesRegex(runtime.RuntimeFailure, "^model_identity_mismatch$"):
            self.load_bundle()

    def test_file_symlink_directory_symlink_and_fifo_are_rejected(self):
        graph = self.path / "graph.onnx"
        graph.unlink()
        graph.symlink_to("missing")
        with self.assertRaisesRegex(runtime.RuntimeFailure, "^model_identity_mismatch$"):
            self.load_bundle()
        graph.unlink()
        os.mkfifo(graph)
        with self.assertRaisesRegex(runtime.RuntimeFailure, "^model_identity_mismatch$"):
            self.load_bundle()
        graph.unlink()
        graph.write_bytes(self.contents)
        alias = self.path / "alias"
        alias.symlink_to(self.path, target_is_directory=True)
        with self.assertRaisesRegex(runtime.RuntimeFailure, "^unsafe_path$"):
            runtime._directory_fd(alias)
        with self.assertRaisesRegex(runtime.RuntimeFailure, "^unsafe_path$"):
            runtime._directory_fd(alias / "child")
        for unsafe in ("relative", self.path / ".." / self.path.name):
            with self.assertRaisesRegex(runtime.RuntimeFailure, "^unsafe_path$"):
                runtime._directory_fd(unsafe)

    def test_missing_and_mismatched_runtime(self):
        with patch.object(importlib.metadata, "version", side_effect=importlib.metadata.PackageNotFoundError):
            with self.assertRaisesRegex(runtime.RuntimeFailure, "^missing_runtime$"):
                runtime.runtime_preflight()
        with patch.object(importlib.metadata, "version", return_value="unexpected"):
            with self.assertRaisesRegex(runtime.RuntimeFailure, "^runtime_identity_mismatch$"):
                runtime.runtime_preflight()

    def test_sdk_and_preset_bytes_are_verified(self):
        preset_bytes = b'{"presets": {"test": {}}}'
        preset = self.path / "preset.json"
        preset.write_bytes(preset_bytes)
        distribution = Mock()
        distribution.locate_file.return_value = preset
        versions = lambda package: runtime.DEPENDENCY_VERSIONS[package]
        identities = {"preset.json": (len(preset_bytes), hashlib.sha256(preset_bytes).hexdigest())}
        with patch.object(runtime, "SDK_FILES", identities), patch.object(runtime, "PRESET_PATH", "preset.json"), \
                patch.object(importlib.metadata, "version", side_effect=versions), \
                patch.object(importlib.metadata, "distribution", return_value=distribution):
            self.assertEqual(runtime.runtime_preflight(), {"presets": {"test": {}}})
            preset.write_bytes(b"x" * len(preset_bytes))
            with self.assertRaisesRegex(runtime.RuntimeFailure, "^runtime_identity_mismatch$"):
                runtime.runtime_preflight()

    def test_runtime_preflight_precedes_model_access(self):
        with patch.object(runtime, "runtime_preflight", side_effect=runtime.RuntimeFailure("missing_runtime")), \
                patch.object(runtime, "read_model_bundle") as model:
            with self.assertRaisesRegex(runtime.RuntimeFailure, "^missing_runtime$"):
                runtime.render(runtime.parse_request(request_bytes()), self.path)
            model.assert_not_called()


class PhonemeAndAudioTests(unittest.TestCase):
    def test_all_phonemes_checked_and_oov_never_dropped(self):
        normalize = Mock(return_value=(["one", "two"], ["sentence"]))
        phonemize = Mock(side_effect=["ab", "ba"])
        self.assertEqual(runtime.checked_phonemes("text", normalize, phonemize, {"a": 1, "b": 2}),
                         (["ab", "ba"], ["sentence"]))
        normalize.assert_called_once_with("text", max_chars=140)
        with self.assertRaisesRegex(runtime.RuntimeFailure, "^unsupported_phonemes$"):
            runtime.checked_phonemes("text", normalize, Mock(side_effect=["ab", "b?"]), {"a": 1, "b": 2})

    def test_empty_excessive_or_invalid_chunks_and_gaps(self):
        for chunks, gaps in (([], []), (["a"] * 17, ["minor"] * 16),
                             (["a", "b"], []), (["a", "b"], ["unknown"])):
            with self.subTest(chunks=len(chunks), gaps=gaps):
                with self.assertRaisesRegex(runtime.RuntimeFailure, "^unsupported_phonemes$"):
                    runtime.checked_phonemes("t", lambda *_a, **_k: (chunks, gaps), lambda value: value, {"a": 1})
        for phone in ("", " ", "a" * 4097, None):
            with self.subTest(phone=str(phone)[:20]):
                with self.assertRaisesRegex(runtime.RuntimeFailure, "^unsupported_phonemes$"):
                    runtime.checked_phonemes("t", lambda *_a, **_k: (["t"], []), lambda _t: phone, {"a": 1})

    def test_wav_format_and_explicit_quantization(self):
        data = runtime.encode_pcm_wav([-1.0, -0.5, 0.0, 0.5, 1.0, 0.5 / 32768, 1.5 / 32768])
        with wave.open(io.BytesIO(data), "rb") as wav:
            self.assertEqual((wav.getnchannels(), wav.getsampwidth(), wav.getframerate(), wav.getnframes()),
                             (1, 2, 24000, 7))
            self.assertEqual(wav.getcomptype(), "NONE")
            self.assertEqual(struct.unpack("<7h", wav.readframes(7)), (-32768, -16384, 0, 16384, 32767, 0, 2))

    def test_audio_must_be_finite_bounded_nonempty_and_mono(self):
        class Oversize:
            def __len__(self):
                return runtime.MAX_FRAMES + 1

        class Stereo(list):
            ndim = 2

        for samples in ([], [True], [float("nan")], [float("inf")], [-1.00001], [1.00001],
                        [None], ["audio"], Oversize(), Stereo([0.0])):
            with self.subTest(samples=repr(samples)):
                with self.assertRaisesRegex(runtime.RuntimeFailure, "^invalid_audio$"):
                    runtime.encode_pcm_wav(samples)


class OfflineAndProcessTests(unittest.TestCase):
    def test_audit_denies_network_children_and_writes(self):
        for event in ("socket.__new__", "socket.connect", "subprocess.Popen", "os.system", "os.exec", "os.spawn",
                      "os.posix_spawn", "os.fork", "os.forkpty"):
            with self.assertRaisesRegex(runtime.RuntimeFailure, "^runtime_network_or_process_forbidden$"):
                runtime._offline_audit(event, ())
        for flag in (os.O_WRONLY, os.O_RDWR, os.O_CREAT, os.O_TRUNC, os.O_APPEND):
            with self.assertRaisesRegex(runtime.RuntimeFailure, "^runtime_write_forbidden$"):
                runtime._offline_audit("open", ("private", None, flag))
        for event in ("os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.chmod",
                      "os.chown", "os.truncate", "os.utime", "os.link", "os.symlink"):
            with self.assertRaisesRegex(runtime.RuntimeFailure, "^runtime_write_forbidden$"):
                runtime._offline_audit(event, ())
        runtime._offline_audit("open", ("private", None, os.O_RDONLY))

    def test_bootstrap_never_permits_network_processes_or_filesystem_writes(self):
        for event in ("socket.__new__", "socket.bind", "socket.connect", "socket.getaddrinfo",
                      "socket.sendto", "subprocess.Popen", "os.system", "os.exec", "os.spawn",
                      "os.posix_spawn", "os.fork", "os.forkpty"):
            with self.assertRaisesRegex(runtime.RuntimeFailure, "^runtime_network_or_process_forbidden$"):
                runtime._bootstrap_audit(event, ())
        with self.assertRaisesRegex(PermissionError, "^runtime_write_forbidden$"):
            runtime._bootstrap_audit("open", ("private", None, os.O_WRONLY | os.O_CREAT))
        for event in ("os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.chmod",
                      "os.chown", "os.truncate", "os.utime", "os.link", "os.symlink"):
            with self.assertRaisesRegex(PermissionError, "^runtime_write_forbidden$"):
                runtime._bootstrap_audit(event, ())
        runtime._bootstrap_audit("open", ("private", None, os.O_RDONLY))

    def test_import_capability_handlers_can_recover_without_creating_anything(self):
        helper = Path(runtime.__file__).resolve()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory).resolve()
            code = """
import json, pathlib, socket, sys, tempfile
namespace = {"__name__": "probe_test"}
exec(pathlib.Path(sys.argv[1]).read_text(), namespace)
tempfile.tempdir = sys.argv[2]
sys.addaudithook(namespace["_bootstrap_audit"])
write_denied = False
try:
    with tempfile.TemporaryDirectory():
        raise AssertionError("must not create a directory")
except OSError:
    write_denied = True
network_denied = False
try:
    socket.socket(socket.AF_INET6)
except namespace["RuntimeFailure"]:
    network_denied = True
sys.addaudithook(namespace["_offline_audit"])
later_write_denied = False
try:
    pathlib.Path(sys.argv[2], "forbidden").write_text("forbidden")
except PermissionError:
    later_write_denied = True
print(json.dumps([write_denied, network_denied, later_write_denied]))
"""
            result = subprocess.run([sys.executable, "-I", "-c", code, str(helper), str(path)],
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout), [True, True, True])
            self.assertEqual(list(path.iterdir()), [])

    def test_bootstrap_then_full_guard_surround_all_dependency_imports(self):
        numpy = types.ModuleType("numpy")
        onnx = types.ModuleType("onnxruntime")
        nano = types.ModuleType("vieneu.v3nano")
        nano.OnnxV3NanoEngine = object
        phonemes = types.ModuleType("vieneu_utils.phonemize_text")
        phonemes.normalize_to_chunks_v3_with_gaps = Mock()
        phonemes.phonemize_text = Mock()
        core = types.ModuleType("vieneu_utils.core_utils")
        core.join_audio_chunks = Mock()
        core.gaps_to_silence = Mock()
        modules = {"numpy": numpy, "onnxruntime": onnx, "vieneu": types.ModuleType("vieneu"),
                   "vieneu.v3nano": nano, "vieneu_utils": types.ModuleType("vieneu_utils"),
                   "vieneu_utils.phonemize_text": phonemes, "vieneu_utils.core_utils": core}
        order = []
        original_import = builtins.__import__

        def record_import(name, *args, **kwargs):
            if name in modules:
                self.assertIs(order[0], runtime._bootstrap_audit)
                self.assertNotIn(runtime._offline_audit, order)
                order.append(name)
            return original_import(name, *args, **kwargs)

        with patch.dict(sys.modules, modules), patch.object(sys, "addaudithook", side_effect=order.append), \
                patch.object(builtins, "__import__", side_effect=record_import), \
                patch.object(tempfile, "tempdir", None):
            dependencies = runtime.import_runtime_dependencies("/verified/model")
            self.assertEqual(tempfile.gettempdir(), "/verified/model")
        self.assertIs(order[-1], runtime._offline_audit)
        self.assertEqual(order[1:-1], ["numpy", "onnxruntime", "vieneu.v3nano",
                                     "vieneu_utils.phonemize_text", "vieneu_utils.core_utils"])
        self.assertIs(dependencies[0], numpy)

    def test_fixed_process_invalid_input_has_no_stdout_or_private_echo(self):
        helper = Path(runtime.__file__).read_text()
        private = "PRIVATE_SECRET_VIETNAMESE_TEXT"
        result = subprocess.run([sys.executable, "-I", "-c", helper, "/missing/model"],
                                input=request_bytes(provider_id=private), stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, timeout=5, check=False)
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        self.assertEqual(result.stderr, b"vieneu_nano_runtime:invalid_request\n")
        self.assertNotIn(private.encode(), result.stderr)


class SamplerInitializationTests(unittest.TestCase):
    def test_sessions_consume_verified_bytes_and_only_bounded_cpu_threads(self):
        class FakeArray:
            def astype(self, _dtype):
                return self

            def __getitem__(self, _index):
                return self

            def __ne__(self, _other):
                return "mask"

        class Constants:
            def __enter__(self):
                return {"null_spk": FakeArray(), "null_style": FakeArray()}

            def __exit__(self, *_arguments):
                return False

        class Engine:
            def __init__(self, *_args, **_kwargs):
                raise AssertionError("The filename/download constructor must never run")

        class Options:
            def __init__(self):
                self.entries = {}

            def add_session_config_entry(self, key, value):
                self.entries[key] = value

        sessions = []

        def create_session(data, options, providers):
            session = Mock()
            session.get_providers.return_value = ["CPUExecutionProvider"]
            session.run.return_value = ["null_context"]
            sessions.append((data, options, providers))
            return session

        numpy = types.ModuleType("numpy")
        numpy.float32, numpy.int64 = "float32", "int64"
        numpy.array = Mock(return_value=FakeArray())
        numpy.load = Mock(return_value=Constants())
        onnx = types.ModuleType("onnxruntime")
        onnx.SessionOptions = Options
        onnx.GraphOptimizationLevel = types.SimpleNamespace(ORT_ENABLE_ALL="all")
        onnx.InferenceSession = create_session
        vieneu = types.ModuleType("vieneu")
        nano = types.ModuleType("vieneu.v3nano")
        nano.OnnxV3NanoEngine = Engine
        config = {"sample_rate": 24000, "flow_fps": 15.625, "steps_default": 16,
                  "cfg_default": 3.0, "vocab": {"a": 1}, "bos_id": 1,
                  "eos_id": 2, "pad_id": 0}
        bundle = {name: ("verified-" + name).encode() for name in runtime.MODEL_FILES}
        bundle["config.json"] = json.dumps(config).encode()
        with patch.dict(sys.modules, {"numpy": numpy, "onnxruntime": onnx,
                                      "vieneu": vieneu, "vieneu.v3nano": nano}):
            engine = runtime.load_sampler(bundle)
        self.assertEqual(engine._null_ctx, "null_context")
        self.assertEqual(engine._null_mask, "mask")
        self.assertEqual(engine.emotion_map, {})
        self.assertEqual(len(sessions), 4)
        self.assertEqual([entry[0] for entry in sessions],
                         [bundle[name] for name in ("text_encoder.onnx", "duration_predictor.onnx",
                                                    "vector_estimator.onnx", "codec_decoder.onnx")])
        for data, options, providers in sessions:
            self.assertIsInstance(data, bytes)
            self.assertEqual(providers, ["CPUExecutionProvider"])
            self.assertEqual((options.intra_op_num_threads, options.inter_op_num_threads), (2, 1))
            self.assertEqual(options.entries, {"session.intra_op.allow_spinning": "0"})
        arguments, keywords = numpy.load.call_args
        self.assertEqual(arguments[0].getvalue(), bundle["constants.npz"])
        self.assertEqual(keywords, {"allow_pickle": False})


class DurationPreflightTests(unittest.TestCase):
    class Array:
        def __init__(self, shape):
            self.shape = shape
            self.reshape_calls = []

        def reshape(self, *shape):
            self.reshape_calls.append(shape)
            return self

        def __ne__(self, _other):
            return "mask"

    def setUp(self):
        self.speaker = self.Array((192,))
        self.style = self.Array((50, 256))
        self.ids = self.Array((1, 10))
        self.numpy = types.ModuleType("numpy")
        self.numpy.float32 = "float32"
        self.numpy.asarray = lambda value, dtype: value
        self.engine = types.SimpleNamespace(
            _lock=nullcontext(), pad=0, encode_phones=Mock(return_value=self.ids),
            s_text=Mock(), s_dur=Mock(), infer=Mock(), vocab={"a": 1, "b": 2},
        )
        self.engine.s_text.run.return_value = ["context"]

    def check_duration(self, value):
        self.engine.s_dur.run.return_value = [[value]]
        return runtime.checked_chunk_duration(self.engine, "ab", self.speaker, self.style, self.numpy)

    def test_prediction_matches_pinned_graph_inputs_and_fixed_speed(self):
        self.assertAlmostEqual(self.check_duration(math.log(3.25)), 3.25)
        self.assertAlmostEqual(self.check_duration(math.log(15.0)), 15.0)
        self.engine.encode_phones.assert_called_with("ab")
        self.engine.s_text.run.assert_called_with(None, {"ids": self.ids, "style": self.style})
        self.engine.s_dur.run.assert_called_with(None, {"ctx": "context", "ctx_mask": "mask",
                                                       "spk": self.speaker})
        self.assertEqual(self.speaker.reshape_calls, [(1, -1), (1, -1)])
        self.assertEqual(self.style.reshape_calls, [((1, 50, 256),), ((1, 50, 256),)])
        self.engine.infer.assert_not_called()

    def test_overlong_nonfinite_or_unusable_predictions_are_rejected(self):
        for value in (math.log(15.001), 10000.0, -10000.0, float("nan"),
                      float("inf"), -float("inf"), None, "invalid"):
            with self.subTest(value=value):
                with self.assertRaisesRegex(runtime.RuntimeFailure, "^unsupported_duration$"):
                    self.check_duration(value)
        self.engine.s_dur.run.return_value = []
        with self.assertRaisesRegex(runtime.RuntimeFailure, "^unsupported_duration$"):
            runtime.checked_chunk_duration(self.engine, "ab", self.speaker, self.style, self.numpy)
        self.engine.infer.assert_not_called()

    def test_later_overlong_chunk_prevents_all_speech_generation(self):
        phonemes = types.ModuleType("vieneu_utils.phonemize_text")
        phonemes.normalize_to_chunks_v3_with_gaps = lambda *_a, **_kw: (["a", "b"], ["sentence"])
        phonemes.phonemize_text = lambda value: value
        core = types.ModuleType("vieneu_utils.core_utils")
        core.join_audio_chunks = Mock()
        core.gaps_to_silence = Mock()
        self.engine.s_dur.run.side_effect = [[[math.log(1.0)]], [[math.log(16.0)]]]
        preset = {"presets": {"Đức Trí": {"speaker_emb": self.speaker, "style": self.style}}}
        modules = {"numpy": self.numpy, "vieneu_utils": types.ModuleType("vieneu_utils"),
                   "vieneu_utils.phonemize_text": phonemes, "vieneu_utils.core_utils": core}
        order = []

        def import_dependencies(_directory):
            order.append("imports_guarded")
            return (self.numpy, phonemes.normalize_to_chunks_v3_with_gaps,
                    phonemes.phonemize_text, core.join_audio_chunks, core.gaps_to_silence)

        def load_sampler(_bundle):
            self.assertEqual(order, ["imports_guarded"])
            self.assertEqual(os.environ["VECLIB_MAXIMUM_THREADS"], "2")
            order.append("load_sampler")
            return self.engine

        with patch.dict(sys.modules, modules), patch.dict(os.environ), \
                patch.object(runtime, "runtime_preflight", return_value=preset), \
                patch.object(runtime, "read_model_bundle", return_value={}), \
                patch.object(runtime, "load_sampler", side_effect=load_sampler), \
                patch.object(runtime, "import_runtime_dependencies", side_effect=import_dependencies), \
                patch.object(runtime, "quiet_runtime", return_value=nullcontext()), \
                patch.object(sys, "addaudithook"), patch.object(runtime.logging, "disable"):
            with self.assertRaisesRegex(runtime.RuntimeFailure, "^unsupported_duration$"):
                runtime.render(runtime.parse_request(request_bytes()), "/unused/model")
        self.assertEqual(self.engine.s_dur.run.call_count, 2)
        self.assertEqual(order, ["imports_guarded", "load_sampler"])
        self.engine.infer.assert_not_called()
        core.join_audio_chunks.assert_not_called()


if __name__ == "__main__":
    unittest.main()
