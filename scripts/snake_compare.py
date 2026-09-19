#!/usr/bin/env python3
"""Prepare identical Snake projects and send the same single prompt to either app.

Preparation makes no provider calls. Live runs are user-invoked, interactive, and
keep exact action approvals unless native auto-approve is explicitly selected. No answer, transcript or result is published.
"""
import argparse
import functools
import hashlib
import http.server
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.parse
from credential_store import Keychain
from try_live import base_environment, obtain_credential

ROOT = pathlib.Path(__file__).resolve().parent.parent
POINTER = ROOT / 'private' / 'snake-comparison.json'
MODEL = 'claude-opus-5'


class GameHandler(http.server.SimpleHTTPRequestHandler):
    def send_head(self):
        path = urllib.parse.unquote(urllib.parse.urlsplit(self.path).path)
        if path not in ['/', '/index.html', '/styles.css', '/game.js']:
            self.send_error(404)
            return None
        file = pathlib.Path(self.directory) / (path.lstrip('/') or 'index.html')
        if file.is_symlink() or not file.is_file():
            self.send_error(404)
            return None
        return super().send_head()


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.write_text(json.dumps(value, indent=2) + '\n')
    path.chmod(0o600)


def prepare():
    base = pathlib.Path(tempfile.mkdtemp(prefix='s1code-snake-'))
    source = ROOT / 'fixtures' / 'snake'
    prompt = (source / 'prompt.txt').read_text()
    (base / 'prompt.txt').write_text(prompt)
    seed = base / 'starter'
    shutil.copytree(source / 'starter', seed)
    environment = base_environment()
    identity = {}
    for field in ['name', 'email']:
        result = subprocess.run(['git', 'config', '--get', 'user.' + field], cwd=ROOT,
                                env=environment, text=True, capture_output=True)
        if not result.returncode and result.stdout.strip():
            identity[field] = result.stdout.strip()
    if len(identity) != 2:
        raise RuntimeError('Configure your human git user.name/user.email before preparing a comparison.')
    def git(*args):
        return subprocess.check_output(['git', '-c', 'core.hooksPath=/dev/null',
                                       '-c', 'commit.gpgsign=false', '-c', 'user.name='+identity['name'],
                                       '-c', 'user.email='+identity['email'], *args],
                                      cwd=seed, env=environment, stderr=subprocess.PIPE)
    git('init', '-q')
    git('add', '.')
    git('commit', '-qm', 'Initialize shared Snake comparison starter')
    commit = git('rev-parse', 'HEAD').decode().strip()
    for arm in ['claude', 's1code']:
        git('clone', '--quiet', '--local', '--no-hardlinks', str(seed), str(base / arm))
        # Agents may inspect local history, but cannot accidentally push a trial.
        subprocess.run(['git', '-C', str(base / arm), 'remote', 'remove', 'origin'],
                       env=environment, check=True, capture_output=True)
    manifest = {'version': 1, 'directory': str(base), 'model': MODEL,
                'starting_commit': commit, 'prompt_sha256': hashlib.sha256(prompt.encode()).hexdigest(),
                'mode': 'interactive product comparison; no model calls during preparation',
                'claude_auth': 'official CLI managed account', 's1code_auth': 'native Anthropic and TypeSafe APIs',
                'cost_comparability': 'subscription and API costs are not equivalent; unknown usage stays unknown'}
    write_json(base / 'manifest.json', manifest)
    write_json(POINTER, manifest)
    return manifest


def load():
    if not POINTER.exists():
        raise RuntimeError('Run prepare first.')
    return json.loads(POINTER.read_text())


def command(arm, workspace, base, model, prompt, requests, auto_approve=False):
    if arm == 'claude':
        return ['claude', '--model', model, '--safe-mode', '--strict-mcp-config',
                '--mcp-config', '{"mcpServers":{}}', '--', prompt]
    if not requests or requests < 1 or requests > 200:
        raise ValueError('S1Code requires --max-requests N (1..200): explicit total API request consent, not a dollar cap.')
    return [str(ROOT / 'target/release/s1code'), '--home', str(base / 'records' / 'sessions'),
            'run', prompt, '--workspace', str(workspace), '--provider', 'claude', '--model', model,
            '--decision', 'jev', '--jev-provider', 'typesafe', '--eviction', 'jev',
            '--max-provider-requests', str(requests), '--max-generations', str(min(requests, 20)),
            '--max-steps', '80'] + (['--auto-approve'] if auto_approve else [])


