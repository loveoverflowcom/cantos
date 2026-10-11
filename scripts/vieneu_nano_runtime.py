"""Fixed, offline VieNeu Nano sample helper, embedded by the Rust local adapter.

Only the pinned preset-only graph bundle and two explicit voices are admitted. This
is a private research sample path; it grants no production or publication rights.
The parent process owns the deadline, process limits and create-only output record.
No third-party implementation or model bytes are included in this repository.
"""

import sys

sys.dont_write_bytecode = True  # -I ignores PYTHONDONTWRITEBYTECODE.

import array
from contextlib import contextmanager
import hashlib
import importlib.metadata
import io
import json
import logging
import math
import os
from pathlib import Path
import stat
import tempfile
import threading
import unicodedata
import wave


PROVIDER_ID = "vieneu-nano-local"
MODEL_ID = "vieneu-nano-aba295eb96a6fa6003ebe417cc1f2802a7adc1dc"
ADAPTER_VERSION = "vieneu-nano-local-v1"
MODEL_REVISION = "aba295eb96a6fa6003ebe417cc1f2802a7adc1dc"
MAX_REQUEST_BYTES = 8_192
MAX_TEXT_BYTES = 2_000
MAX_PHONE_CHARS = 4_096
MAX_CHUNKS = 16
MAX_CHUNK_SECONDS = 15.0
SAMPLE_RATE = 24_000
MAX_FRAMES = SAMPLE_RATE * 120
VOICE_NAMES = {"duc-tri": "Đức Trí", "my-duyen": "Mỹ Duyên"}
DEPENDENCY_VERSIONS = {
    "vieneu": "3.8.3",
    "sea-g2p": "0.9.1",
    "onnxruntime": "1.24.4",
    "numpy": "2.3.3",
    "huggingface_hub": "0.35.3",
}

