#!/usr/bin/env python3
"""Conservative local source/history publication check. Never print matched bytes.

This is a guard, not a complete secret detector. Does not read credential stores,
publish anything, rewrite history or include ignored files in a release.
"""
import json
import pathlib
import re
import subprocess
import sys

RULES = {
    'provider credential': rb'(?:sk-ant-api[0-9]+-[A-Za-z0-9_-]{50,}|sk-or-v1-[a-f0-9]{64}|apikey_[A-Za-z0-9]{20,}|sk-(?:proj-)?[A-Za-z0-9_-]{48,})',
    'GitHub credential': rb'(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{50,})',
    'private key': rb'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----',
    'personal filesystem path': rb'/(?:Users|home)/[A-Za-z0-9_.-]+/',
}
PRIVATE_NAMES = {'.DS_Store', '.env', 'credentials.json', 'auth.json'}
PRIVATE_ROOTS = {'target', 'private', 'eval-results', '.s1code', '.nerve'}


def findings(data):
    return [name for name, pattern in RULES.items() if re.search(pattern, data)]


def git(*args):
    return subprocess.check_output(['git', *args])


def main():
    # Test with assembled dummy material, so no realistic key is committed.
    assert findings(b'sk-or-v1-' + b'a' * 64) == ['provider credential']
    assert findings(b'sk-ant-api03-' + b'a' * 90) == ['provider credential']
    issues = []
    paths = git('ls-files', '--cached', '--others', '--exclude-standard', '-z').split(b'\0')
    for raw in set(paths) - {b''}:
        path = pathlib.Path(raw.decode())
        if path.parts[0] in PRIVATE_ROOTS or path.name in PRIVATE_NAMES or path.name.startswith('.env.'):
            issues.append({'location': str(path), 'rule': 'private filename'})
        if path.is_symlink():
            issues.append({'location': str(path), 'rule': 'source symlink requires manual review'})
        elif path.is_file():
            issues.extend({'location': str(path), 'rule': rule} for rule in findings(path.read_bytes()))
    objects = git('rev-list', '--objects', '--all').splitlines()
    # Batch object reads avoid spawning one git process per historical blob.
    request = b''.join(entry.split(b' ', 1)[0] + b'\n' for entry in objects)
    output = subprocess.check_output(['git', 'cat-file', '--batch'], input=request)
    offset = 0
    for entry in objects:
        end = output.index(b'\n', offset)
        oid, kind, size = output[offset:end].split()
        start = end + 1
        data = output[start:start + int(size)]
        offset = start + int(size) + 1
        if kind in (b'blob', b'commit'):
            issues.extend({'location': 'git-object:' + oid[:12].decode(), 'rule': rule} for rule in findings(data))
    print(json.dumps({'check': 'source and reachable history', 'objects_scanned': len(objects),
                      'findings': issues, 'limitation': 'Pattern checks are not proof of absence; review release contents.'}, indent=2))
    return 1 if issues else 0


if __name__ == '__main__':
    sys.exit(main())
