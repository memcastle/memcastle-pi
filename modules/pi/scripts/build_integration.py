#!/usr/bin/env python3
"""Build the Pi extension with its MCP dependency bundled, as the old Core release did."""

import json
from pathlib import Path
import subprocess
import tomllib

root = Path(__file__).resolve().parents[1]
subprocess.run(("bun", "install", "--frozen-lockfile"), cwd=root, check=True)
subprocess.run((
    "bun", "build", "src/extension.ts", "--outdir", "dist", "--target=node",
    "--external", "@earendil-works/pi-coding-agent",
    "--external", "@earendil-works/pi-ai",
    "--external", "@earendil-works/pi-agent-core",
    "--external", "@earendil-works/pi-tui",
), cwd=root, check=True)
with (root / "memcastle-integration.toml").open("rb") as source:
    version = tomllib.load(source)["integration"]["version"]
(root / "dist/package.json").write_text(json.dumps({
    "name": "memcastle-pi",
    "version": version,
    "private": True,
    "type": "module",
    "license": "MIT",
    "pi": {"extensions": ["./extension.js"]},
}, indent=2) + "\n")