# Publisher metadata at MODEL_REVISION; hashes cover bytes, not a directory name.
MODEL_FILES = {
    "codec_decoder.onnx": (99_319_941, "b0ab15e7828a39d53679e25b1ba4ba415a61311307202a6323130b9e1cc3029d"),
    "duration_predictor.onnx": (727_809, "20fd7fa60006d0a48ee82e0451b3f920d2052588c083c756cce669f3947c0a68"),
    "text_encoder.onnx": (26_519_943, "204f02cccae1f16ccb2d3840f05721a37fe250b82cbd456337a0ccb49615e4bf"),
    "vector_estimator.onnx": (155_132_418, "c6c1d4398ca35d3ad1bd3f0459413d1b975d09e7f2f3a2b4493a7b92bbf6ce93"),
    "config.json": (2_927, "3762f1716fd8d7a1451ddf2e051c03b0a341e8adda4f1dbd0db0ae9777514e56"),
    "constants.npz": (53_448, "7c011938effe41687a9af85107a31a040dfcf0fb3d0f048ec10f4fb65c1a8829"),
}
PRESET_PATH = "vieneu/assets/voices_v3_nano.json"
SDK_FILES = {
    "vieneu/__init__.py": (50, "404060a9718835967be26ad022261dcd08d589605f0e24c0245637469b10439b"),
    "vieneu/factory.py": (2_065, "0cd4631298d4da9de59471c3f3371a8c894b2b9019e40f584373e2c1a3ca3265"),
    "vieneu/base.py": (19_719, "48cc3f8b809a6bfd06e6fea6344673541e547068fb310caaef0dbc1099af6ac5"),
    "vieneu/v3nano.py": (26_685, "0e0bb4c356d3c0292c8e0990f03de7e6a7e246db6b004d66e5dacb070c738de3"),
    "vieneu_utils/__init__.py": (0, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
    "vieneu_utils/core_utils.py": (41_652, "baff0fac1e3dfe38ff77e5401b289c5de17bcdc3b385640844e1846f45728bf9"),
    "vieneu_utils/phonemize_text.py": (19_882, "ce3c0ab79b1232cb9a62dc8360aedbdd7646318f46d59036603615a957dfe6a7"),
    PRESET_PATH: (2_316_799, "2ca4cbf475409e61fda662fbd3fb3a3ac3e980bbc3925fedb4d1199c4ccac3e7"),
}
VIETNAMESE_LETTERS = (
    "àáảãạăằắẳẵặâầấẩẫậèéẻẽẹêềếểễệìíỉĩịòóỏõọôồốổỗộơờớởỡợùúủũụưừứửữựỳýỷỹỵđ"
)
TEXT_CHARACTERS = frozenset(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 .,!?;:-'\"()"
    + VIETNAMESE_LETTERS + VIETNAMESE_LETTERS.upper()
)


class RuntimeFailure(Exception):
    """A fixed error class; sensitive text, paths and underlying errors stay private."""


def _unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise RuntimeFailure("invalid_request")
        result[key] = value
    return result


def _reject_constant(_value):
    raise RuntimeFailure("invalid_request")


def valid_text(text):
    if not isinstance(text, str) or not text.strip():
        return False
    try:
        encoded = text.encode("utf-8")
    except UnicodeEncodeError:
        return False
    return (len(encoded) <= MAX_TEXT_BYTES
            and unicodedata.normalize("NFC", text) == text
            and all(character in TEXT_CHARACTERS for character in text))


def parse_request(raw):
    """Validate the complete narrow process protocol before any runtime import."""
    if not isinstance(raw, bytes) or not raw or len(raw) > MAX_REQUEST_BYTES:
        raise RuntimeFailure("invalid_request")
    try:
        request = json.loads(raw.decode("utf-8"), object_pairs_hook=_unique_object,
                             parse_constant=_reject_constant)
    except (UnicodeError, ValueError, RecursionError):
        raise RuntimeFailure("invalid_request") from None
    if (not isinstance(request, dict)
            or set(request) != {"provider_id", "model_id", "adapter_version", "voice_id", "language", "text"}
            or request["provider_id"] != PROVIDER_ID
            or request["model_id"] != MODEL_ID
            or request["adapter_version"] != ADAPTER_VERSION
            or not isinstance(request["voice_id"], str)
            or request["voice_id"] not in VOICE_NAMES
            or request["language"] not in ("vi", "vi-VN")
            or not valid_text(request["text"])):
        raise RuntimeFailure("invalid_request")
    return request


def _directory_fd(path):
    """Walk an absolute local path without following symlinks, including ancestors."""
    path = Path(path)
    if not path.is_absolute() or ".." in path.parts:
        raise RuntimeFailure("unsafe_path")
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
    current = os.open("/", flags)
    try:
        for component in path.parts[1:]:
            following = os.open(component, flags, dir_fd=current)
            os.close(current)
            current = following
        return current
    except OSError:
        os.close(current)
        raise RuntimeFailure("unsafe_path") from None


def _read_pinned_at(directory, name, size, expected_sha256, error_class):
    """Read once under byte/type bounds; inference consumes these exact hashed bytes."""
    descriptor = None
    try:
        descriptor = os.open(name, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW,
                             dir_fd=directory)
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode) or before.st_size != size:
            raise RuntimeFailure(error_class)
        digest = hashlib.sha256()
        chunks = []
        total = 0
        while True:
            chunk = os.read(descriptor, min(131_072, size - total + 1))
            if not chunk:
                break
            total += len(chunk)
            if total > size:
                raise RuntimeFailure(error_class)
            digest.update(chunk)
            chunks.append(chunk)
        after = os.fstat(descriptor)
        def identity(value):
            return (value.st_dev, value.st_ino, value.st_size,
                    value.st_mtime_ns, value.st_ctime_ns)
        if (total != size or identity(before) != identity(after)
                or digest.hexdigest() != expected_sha256):
            raise RuntimeFailure(error_class)
        return b"".join(chunks)
    except OSError:
        raise RuntimeFailure(error_class) from None
    finally:
        if descriptor is not None:
            os.close(descriptor)


def _read_pinned_path(path, size, digest):
    directory = _directory_fd(Path(path).parent)
    try:
        return _read_pinned_at(directory, Path(path).name, size, digest,
                               "runtime_identity_mismatch")
    finally:
        os.close(directory)


