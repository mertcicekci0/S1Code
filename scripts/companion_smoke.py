#!/usr/bin/env python3
"""Offline binary-level MCP discovery; no keys, network or account changes."""
import json
import os
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parent.parent
BINARY = ROOT / 'target/release/s1code'
env = {key: value for key, value in os.environ.items()
       if key in ('PATH', 'HOME', 'TMPDIR', 'LANG')}
env['S1CODE_KEYCHAIN'] = 'off'
messages = [
    {'jsonrpc': '2.0', 'id': 1, 'method': 'initialize',
     'params': {'protocolVersion': '2025-06-18', 'capabilities': {},
                'clientInfo': {'name': 'offline-check', 'version': '1'}}},
    {'jsonrpc': '2.0', 'method': 'notifications/initialized'},
    {'jsonrpc': '2.0', 'id': 2, 'method': 'tools/list'},
    {'jsonrpc': '2.0', 'id': 3, 'method': 'tools/call',
     'params': {'name': 'decision_budget', 'arguments': {}}},
]
result = subprocess.run([str(BINARY), 'mcp', '--max-provider-requests', '1'],
                        input=''.join(json.dumps(m) + '\n' for m in messages),
                        text=True, capture_output=True, env=env, timeout=10, check=True)
rows = [json.loads(line) for line in result.stdout.splitlines()]
assert [r['id'] for r in rows] == [1, 2, 3]
assert rows[0]['result']['capabilities'] == {'tools': {}}
assert {t['name'] for t in rows[1]['result']['tools']} == {'rank_evidence', 'decision_budget'}
budget = json.loads(rows[2]['result']['content'][0]['text'])
assert budget['requests_used'] == 0 and budget['requests_remaining'] == 1
assert not result.stderr
missing = subprocess.run([str(BINARY), 'mcp'], text=True, capture_output=True,
                         env=env, timeout=10)
assert missing.returncode != 0 and '--max-provider-requests' in missing.stderr
print('PASS: MCP handshake, tool discovery, local budget, required cap, clean JSONL and EOF; no provider calls')
