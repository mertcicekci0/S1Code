#!/usr/bin/env python3
"""Prepare matched local workspaces for inspecting bounded decision behavior.

This script makes no model request, starts no agent, and records no benchmark.
"""

import json
import pathlib
import shutil
import tempfile


ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCE = ROOT / "fixtures" / "jev-showcase"
POINTER = ROOT / "private" / "jev-showcase.json"


def main():
    base = pathlib.Path(tempfile.mkdtemp(prefix="s1code-jev-showcase-"))
    for policy in ("rules", "jev"):
        shutil.copytree(SOURCE / "starter", base / policy)
    prompt = (SOURCE / "prompt.txt").read_text()
    POINTER.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    POINTER.write_text(json.dumps({"directory": str(base)}, indent=2) + "\n")
    POINTER.chmod(0o600)
    print(f"Prepared matched workspaces in {base}")
    print(f"Rules workspace: {base / 'rules'}")
    print(f"Jev workspace:   {base / 'jev'}")
    print("\nPaste this same prompt into each session:\n")
    print(prompt)
    print("Run the initial test yourself to confirm the shared failure:")
    print(f"  cd {base / 'rules'} && python3 -m unittest discover -s tests -v")


if __name__ == "__main__":
    main()