def read_model_bundle(path):
    directory = _directory_fd(path)
    try:
        if set(os.listdir(directory)) != set(MODEL_FILES):
            raise RuntimeFailure("model_identity_mismatch")
        return {name: _read_pinned_at(directory, name, size, digest,
                                      "model_identity_mismatch")
                for name, (size, digest) in MODEL_FILES.items()}
    finally:
        os.close(directory)


def runtime_preflight():
    """Exact installed SDK/presets are checked before reading the larger model."""
    try:
        for package, version in DEPENDENCY_VERSIONS.items():
            if importlib.metadata.version(package) != version:
                raise RuntimeFailure("runtime_identity_mismatch")
        distribution = importlib.metadata.distribution("vieneu")
    except importlib.metadata.PackageNotFoundError:
        raise RuntimeFailure("missing_runtime") from None
    preset = None
    for name, (size, digest) in SDK_FILES.items():
        data = _read_pinned_path(distribution.locate_file(name), size, digest)
        if name == PRESET_PATH:
            preset = json.loads(data)
    return preset


def _offline_audit(event, arguments):
    """Additional Python API guard; trusted native dependencies are not an OS sandbox."""
    if event.startswith("socket.") or event in {
        "subprocess.Popen", "os.system", "os.exec", "os.spawn",
        "os.posix_spawn", "os.fork", "os.forkpty",
    }:
        raise RuntimeFailure("runtime_network_or_process_forbidden")
    if event in {"os.mkdir", "os.remove", "os.rename", "os.rmdir", "os.chmod",
                 "os.chown", "os.truncate", "os.utime", "os.link", "os.symlink"}:
        raise RuntimeFailure("runtime_write_forbidden")
    if event == "open":
        flags = arguments[2]
        if flags & (os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND):
            raise RuntimeFailure("runtime_write_forbidden")


def _bootstrap_audit(event, arguments):
    """Deny import probes with the error type their capability checks handle.

    Network and child processes remain fully denied. Filelock probes filesystem
    capabilities at import and handles OSError; PermissionError lets it conclude
    the capability is unavailable while the operation remains blocked. This hook
    stays installed, so later writes may raise PermissionError before the full
    guard runs. Neither error type authorizes a write or a synthesis fallback.
    """
    try:
        _offline_audit(event, arguments)
    except RuntimeFailure as error:
        if str(error) == "runtime_write_forbidden":
            raise PermissionError("runtime_write_forbidden") from None
        raise


def import_runtime_dependencies(model_directory):
    """Import the exact local dependencies before installing the inference guard."""
    # read_model_bundle has already verified this absolute directory. Cache only
    # its name: tempfile must not run its own write-based directory discovery.
    tempfile.tempdir = os.fspath(model_directory)
    sys.addaudithook(_bootstrap_audit)
    import numpy as np
    import onnxruntime  # Preload the native module; load_sampler reuses it.
    from vieneu.v3nano import OnnxV3NanoEngine  # Preload without constructing it.
    from vieneu_utils.phonemize_text import normalize_to_chunks_v3_with_gaps, phonemize_text
    from vieneu_utils.core_utils import join_audio_chunks, gaps_to_silence
    sys.addaudithook(_offline_audit)
    return np, normalize_to_chunks_v3_with_gaps, phonemize_text, join_audio_chunks, gaps_to_silence


@contextmanager
def quiet_runtime():
    """Discard even native diagnostics without keeping arbitrary log bodies in memory."""
    sys.stdout.flush()
    sys.stderr.flush()
    saved_stdout, saved_stderr = os.dup(1), os.dup(2)
    sink = os.open(os.devnull, os.O_WRONLY)
    try:
        os.dup2(sink, 1)
        os.dup2(sink, 2)
        yield
    finally:
        sys.stdout.flush()
        sys.stderr.flush()
        os.dup2(saved_stdout, 1)
        os.dup2(saved_stderr, 2)
        os.close(saved_stdout)
        os.close(saved_stderr)
        os.close(sink)


