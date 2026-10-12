#!/usr/bin/env python3
"""Exercise a built plugin archive against a Core with the plugin-install contract.

MEMCASTLE_BIN selects a released or locally built Core. Everything else lives in a
temporary XDG home; neither the user's palace nor their agent configuration is used.
"""

import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tarfile
import tempfile
import time
import tomllib
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen
from urllib.parse import quote


ROOT = Path(__file__).resolve().parents[1]


def request(base: str, route: str, *, method: str = "GET", data: bytes | None = None,
            headers: dict[str, str] | None = None):
    with urlopen(Request(base + route, data=data, method=method, headers=headers or {}), timeout=20) as response:
        return json.load(response)


def incompatible(archive: bytes) -> bytes:
    """Change only the parent Core bound to exercise the host's version gate."""
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as bundle:
        entries = [(info.name, bundle.extractfile(info).read()) for info in bundle if info.isfile()]
    output = io.BytesIO()
    with gzip.GzipFile(fileobj=output, mode="wb", filename="", mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w") as bundle:
            for name, data in entries:
                if name == "plugin.toml":
                    data = data.replace(b'memcastle = ">=0.4, <0.6"', b'memcastle = ">=99"', 1)
                info = tarfile.TarInfo(name)
                info.size = len(data)
                bundle.addfile(info, io.BytesIO(data))
    return output.getvalue()


def main() -> None:
    binary = Path(os.environ["MEMCASTLE_BIN"]).resolve()
    manifest = tomllib.loads((ROOT / "plugin.toml").read_text())
    plugin = manifest["plugin"]["id"]
    archive = (ROOT / "dist" / f"{plugin}-{manifest['plugin']['version']}.tar.gz").read_bytes()
    with tempfile.TemporaryDirectory(prefix="memcastle-plugin-smoke-") as scratch:
        root = Path(scratch)
        env = {key: value for key, value in os.environ.items() if not key.startswith("MEMCASTLE_")}
        (root / "assets").mkdir()
        registry = root / "plugins.json"
        (root / f"{plugin}-{manifest['plugin']['version']}.tar.gz").write_bytes(archive)
        registry.write_text(json.dumps({"format": 1, "name": "Pi smoke registry", "plugins": [{
            "id": "pi", "description": manifest["plugin"]["description"],
            "repository": "memcastle/memcastle-pi",
            "modules": [{"id": module["id"], "type": module["type"]} for module in manifest["modules"]],
            "versions": [{"version": manifest["plugin"]["version"],
                          "url": f"pi-{manifest['plugin']['version']}.tar.gz",
                          "sha256": hashlib.sha256(archive).hexdigest()}],
        }]}))
        config = root / "config.toml"
        config.write_text(f'[plugins]\nregistries = ["{registry}"]\n')
        env.update({"HOME": scratch, "XDG_DATA_HOME": str(root / "data"),
                    "XDG_CONFIG_HOME": str(root / "config"), "XDG_STATE_HOME": str(root / "state"),
                    "MEMCASTLE_CONFIG": str(config),
                    "MEMCASTLE_ASSETS_DIR": str(root / "assets")})
        # The local installer asks the agent its version; the stand-in never loads user plugins.
        agent = root / "bin"
        agent.mkdir()
        (root / "pi-packages").write_text("")
        (agent / "pi").write_text("""#!/bin/sh
case "$1" in
  --version) printf '1.18.34\\n' ;;
  install) printf '%s\\n' "$2" >> "$FAKE_AGENT_STATE" ;;
  list) printf 'User packages:\\n'; while IFS= read -r p; do printf '  %s\\n    %s\\n' "$p" "$p"; done < "$FAKE_AGENT_STATE" ;;
  remove) grep -vxF "$2" "$FAKE_AGENT_STATE" > "$FAKE_AGENT_STATE.tmp"; mv "$FAKE_AGENT_STATE.tmp" "$FAKE_AGENT_STATE" ;;
  *) exit 2 ;;
esac
""")
        (agent / "pi").chmod(0o755)
        env["PATH"] = str(agent) + os.pathsep + os.environ["PATH"]
        env["FAKE_AGENT_STATE"] = str(root / "pi-packages")
        with socket.socket() as probe:
            probe.bind(("127.0.0.1", 0))
            port = probe.getsockname()[1]
        base = f"http://127.0.0.1:{port}"
        process = subprocess.Popen([str(binary), "--palace", str(root / "palace"), "serve",
                                    "--bind", "127.0.0.1", "--port", str(port)],
                                   cwd=root, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        try:
            for _ in range(120):
                if process.poll() is not None:
                    raise RuntimeError(f"daemon exited during startup: {process.stderr.read().decode()}")
                try:
                    request(base, "/api/health")
                    break
                except (URLError, TimeoutError):
                    time.sleep(0.25)
            else:
                raise RuntimeError("temporary daemon did not become healthy within 30 seconds")
            preview = request(base, "/api/plugins/preview", method="POST", data=archive)
            assert preview["id"] == plugin
            expected = {(item["id"], item["type"]) for item in manifest.get("modules", [])}
            assert {(item["id"], item["type"]) for item in preview["modules"]} == expected
            discovered = request(base, "/api/plugin-registry/search?q=pi")
            assert discovered["plugins"][0]["id"] == "pi" and not discovered["warnings"], discovered
            indexed = request(base, "/api/plugin-registry/plugins/pi")
            assert indexed["archive_digest"] == hashlib.sha256(archive).hexdigest(), indexed
            assert {(item["id"], item["type"]) for item in indexed["plugin"]["modules"]} == expected
            assert not request(base, "/api/plugins")
            consents = {item["source"]: item["digest"] for item in preview["source_consents"]}
            installed = request(base, "/api/plugin-registry/install", method="POST",
                                headers={"Content-Type": "application/json"}, data=json.dumps({
                                    "id": "pi", "version": manifest["plugin"]["version"],
                                    "archive_digest": indexed["archive_digest"], "consents": consents,
                                }).encode())
            assert installed["id"] == plugin
            assert request(base, f"/api/plugins/{plugin}")["id"] == plugin
            for item in manifest.get("modules", []):
                if item["type"] == "source":
                    source = request(base, f"/api/source-packages/{item['id']}")
                    assert source["state"] == "installed", source
                    command = subprocess.run([str(binary), "--palace", str(root / "palace"),
                                              "source", "enable", item["id"]], env=env,
                                             capture_output=True, text=True)
                    assert command.returncode == 0, command.stderr
                elif item["type"] == "integration":
                    command = subprocess.run([str(binary), "--json", "integration", "install", item["id"]],
                                             env=env, capture_output=True, text=True)
                    assert command.returncode == 0, command.stderr
                    assert json.loads(command.stdout)["id"] == item["id"]
                    installed_copy = root / "data/memcastle/agents/pi"
                    agent_package = json.loads((installed_copy / "package.json").read_text())
                    assert agent_package["pi"]["extensions"] == ["./extension.js"]
                    assert (installed_copy / "extension.js").is_file()
                    assert (installed_copy / "skills/wake-up/SKILL.md").is_file()
                    assert str(installed_copy) in (root / "pi-packages").read_text().splitlines()
            # A copied Pi session must mine through the plugin's real WASM host, retain provenance, and not
            # acquire it a second time once the cursor has advanced.
            sessions = root / "sessions" / "--home-me-project--"
            sessions.mkdir(parents=True)
            name = "2026-07-14T14-27-12-546Z_019f6106.jsonl"
            shutil.copyfile(ROOT / "modules/pi/fixtures/pi-sessions/tree/--home-me-project--" / name, sessions / name)
            os.utime(sessions / name, (1000, 1000))
            def mine():
                job = request(base, "/api/jobs", method="POST", headers={"Content-Type": "application/json"},
                              data=json.dumps({"type": "mine", "source": "pi", "locator": str(sessions.parent),
                                               "requested_by": "test"}).encode())
                for _ in range(300):
                    result = request(base, f"/api/jobs/{job['id']}")
                    if result["status"] in ("completed", "failed", "cancelled"):
                        assert result["status"] == "completed", result
                        return result["result"]
                    time.sleep(0.1)
                raise AssertionError("Pi mining did not complete within 30 seconds")
            assert mine()["created"] == 1
            hits = request(base, "/api/search?q=" + quote("rotate signing keys"))
            assert len(hits) == 1 and hits[0]["source"]["origin"]["source"] == "pi", hits
            assert not request(base, "/api/search?q=" + quote("SECRET-TOOL-OUTPUT"))
            assert mine()["documents"] == 0
            if b'memcastle = ">=0.4, <0.6"' in archive:
                try:
                    request(base, "/api/plugins/preview", method="POST", data=incompatible(archive))
                except HTTPError as error:
                    assert error.code == 400, error.read()
                else:
                    raise AssertionError("an incompatible parent manifest was accepted")
        finally:
            process.terminate()
            try:
                process.communicate(timeout=20)
            except subprocess.TimeoutExpired:
                process.kill()
                process.communicate()
    print("Pi plugin discovery, install, mining, integration and version gate passed")


if __name__ == "__main__":
    main()
