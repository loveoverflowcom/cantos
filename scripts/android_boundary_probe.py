"""Explicit local negative graph probe, only invoked by cargo xtask theatre-host."""
import argparse
from pathlib import Path
import subprocess
import tempfile


def probe_script(variant, probe):
    if variant not in {"debug", "release"} or probe not in {"sandbox", "transitive"}:
        raise ValueError("unknown boundary probe")
    dependency = "app.project(':preview-sandbox')" if probe == "sandbox" else "'org.jetbrains.kotlin:kotlin-test-junit:2.2.20'"
    return ("gradle.projectsEvaluated {\n"
            "  def app = gradle.rootProject.project(':androidApp')\n"
            f"  app.dependencies.add('{variant}Implementation', {dependency})\n"
            "}\n")


def admitted_failure(variant, probe, code, output):
    if code == 0:
        return False
    if probe == "sandbox":
        return (f"Incomplete {variant} graph" in output or f"Forbidden production {variant} dependencies:" in output) and "preview-sandbox" in output
    # JUnit is transitive through kotlin-test-junit. Require BOTH the intended boundary failure
    # and the transitive component; a generic Gradle/tool/network failure never passes.
    return f"Forbidden production {variant} dependencies:" in output and "junit:junit:" in output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--variant", required=True)
    parser.add_argument("--probe", required=True)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("gradle", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    script = probe_script(args.variant, args.probe)
    base = args.gradle[1:] if args.gradle[:1] == ["--"] else args.gradle
    if not base or Path(base[0]).name != "gradle":
        raise ValueError("xtask must supply the existing Gradle command")
    with tempfile.TemporaryDirectory(prefix="cantos-boundary-probe-") as temporary:
        init = Path(temporary) / "probe.gradle"
        init.write_text(script)
        capital = args.variant.capitalize()
        command = base + ["--offline", "--init-script", str(init), f":androidApp:assert{capital}DependencyBoundary"]
        result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=180)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(result.stdout)
    if not admitted_failure(args.variant, args.probe, result.returncode, result.stdout):
        raise ValueError("negative graph probe did not fail at the intended boundary; inspect " + str(args.report))
    print(f"PASS {args.variant} {args.probe}: injected dependency rejected by resolved graph guard (no APK built)")


if __name__ == "__main__":
    main()
