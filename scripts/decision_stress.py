#!/usr/bin/env python3
"""Prepare identical Claude Code and S1Code workspaces without running either."""

import hashlib
import json
import pathlib
import shutil
import tempfile


ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCE = ROOT / "fixtures" / "decision-stress"
POINTER = ROOT / "private" / "decision-stress.json"


def digest_tree(directory):
    digest = hashlib.sha256()
    for path in sorted(
        p
        for p in directory.rglob("*")
        if p.is_file() and "__pycache__" not in p.parts and p.suffix != ".pyc"
    ):
        relative = path.relative_to(directory).as_posix()
        digest.update(relative.encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def main():
    base = pathlib.Path(tempfile.mkdtemp(prefix="s1code-decision-stress-"))
    for arm in ("claude", "s1code"):
        shutil.copytree(
            SOURCE / "starter",
            base / arm,
            ignore=shutil.ignore_patterns("__pycache__", "*.pyc"),
        )
    prompt = (SOURCE / "prompt.txt").read_text()
    tree_hash = digest_tree(base / "claude")
    assert tree_hash == digest_tree(base / "s1code")
    record = {"directory": str(base), "starting_tree_sha256": tree_hash}
    POINTER.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    POINTER.write_text(json.dumps(record, indent=2) + "\n")
    POINTER.chmod(0o600)
    print(f"Claude Code workspace: {base / 'claude'}")
    print(f"S1Code workspace:      {base / 's1code'}")
    print(f"Identical tree SHA256: {tree_hash}")
    print("\nTerminal 1:")
    print(f"  cd {base / 'claude'} && claude --model opus")
    print("Terminal 2:")
    print(f"  cd {base / 's1code'} && s1code")
    print("\nUse Opus 5 in both terminals and paste this exact prompt:\n")
    print(prompt)
    print("The shared fixture starts with two failing assertions. Verify both outputs")
    print("afterward with: python3 -m unittest discover -s tests -v")


if __name__ == "__main__":
    main()