def checked_phonemes(text, normalize, phonemize, vocab):
    """Do not inherit upstream's warning-and-drop handling of unknown phonemes."""
    chunks, gaps = normalize(text, max_chars=140)
    if (not chunks or len(chunks) > MAX_CHUNKS or len(gaps) != len(chunks) - 1
            or any(gap not in {"para", "sentence", "minor"} for gap in gaps)):
        raise RuntimeFailure("unsupported_phonemes")
    phones = []
    for chunk in chunks:
        phone = phonemize(chunk)
        if (not isinstance(phone, str) or not phone.strip() or len(phone) > MAX_PHONE_CHARS
                or any(character not in vocab for character in phone)):
            raise RuntimeFailure("unsupported_phonemes")
        phones.append(phone)
    return phones, gaps


def load_sampler(bundle):
    """Initialize only the pinned sampler, from verified bytes, with CPU sessions."""
    import numpy as np
    import onnxruntime as ort
    from vieneu.v3nano import OnnxV3NanoEngine

    config = json.loads(bundle["config.json"])
    if (config["sample_rate"] != SAMPLE_RATE or config["flow_fps"] != 15.625
            or config["steps_default"] != 16 or config["cfg_default"] != 3.0):
        raise RuntimeFailure("model_identity_mismatch")
    # Bypass upstream's filename loader/download path. ONNX and NPZ consume the
    # same bytes that passed the checksum; no hash-to-reopen replacement race.
    engine = OnnxV3NanoEngine.__new__(OnnxV3NanoEngine)
    engine._lock = threading.RLock()
    engine.vocab = config["vocab"]
    engine.bos, engine.eos, engine.pad = config["bos_id"], config["eos_id"], config["pad_id"]
    engine.fps = config["flow_fps"]
    engine.emotion_map = {}  # Inline emotion/cloning controls are outside this contract.
    engine._oov_warned = set()
    with np.load(io.BytesIO(bundle["constants.npz"]), allow_pickle=False) as constants:
        engine.null_spk = constants["null_spk"].astype(np.float32)[None]
        engine.null_style = constants["null_style"].astype(np.float32)[None]
    options = ort.SessionOptions()
    options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    options.inter_op_num_threads = 1
    options.intra_op_num_threads = 2
    options.log_severity_level = 3
    options.add_session_config_entry("session.intra_op.allow_spinning", "0")
    for attribute, filename in (("s_text", "text_encoder.onnx"),
                                ("s_dur", "duration_predictor.onnx"),
                                ("s_ve", "vector_estimator.onnx"),
                                ("s_dec", "codec_decoder.onnx")):
        session = ort.InferenceSession(bundle[filename], options,
                                       providers=["CPUExecutionProvider"])
        if session.get_providers() != ["CPUExecutionProvider"]:
            raise RuntimeFailure("runtime_identity_mismatch")
        setattr(engine, attribute, session)
    null_ids = np.array([[engine.bos, engine.eos]], dtype=np.int64)
    engine._null_ctx = engine.s_text.run(None, {"ids": null_ids, "style": engine.null_style})[0]
    engine._null_mask = null_ids != engine.pad
    return engine


def checked_chunk_duration(engine, phone, speaker, style, numpy):
    """Reject the pinned sampler's 15-second clamp before speech synthesis.

    The SDK's infer predicts duration and clamps it before sampling. Evaluate those
    same two graphs first, at the fixed speed of 1.0. This extra prediction leaves
    the upstream sampling recipe intact and makes an overlong chunk an error.
    """
    with engine._lock:
        speaker_batch = numpy.asarray(speaker, dtype=numpy.float32).reshape(1, -1)
        style_array = numpy.asarray(style, dtype=numpy.float32)
        style_batch = style_array.reshape((1,) + style_array.shape[-2:])
        ids = engine.encode_phones(phone)
        context = engine.s_text.run(None, {"ids": ids, "style": style_batch})[0]
        prediction = engine.s_dur.run(None, {"ctx": context, "ctx_mask": ids != engine.pad,
                                            "spk": speaker_batch})
        try:
            log_seconds = float(prediction[0][0])
        except (TypeError, ValueError, OverflowError, IndexError):
            raise RuntimeFailure("unsupported_duration") from None
        if (not math.isfinite(log_seconds)
                or log_seconds > math.log(MAX_CHUNK_SECONDS)):
            raise RuntimeFailure("unsupported_duration")
        seconds = math.exp(log_seconds)
        if not 0.0 < seconds <= MAX_CHUNK_SECONDS:
            raise RuntimeFailure("unsupported_duration")
        return seconds