def run(arm, manifest, requests, auto_approve=False):
    if not sys.stdin.isatty() or not sys.stdout.isatty():
        raise RuntimeError('Run this in your terminal; tool approvals remain interactive.')
    base = pathlib.Path(manifest['directory'])
    workspace = base / arm
    prompt = (base / 'prompt.txt').read_text()
    if hashlib.sha256(prompt.encode()).hexdigest() != manifest['prompt_sha256']:
        raise RuntimeError('Shared prompt changed; prepare a new comparison.')
    record = base / 'records' / (arm + '.json')
    if record.exists():
        raise RuntimeError('This arm was already started. Use prepare for a fresh trial; do not overwrite results.')
    environment = base_environment()
    status = subprocess.check_output(['git', 'status', '--porcelain'], cwd=workspace, env=environment)
    head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=workspace, env=environment).decode().strip()
    if status or head != manifest['starting_commit']:
        raise RuntimeError('Starting workspace changed; prepare a new comparison.')
    argv = command(arm, workspace, base, manifest['model'], prompt, requests, auto_approve)
    if arm == 's1code':
        # Build scripts never receive provider credentials.
        subprocess.run(['cargo', 'build', '--locked', '--release'], cwd=ROOT, env=environment, check=True)
        store = Keychain() if sys.platform == 'darwin' else None
        environment['ANTHROPIC_API_KEY'] = obtain_credential('Anthropic secret: ', 'claude', store)
        environment['TYPESAFE_API_KEY'] = obtain_credential('TypeSafe secret: ', 'typesafe', store)
    else:
        if not shutil.which('claude'):
            raise RuntimeError('Install the official Claude Code CLI and complete its managed login first.')
        account = subprocess.run(['claude', 'auth', 'status'], env=environment,
                                 capture_output=True, text=True, timeout=20)
        if account.returncode:
            raise RuntimeError('Claude managed login is unavailable. Run claude auth login, then retry; this arm has not started.')
    result = {'arm': arm, 'status': 'started', 'model_requested': manifest['model'],
              'prompt_sha256': manifest['prompt_sha256'], 'starting_commit': head,
              'max_provider_requests': requests if arm == 's1code' else None,
              'auto_approve_supported_actions': auto_approve if arm == 's1code' else False,
              'cost_usd': None, 'usage': None, 'gameplay_verified': False,
              'timing_note': 'Interactive wall time includes approvals and user idle time; not inference latency.'}
    write_json(record, result)
    started = time.monotonic()
    try:
        exit_code = subprocess.call(argv, cwd=workspace, env=environment)
        result.update(status='process_exited', exit_code=exit_code)
    except KeyboardInterrupt:
        result.update(status='interrupted', exit_code=130)
    finally:
        result['interactive_wall_seconds'] = time.monotonic() - started
        write_json(record, result)
    print('Run recorded privately. Process exit is not proof that the game works; inspect it in the browser.')


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='action', required=True)
    sub.add_parser('prepare')
    sub.add_parser('prompt')
    sub.add_parser('claude')
    native = sub.add_parser('s1code')
    native.add_argument('--max-requests', type=int, required=True)
    native.add_argument('--auto-approve', '--full-access', action='store_true', help='Approve supported native actions automatically; other commands/paths remain denied')
    serve = sub.add_parser('serve')
    serve.add_argument('arm', choices=['claude', 's1code'])
    args = parser.parse_args()
    manifest = prepare() if args.action == 'prepare' else load()
    if args.action == 'prepare':
        print('Two clean projects prepared at:', manifest['directory'])
        print('Same starting commit:', manifest['starting_commit'])
        print('No model calls. Use claude or s1code --max-requests N to send the identical prompt once.')
    elif args.action == 'prompt':
        print((pathlib.Path(manifest['directory']) / 'prompt.txt').read_text())
    elif args.action == 'serve':
        port = 8081 if args.arm == 'claude' else 8082
        handler = functools.partial(GameHandler,
                                    directory=str(pathlib.Path(manifest['directory']) / args.arm))
        print(f'{args.arm}: http://127.0.0.1:{port} — separate origins isolate best-score storage.', flush=True)
        with http.server.ThreadingHTTPServer(('127.0.0.1', port), handler) as server:
            server.serve_forever()
    else:
        run(args.action, manifest, getattr(args, 'max_requests', None), getattr(args, 'auto_approve', False))


if __name__ == '__main__':
    try:
        main()
    except (RuntimeError, ValueError, subprocess.CalledProcessError) as error:
        print('Comparison setup:', error, file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        sys.exit(130)
