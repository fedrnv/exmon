"""Manual hosted qualification. No physical interfaces, releases or signing."""
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "target/platform-qualification"
OUT.mkdir(parents=True, exist_ok=True)
FIXTURE = ROOT / "Tasks/items/quality/EXM-035-validate-windows-macos-and-linux-behavior/evidence/20261126T095000Z-windows-profile.json"
EXPECTED_HASH = "8f8b9c51a2a0ae2ca05c9d350ef24ba6fb126beba2e00c980ac1537edc6950db"
RESULTS = []
DEADLINE = time.monotonic() + 2400
ENV = os.environ.copy()
ENV["CARGO_TARGET_DIR"] = str(ROOT / "target")
SMOKES = {
    "native_shell_smoke": "exm025-native.png",
    "native_incoming_smoke": "exm026-native.png",
    "native_outgoing_smoke": "exm027-native.png",
    "native_charts_smoke": "exm028-native.png",
    "native_settings_smoke": "exm029-native.png",
    "native_inspection_smoke": "exm030-native.png",
    "native_sessions_smoke": "exm032-native.png",
    "native_hid_smoke": "exm014-native.png",
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tail(path):
    with path.open("rb") as stream:
        stream.seek(0, os.SEEK_END)
        stream.seek(max(0, stream.tell() - 65536))
        return "\n".join(stream.read().decode(errors="replace").splitlines()[-60:])


def save():
    (OUT / "results.json").write_text(json.dumps(RESULTS, indent=2) + "\n")


def blocked(name):
    RESULTS.append({"name": name, "status": "not-run", "reason": "workspace build failed"})
    save()


def graphics_inventory():
    # Retain model/version fields only, never device UUIDs or serial numbers.
    try:
        if sys.platform == "linux":
            probe = subprocess.run(["vulkaninfo", "--summary"], capture_output=True,
                                   text=True, timeout=30)
            keys = {"apiVersion", "driverVersion", "vendorID", "deviceType",
                    "deviceName", "driverName", "driverInfo"}
            fields = [line.strip() for line in probe.stdout.splitlines()
                      if line.partition("=")[0].strip() in keys]
            return {"exit_code": probe.returncode, "fields": fields}
        probe = subprocess.run(["system_profiler", "SPDisplaysDataType", "-json"],
                               capture_output=True, text=True, timeout=30)
        data = json.loads(probe.stdout) if probe.returncode == 0 else {}
        keys = {"_name", "sppci_model", "spdisplays_vendor", "spdisplays_metalfamily"}
        return {"exit_code": probe.returncode,
                "devices": [{key: value for key, value in device.items() if key in keys}
                            for device in data.get("SPDisplaysDataType", [])]}
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        return {"status": "unavailable", "error_type": type(error).__name__}


def stop(process):
    if process.poll() is None:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            return
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=5)


def run(name, args, timeout):
    remaining = DEADLINE - time.monotonic()
    record = {"name": name, "command": args, "status": "not-run"}
    RESULTS.append(record)
    if remaining <= 0:
        record["reason"] = "overall qualification budget exhausted"
        save()
        return False
    started = time.monotonic()
    print(f"RUN {name}: {args}", flush=True)
    with (OUT / f"{name}.log").open("w") as log:
        try:
            process = subprocess.Popen(args, cwd=ROOT, env=ENV, stdout=log,
                                       stderr=subprocess.STDOUT, start_new_session=True)
            try:
                code = process.wait(timeout=min(timeout, remaining))
                record.update(status="passed" if code == 0 else "failed", exit_code=code)
            except subprocess.TimeoutExpired:
                stop(process)
                record["status"] = "timed-out"
        except OSError as error:
            record.update(status="unavailable", reason=str(error))
    record["seconds"] = round(time.monotonic() - started, 3)
    save()
    print(f"RESULT {name}: {record['status']}", flush=True)
    if record["status"] != "passed":
        print(tail(OUT / f"{name}.log"))
    return record["status"] == "passed"