def encode_pcm_wav(samples):
    """Fixed mono s16le: ties-to-even round(x*32768), saturating positive 1.0.

    Reject nonfinite/out-of-range samples and empty or excessive output. Quantization
    is explicit instead of relying on the platform's sound-file default subtype.
    """
    if (getattr(samples, "ndim", 1) != 1 or not 0 < len(samples) <= MAX_FRAMES):
        raise RuntimeFailure("invalid_audio")
    pcm = array.array("h")
    for sample in samples:
        if isinstance(sample, bool):
            raise RuntimeFailure("invalid_audio")
        try:
            value = float(sample)
        except (TypeError, ValueError, OverflowError):
            raise RuntimeFailure("invalid_audio") from None
        if not math.isfinite(value) or not -1.0 <= value <= 1.0:
            raise RuntimeFailure("invalid_audio")
        pcm.append(max(-32_768, min(32_767, round(value * 32_768))))
    if sys.byteorder != "little":
        pcm.byteswap()
    output = io.BytesIO()
    with wave.open(output, "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(SAMPLE_RATE)
        wav.writeframes(pcm.tobytes())
    return output.getvalue()


def render(request, model_directory):
    preset_pack = runtime_preflight()
    bundle = read_model_bundle(model_directory)
    with quiet_runtime():
        logging.disable(logging.CRITICAL)
        # The fixed dependencies run locally. Import-time availability probes
        # remain denied; model sessions receive verified bytes directly.
        os.environ["HF_HUB_OFFLINE"] = "1"
        os.environ["TRANSFORMERS_OFFLINE"] = "1"
        os.environ["OMP_NUM_THREADS"] = "2"
        os.environ["OPENBLAS_NUM_THREADS"] = "2"
        os.environ["VECLIB_MAXIMUM_THREADS"] = "2"
        (np, normalize_to_chunks_v3_with_gaps, phonemize_text,
         join_audio_chunks, gaps_to_silence) = import_runtime_dependencies(model_directory)

        engine = load_sampler(bundle)
        preset = preset_pack["presets"][VOICE_NAMES[request["voice_id"]]]
        speaker = np.asarray(preset["speaker_emb"], dtype=np.float32)
        style = np.asarray(preset["style"], dtype=np.float32)
        if speaker.shape != (192,) or style.shape != (50, 256):
            raise RuntimeFailure("runtime_identity_mismatch")
        phones, gaps = checked_phonemes(request["text"], normalize_to_chunks_v3_with_gaps,
                                        phonemize_text, engine.vocab)
        # Check every chunk before generating any speech, including later chunks.
        for phone in phones:
            checked_chunk_duration(engine, phone, speaker, style, np)
        chunks = []
        total = 0
        for index, phone in enumerate(phones):
            chunk = engine.infer(phone, speaker, style, steps=16, cfg=3.0,
                                 sway=0.0, speed=1.0, seed=index)
            total += len(chunk)
            if total > MAX_FRAMES:
                raise RuntimeFailure("invalid_audio")
            chunks.append(chunk)
        samples = join_audio_chunks(chunks, SAMPLE_RATE, silence_ps=gaps_to_silence(gaps))
        return encode_pcm_wav(samples)


def main():
    try:
        if len(sys.argv) != 2:
            raise RuntimeFailure("invalid_request")
        request = parse_request(sys.stdin.buffer.read(MAX_REQUEST_BYTES + 1))
        output = render(request, sys.argv[1])
        sys.stdout.buffer.write(output)
        sys.stdout.buffer.flush()
        return 0
    except RuntimeFailure as error:
        code = str(error)
    except (Exception, KeyboardInterrupt):
        code = "runtime_failure"
    sys.stderr.write("vieneu_nano_runtime:" + code + "\n")
    return 2


if __name__ == "__main__":
    sys.exit(main())
