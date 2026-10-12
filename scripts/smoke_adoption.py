#!/usr/bin/env python3
"""Prove a mined legacy Pi source survives its explicit handoff to the Pi plugin."""

import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time
from urllib.error import HTTPError, URLError

from smoke_install import ROOT, request


def main() -> None:
    binary = Path(os.environ["MEMCASTLE_BIN"]).resolve()
    plugin = (ROOT / "dist/pi-0.1.0.tar.gz").read_bytes()
    # A legacy package reproduces the old source's installation without building Core or touching a real palace.
    subprocess.run((str(binary), "source", "package", str(ROOT / "modules/pi"), "--no-build"),
                   check=True, capture_output=True)
    legacy = (ROOT / "modules/pi/dist/pi-0.1.0.tar.gz").read_bytes()
    with tempfile.TemporaryDirectory(prefix="memcastle-pi-adopt-") as scratch:
        root = Path(scratch)
        (root / "assets").mkdir()
        env = {key: value for key, value in os.environ.items() if not key.startswith("MEMCASTLE_")}
        env.update({"HOME": scratch, "XDG_DATA_HOME": str(root / "data"),
                    "XDG_CONFIG_HOME": str(root / "config"), "XDG_STATE_HOME": str(root / "state"),
                    "MEMCASTLE_ASSETS_DIR": str(root / "assets")})
        with socket.socket() as probe:
            probe.bind(("127.0.0.1", 0))
            port = probe.getsockname()[1]
        base = f"http://127.0.0.1:{port}"
        process = subprocess.Popen((str(binary), "--palace", str(root / "palace"), "serve",
                                    "--bind", "127.0.0.1", "--port", str(port)),
                                   cwd=root, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        try:
            for _ in range(120):
                if process.poll() is not None:
                    raise RuntimeError(f"daemon exited: {process.stderr.read().decode()}")
                try:
                    request(base, "/api/health")
                    break
                except (URLError, TimeoutError):
                    time.sleep(0.25)
            else:
                raise AssertionError("disposable daemon did not become healthy")
            try:
                request(base, "/api/source-packages", method="POST", data=legacy)
            except HTTPError as error:
                refusal = json.load(error)
                assert refusal["code"] == "memcastle::source::consent_required", refusal
                digest = refusal["help"].split("--consent ", 1)[1].split("`", 1)[0]
            else:
                raise AssertionError("legacy Pi source installed without permissions consent")
            installed = request(base, "/api/source-packages?consent=" + digest + "&enable=true",
                                method="POST", data=legacy)
            assert installed["source"]["name"] == "pi", installed
            session = root / "sessions" / "--home-me-project--"
            session.mkdir(parents=True)
            name = "2026-07-14T14-27-12-546Z_019f6106.jsonl"
            shutil.copyfile(ROOT / "modules/pi/fixtures/pi-sessions/tree/--home-me-project--" / name, session / name)
            os.utime(session / name, (1000, 1000))
            miner = json.dumps({"source": "pi", "locator": str(session.parent)}).encode()
            request(base, "/api/miners/pi", method="PUT", data=miner,
                    headers={"Content-Type": "application/json"})
            def mine():
                job = request(base, "/api/jobs", method="POST", data=json.dumps({
                    "type": "mine", "source": "pi", "locator": str(session.parent), "requested_by": "test",
                }).encode(), headers={"Content-Type": "application/json"})
                for _ in range(300):
                    result = request(base, f"/api/jobs/{job['id']}")
                    if result["status"] in ("completed", "failed", "cancelled"):
                        assert result["status"] == "completed", result
                        return result["result"]
                    time.sleep(0.1)
                raise AssertionError("Pi mining did not complete within 30 seconds")
            assert mine()["created"] == 1
            before = next(item for item in request(base, "/api/sources")["sources"] if item["source"] == "pi")
            request(base, "/api/source-packages/pi", method="DELETE")
            preview = request(base, "/api/plugins/preview", method="POST", data=plugin)
            assert preview["adoptions"] == ["pi"], preview
            consents = {item["source"]: item["digest"] for item in preview["source_consents"]}
            try:
                request(base, "/api/plugins", method="POST", data=plugin,
                        headers={"X-MemCastle-Consents": json.dumps(consents)})
            except HTTPError as error:
                assert error.code == 409, error.read()
            else:
                raise AssertionError("legacy mined data was adopted without explicit permission")
            request(base, "/api/plugins", method="POST", data=plugin, headers={
                "X-MemCastle-Consents": json.dumps(consents),
                "X-MemCastle-Adopt-Sources": json.dumps(["pi"]),
            })
            after = next(item for item in request(base, "/api/sources")["sources"] if item["source"] == "pi")
            assert before["cursor"] == after["cursor"] and before["documents"] == after["documents"]
            assert request(base, "/api/miners/pi")["source"] == "pi"
            assert request(base, "/api/source-packages/pi")["state"] == "installed"
            request(base, "/api/source-packages/pi/enable", method="POST")
            assert mine()["documents"] == 0, "adoption must not rewind the source cursor"
        finally:
            process.terminate()
            try:
                process.communicate(timeout=20)
            except subprocess.TimeoutExpired:
                process.kill()
                process.communicate()
    print("legacy Pi miner, cursor and mined data survived explicit plugin adoption")


if __name__ == "__main__":
    main()