def startup_probe(executable):
    record = {"name": "installed-startup", "status": "not-run",
              "scope": "alive after eight seconds; termination is fixture cleanup, not normal close"}
    RESULTS.append(record)
    if not executable.is_file() or time.monotonic() + 10 >= DEADLINE:
        record["reason"] = "installed executable or remaining budget unavailable"
        save()
        return
    record["sha256"] = digest(executable)
    with (OUT / "installed-startup.log").open("w") as log:
        process = subprocess.Popen([str(executable)], cwd=ROOT, env=ENV,
                                   stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            try:
                code = process.wait(timeout=8)
                record.update(status="failed", exit_code=code, reason="exited before startup probe")
            except subprocess.TimeoutExpired:
                record["status"] = "passed"
        finally:
            stop(process)
    save()


def main():
    os.chdir(ROOT)
    if sys.platform not in ("linux", "darwin"):
        raise RuntimeError("this supervisor supports only its declared Linux/macOS jobs")
    machine = platform.machine()
    expected = os.environ["EXMON_EXPECTED_MACHINE"]
    if machine != expected:
        raise RuntimeError(f"runner architecture {machine} differs from declared {expected}")
    if digest(FIXTURE) != EXPECTED_HASH:
        raise RuntimeError("Windows-exported fixture bytes changed")
    metadata = {
        "source": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "system": platform.system(), "release": platform.release(), "machine": machine,
        "runner_os": os.environ.get("RUNNER_OS"), "runner_arch": os.environ.get("RUNNER_ARCH"),
        "runner_image": os.environ.get("ImageOS"), "runner_image_version": os.environ.get("ImageVersion"),
        "cargo_lock_sha256": digest(ROOT / "Cargo.lock"),
        "policy_sha256": digest(ROOT / "Tasks/tasks_file_structure_v5_7.md"),
        "profile": "default debug assertions/optimization; debug symbols omitted and incremental disabled to bound disk",
        "backend": ENV.get("WGPU_BACKEND"), "fixture_sha256": EXPECTED_HASH,
        "graphics": graphics_inventory(),
    }
    (OUT / "environment.json").write_text(json.dumps(metadata, indent=2) + "\n")
    cargo = ["cargo", "+1.98.0"]
    run("toolchain", ["rustc", "+1.98.0", "-vV"], 30)
    built = run("build", cargo + ["build", "--workspace", "--locked"], 1200)
    if built:
        run("workspace-tests", cargo + ["test", "--workspace", "--locked", "--", "--test-threads=1"], 900)
    else:
        blocked("workspace-tests")
    run("format", cargo + ["fmt", "--all", "--", "--check"], 120)
    if built:
        run("lint", cargo + ["clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"], 600)
    else:
        blocked("lint")
    run("architecture", ["node", "scripts/check-architecture.mjs"], 60)
    run("native-feature-build", cargo + ["check", "-p", "exmon-capture", "--all-features", "--locked"], 300)
    if built:
        run("hid-production-ui", cargo + ["test", "-p", "exmon-gui", "--features", "native-hid", "--test", "hid", "--locked", "--", "--test-threads=1"], 300)
    else:
        blocked("hid-production-ui")
    exported = OUT / "imported-profile.json"
    if run("portable-profile", cargo + ["run", "-p", "exmon-app", "--example", "portable_profile_smoke", "--locked", "--", "import", str(FIXTURE), str(exported)], 180):
        RESULTS.append({"name": "portable-exact-bytes", "status": "passed" if digest(exported) == EXPECTED_HASH else "failed", "sha256": digest(exported)})
        save()
    for example in SMOKES:
        if built:
            run(example, cargo + ["run", "-p", "exmon-gui", "--example", example, "--locked"], 180)
        else:
            blocked(example)
    install_root = Path(os.environ["RUNNER_TEMP"]) / "exmon-developer-install"
    if built and run("developer-install", cargo + ["install", "--path", "crates/gui", "--debug", "--locked", "--root", str(install_root)], 300):
        startup_probe(install_root / "bin/exmon-gui")
        run("developer-uninstall", cargo + ["uninstall", "--root", str(install_root), "exmon-gui"], 60)
        RESULTS.append({"name": "developer-uninstall-files", "status": "failed" if (install_root / "bin/exmon-gui").exists() else "passed"})
    elif not built:
        blocked("developer-install")
    images = []
    for filename in SMOKES.values():
        source = ROOT / "target" / filename
        if source.is_file() and source.stat().st_size <= 4 * 1024 * 1024:
            destination = OUT / filename
            shutil.copyfile(source, destination)
            images.append({"file": filename, "sha256": digest(destination)})
    (OUT / "images.json").write_text(json.dumps(images, indent=2) + "\n")
    save()
    failed = [r["name"] for r in RESULTS if r["status"] != "passed"]
    print("QUALIFICATION_FAILED " + json.dumps(failed) if failed else "QUALIFICATION_PASSED", flush=True)
    return bool(failed)


if __name__ == "__main__":
    raise SystemExit(main())
