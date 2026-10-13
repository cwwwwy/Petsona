#!/usr/bin/env python3
"""Run isolated macOS native pet, composer, lifecycle and protocol checks."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import socket
import subprocess
import time
import urllib.error
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parent.parent


def request(port, path, payload=None):
    data = json.dumps(payload).encode() if payload is not None else None
    query = urllib.request.Request(f"http://127.0.0.1:{port}/{path}", data=data, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(query, timeout=1) as response:
        return json.load(response)


def prepare(base, port, populated):
    base.mkdir(parents=True)
    home = base / "home"
    home.mkdir()
    if populated:
        (home / "macos-window-position.json").write_text(json.dumps({"x": 200, "y": 300, "backingScale": 1, "displayId": "missing-test-display"}))
        shutil.copytree(ROOT / "crates/petsona-core/testdata/v2-test-pet", home / "pets/test_fixture_v2")
    config = {
        "activePet": "test_fixture_v2" if populated else None,
        "stateServer": {"enabled": True, "port": port},
        "greeting": {"enabled": False},
        "window": {"scale": 1.0},
        "deepseek": {"provider": "custom", "baseUrl": "http://127.0.0.1:1/v1", "model": "acceptance-only", "apiKeyEnv": "PETSONA_ACCEPT_KEY"},
    }
    (home / "config.json").write_text(json.dumps(config), encoding="utf-8")
    env = os.environ.copy()
    env.update({
        "PETSONA_HOME": str(home),
        "PETSONA_CREDENTIAL_SERVICE": "com.petsona.accept." + base.parent.name,
        "PETSONA_AUTOSTART_DIR": str(base / "LaunchAgents"),
        "PETSONA_AUTOSTART_LABEL": "com.petsona.accept." + base.parent.name,
        "PETSONA_SHELL_LOG": str(base / "shell.log"),
        "RUST_LOG": "info",
        "PETSONA_OVERLAY_DIAGNOSTICS": str(base / "overlay.json"),
        "PETSONA_OVERLAY_SHOT": str(base / "pet.png"),
        "PETSONA_COMPOSER_SHOT": str(base / "composer.png"),
    })
    # A fake key and local-only endpoint keep automatic checks offline.
    env["PETSONA_ACCEPT_KEY"] = "fake-local-acceptance-key"
    return env


def wait_health(process, port):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if process.poll() is not None:
            error = process.stderr.read().decode(errors="replace") if process.stderr else ""
            raise RuntimeError(f"app exited before health: {process.returncode} {error}")
        try:
            return request(port, "health")
        except (OSError, urllib.error.URLError):
            time.sleep(0.1)
    raise TimeoutError("isolated state server did not start")


def wait_overlay(base, condition, process, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            result = json.loads((base / "overlay.json").read_text())
            if condition(result):
                return result
        except (OSError, ValueError):
            pass
        if process.poll() is not None:
            raise RuntimeError(f"app ended before native pet diagnostics: {process.returncode}")
        time.sleep(0.05)
    raise TimeoutError(f"native pet check timed out; see {base}")


def wait_state(port, expected, negate=False):
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline:
        health = request(port, "health")
        if (health["state"] == expected) != negate:
            return health
        time.sleep(0.05)
    raise TimeoutError(f"state did not reach {expected}, negate={negate}: {health}")


def verify_protocol(port):
    source = "mac-protocol-accept"
    request(port, "state", {"source": source, "state": "waiting", "ttlMs": 350})
    wait_state(port, "waiting")
    expired = wait_state(port, "waiting", negate=True)
    request(port, "state", {"source": source, "state": "running", "ttlMs": 0})
    wait_state(port, "running")
    time.sleep(0.5)
    assert request(port, "health")["state"] == "running", "sticky state expired"
    try:
        request(port, "state", {"source": source, "state": "nope"})
    except urllib.error.HTTPError as error:
        assert error.code == 400, error.code
    else:
        raise AssertionError("invalid state accepted")
    assert request(port, "health")["state"] == "running", "invalid state mutated state"
    request(port, "state", {"source": source, "action": "CLEAR"})
    cleared = wait_state(port, "running", negate=True)
    return {"ttlExpiry": True, "sticky": True, "invalid400": True, "clear": True, "expiredState": expired["state"], "clearedState": cleared["state"]}


def verify(app, base, port, populated):
    env = prepare(base, port, populated)
    args = [str(app), "--exit-after-ms", "14000" if populated else "3000"]
    if populated:
        args.extend(["--verify-content-lifecycle", "--verify-pet-layout", "--verify-composer"])
    with (base / "process.log").open("w") as log:
        process = subprocess.Popen(args, cwd=ROOT, env=env, stdout=log, stderr=log)
        try:
            health = wait_health(process, port)
            pets = request(port, "pets")
            overlay = None
            protocol = None
            if populated:
                assert "test_fixture_v2" in json.dumps(pets), pets
                protocol = verify_protocol(port)
                request(port, "state", {"source": "mac-accept", "state": "running", "message": "原生玻璃气泡验收", "ttlMs": 0})
                overlay = wait_overlay(base, lambda data: data["visible"] and data["distinctFrames"] >= 2 and data.get("captureAlpha"), process)
                bubble = wait_overlay(base, lambda data: data.get("bubble", {}).get("visible") and data["bubble"]["alpha"] > 0.95, process)
                assert not bubble["bubble"]["key"] and 0 < bubble["bubble"]["progress"] <= 1
                assert not overlay["key"] and not overlay["opaque"] and not overlay["shadow"], overlay
                assert overlay["level"] == 3, overlay
                assert overlay["captureAlpha"][0] > 0 and overlay["captureAlpha"][1] > 0, overlay
                assert 0 < overlay["idleOpaque"] < 192 * 208, overlay
                # Before the runtime-only layout hook starts, a missing display
                # must fall back to a visible one and decode saved density.
                for axis, size in [("x", "width"), ("y", "height")]:
                    start = {"x": 200, "y": 300}[axis] * overlay["backing"]
                    expected = max(overlay["work"][axis], min(start, overlay["work"][axis] + overlay["work"][size] - overlay["physical"][size]))
                    assert abs(overlay["physical"][axis] - expected) < 1, overlay
                with (base / "second.log").open("w") as second_log:
                    second_env = env.copy()
                    second_env["PETSONA_SHELL_LOG"] = str(base / "second-shell.log")
                    second_env["PETSONA_OVERLAY_DIAGNOSTICS"] = str(base / "second-overlay.json")
                    second_env.pop("PETSONA_OVERLAY_SHOT", None)
                    second = subprocess.Popen([str(app)], cwd=ROOT, env=second_env, stdout=second_log, stderr=second_log)
                    try:
                        assert second.wait(timeout=10) == 0
                    finally:
                        if second.poll() is None:
                            second.kill(); second.wait()
                assert "native fault notice locked=true" in (base / "second-shell.log").read_text()
                assert process.poll() is None and request(port, "health")["ok"]
            else:
                assert not health.get("pet"), health
            if populated:
                # Runtime chooses the message duration. Wait for actual expiry
                # rather than assuming a fixed four-second wall-clock delay.
                expired = wait_overlay(base, lambda data: not data["bubble"]["visible"], process)
                assert not expired["bubble"]["visible"], expired
            code = process.wait(timeout=20)
            assert code == 0, code
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    process.kill(); process.wait()
    shell = (base / "shell.log").read_text()
    if populated:
        assert "macos: lifecycle PASS" in shell and "macos: pet layout PASS" in shell, shell
        assert "pass=false" not in shell, shell
        saved = json.loads((base / "home/macos-window-position.json").read_text())
        # Restart against the saved sidecar after the first process exits.
        (base / "overlay.json").unlink()
        with (base / "restart.log").open("w") as log:
            restarted = subprocess.Popen([str(app), "--exit-after-ms", "3000"], cwd=ROOT, env=env, stdout=log, stderr=log)
            try:
                restored = wait_overlay(base, lambda data: data["visible"] and data["kind"] == "pet", restarted)
                assert restored["displayId"] == saved["displayId"]
                for axis in ["x", "y"]:
                    assert abs(restored["physical"][axis] - saved[axis]) < 1, (restored, saved)
                assert restarted.wait(timeout=10) == 0
            finally:
                if restarted.poll() is None:
                    restarted.kill(); restarted.wait()
    else:
        assert "watcher: no pet loaded" in shell and "macos: content shown=settings" in shell, shell
        empty = json.loads((base / "overlay.json").read_text())
        assert not empty["visible"], empty
    with socket.socket() as probe:
        assert probe.connect_ex(("127.0.0.1", port)) != 0, "port retained after exit"
    assert not (base / "LaunchAgents").exists(), "automated app touched autostart"
    return {"composerFocus": "PASS" if "macos: composer verify PASS" in shell else ("FAIL" if populated else "N/A"), "protocol": protocol, "home": env["PETSONA_HOME"], "launcherPid": process.pid, "exitCode": code, "health": health, "pets": pets, "overlay": overlay, "lifecycle": "PASS" if populated else "empty-start PASS", "portReleased": True, "positionRestart": populated, "singleInstance": populated}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, default=ROOT / ".scratch/macos/debug/Petsona.app/Contents/MacOS/petsona-desktop")
    parser.add_argument("--port", type=int, default=17943)
    args = parser.parse_args()
    if platform.system() != "Darwin":
        parser.error("macOS required")
    args.app = args.app.resolve()
    if not args.app.is_file():
        parser.error("build the Mac acceptance app first")
    if not 1024 <= args.port <= 65535 or args.port == 17872:
        parser.error("use a dedicated non-default port")
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", args.port))  # Refuse to collide with another app.
    base = ROOT / ".scratch/macos/accept" / uuid.uuid4().hex
    # Pin evidence to the exact binary and read-only source state before launch.
    source = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True)
    changes = subprocess.run(["git", "status", "--porcelain"], cwd=ROOT, capture_output=True, text=True)
    identity = {"sha256": hashlib.sha256(args.app.read_bytes()).hexdigest(),
                "sourceCommit": source.stdout.strip() if source.returncode == 0 else None,
                "sourceChanges": changes.stdout.splitlines() if changes.returncode == 0 else None,
                "osVersion": platform.mac_ver()[0], "port": args.port,
                "startedUtc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}
    results = []
    failures = []
    for populated, name in [(True, "populated"), (False, "empty")]:
        try:
            result = verify(args.app, base / name, args.port, populated)
            results.append(result)
            if result["composerFocus"] == "FAIL":
                failures.append({"case": name, "check": "composerFocus", "error": "native key focus verification failed; see shell.log"})
        except Exception as error:
            failures.append({"case": name, "error": str(error)})
    report = {"status": "FAIL" if failures else "PASS", "binary": identity, "architecture": platform.machine(), "cwd": str(ROOT), "app": str(args.app), "results": results, "failures": failures, "pending": ["true login autostart", "macOS26/Intel/mixed displays", "Windows shared regression"]}
    (base / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2))
    print(f"{report['status']} macOS native regression: pet, lifecycle, protocol, position restart, empty start and composer focus. No input injection.")
    print(f"Evidence: {base / 'report.json'}")
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
