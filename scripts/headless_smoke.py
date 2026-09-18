#!/usr/bin/env python3
"""Exercise durable CLI approval/resume and export using the labeled offline fixture."""
import json
import pathlib
import subprocess
import tempfile

binary = pathlib.Path("target/debug/s1code").resolve()
with tempfile.TemporaryDirectory(prefix="s1code-headless-") as directory:
    base = pathlib.Path(directory)
    home, workspace = base / "data", base / "repo"

    def command(*args):
        result = subprocess.run(
            [str(binary), "--home", str(home), *map(str, args)],
            capture_output=True, text=True, timeout=30, check=True,
        )
        return [json.loads(line) for line in result.stdout.splitlines() if line.strip()]

    events = command("demo", "--offline", "--workspace", workspace, "--headless")
    checkpoint = next((home / "sessions").glob("*/checkpoint.json"))
    approvals = 0
    while True:
        state = json.loads(checkpoint.read_text())
        if state["status"] == "completed":
            break
        assert state["status"] == "awaiting_approval", state["status"]
        assert approvals < 3, "unexpected additional action"
        approvals += 1
        events += command("resume", state["id"], "--headless", "--approve", state["pending"]["id"])
    assert approvals == 3
    assert state["verified"]["exit_code"] == 0
    assert state["metrics"]["generative_calls"] == 0
    assert any(e["kind"] == "tool_result" and e["data"].get("exit_code") == 1 for e in events)
    assert "int(text.strip())" in (workspace / "parser.py").read_text()
    exported = base / "export.jsonl"
    command("export", state["id"], exported)
    text = exported.read_text()
    assert str(base) not in text
    records = [json.loads(line) for line in text.splitlines()]
    assert records and all("REPLAY" in r["playback"] for r in records)
    assert all("OFFLINE SIMULATION" in r["event"]["data"]["execution_label"] for r in records)
    replay = command("replay", exported)
    assert len(replay) == len(records)
    print(json.dumps({"mode": "OFFLINE SIMULATION", "real_tools_and_resume": "passed", "approval_processes": approvals, "verification_exit": 0, "sanitized_replay": "passed", "model_inference": False}))
