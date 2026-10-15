"""Opt-in comparison-backend experiment, invoked only by cargo xtask visual-compare."""

import json
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import time

from visual_compare import ROOT, changed_mask, digest, fresh_output, pillow, read_png


def main():
    output = None
    try:
        Image, ImageChops, _ = pillow()
        spike = ROOT / "tools/visual-compare/spike"
        for name, version in {"looks-same": "10.0.1", "pixelmatch": "8.0.0",
                              "pngjs": "7.0.0", "odiff-bin": "4.5.0"}.items():
            installed = json.loads((spike / "node_modules" / name / "package.json").read_text())
            if installed.get("version") != version:
                raise ImportError(f"{name} {version} required; rerun explicit isolated npm setup")
        manifest = Path(sys.argv[1]).resolve()
        pairs = json.loads(manifest.read_text(encoding="utf-8"))
        if not isinstance(pairs, list) or not 1 <= len(pairs) <= 20:
            raise ValueError("spike manifest must contain 1–20 PNG pairs")
        for pair in pairs:
            if not isinstance(pair, dict) or not all(isinstance(pair.get(k), str)
                                                   for k in ("id", "reference", "actual")):
                raise ValueError("each spike pair needs id, reference and actual paths")
            for side in ("reference", "actual"):
                pair[side] = str((manifest.parent / pair[side]).resolve())
        output = fresh_output(None, "visual-spike")
        resolved = output / "pairs.json"
        resolved.write_text(json.dumps(pairs, indent=2) + "\n", encoding="utf-8")
        records = []
        for pair in pairs:
            times = []
            for trial in range(6):
                started = time.perf_counter()
                reference_bytes, a = read_png(pair["reference"], Image)
                actual_bytes, b = read_png(pair["actual"], Image)
                if a.size != b.size:
                    raise ValueError("spike pairs must have matching dimensions")
                _, changed = changed_mask(a, b, 8, ImageChops)
                Image.frombytes("L", a.size, changed).save(output / "pillow-mask.png")
                if trial:
                    times.append((time.perf_counter() - started) * 1000)
            records.append(dict(id=pair["id"], name="Pillow", diff_pixels=changed.count(255),
                                median_ms=round(statistics.median(times), 2),
                                reference_sha256=digest(reference_bytes), actual_sha256=digest(actual_bytes)))
        candidate = ROOT / "tools/visual-compare/spike/benchmark.mjs"
        subprocess.run(["node", str(candidate), str(resolved), str(output)], check=True, timeout=120)
        records += json.loads((output / "node-results.json").read_text())
        report = dict(records=records, machine=platform.platform(), python=platform.python_version(),
                      node=subprocess.check_output(["node", "--version"], text=True).strip(),
                      method="one warm-up, five samples; median decode+compare+mask/diff encode ms; "
                             "not full MVP, perceptual counts are not interchangeable",
                      parameters={"Pillow": {"version": "11.3.0", "max_channel_threshold": 8},
                                  "looks-same": {"version": "10.0.1", "tolerance": 2.3,
                                                 "ignoreCaret": False, "ignoreAntialiasing": True},
                                  "pixelmatch": {"version": "8.0.0", "threshold": .1, "includeAA": False},
                                  "odiff": {"version": "4.5.0", "threshold": .1, "antialiasing": True}},
                      source_sha256=digest(Path(__file__).read_bytes()),
                      npm_lock_sha256=digest((candidate.parent / "package-lock.json").read_bytes()))
        (output / "benchmark.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(f"Benchmark: {output / 'benchmark.json'}")
        return 0
    except (ImportError, FileNotFoundError) as error:
        print(f"UNSUPPORTED: optional spike tools missing: {error}; no install attempted", file=sys.stderr)
        return 3
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Spike failed: {error}; partial artifacts: {output}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
