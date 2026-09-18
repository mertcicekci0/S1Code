#!/usr/bin/env python3
"""Explicitly budgeted, hidden-input Claude/Jev checks. Never stores keys."""
import argparse
import getpass
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import warnings

ROOT = pathlib.Path(__file__).resolve().parent.parent
SAFE_ENV = ('PATH', 'HOME', 'CARGO_HOME', 'RUSTUP_HOME', 'TMPDIR', 'LANG', 'LC_ALL',
            'TERM', 'COLORTERM', 'TERM_PROGRAM')


def base_environment(source=None):
    source = os.environ if source is None else source
    return {name: source[name] for name in SAFE_ENV if name in source}


def credential(label, provider):
    # Never let getpass fall back to echoed input when no usable terminal exists.
    with warnings.catch_warnings():
        warnings.simplefilter('error', getpass.GetPassWarning)
        value = getpass.getpass(label).strip()
    if not value or any(c.isspace() for c in value) or '...' in value or '…' in value:
        raise ValueError('Empty/invalid key. No request was sent.')
    if provider == 'claude' and value.startswith('apikey_'):
        raise ValueError('That is a key ID, not its secret. Copy the secret from the provider console.')
    return value


def prepare_checks(environment):
    # Compile before collecting secrets, and give no credentials to Cargo/build scripts.
    result = subprocess.run(
        ['cargo', 'test', '--locked', '--test', 'live', '--no-run', '--message-format=json'],
        cwd=ROOT, env=environment, text=True, stdout=subprocess.PIPE, check=True,
    )
    for line in result.stdout.splitlines():
        record = json.loads(line)
        if record.get('reason') == 'compiler-artifact' and record.get('target', {}).get('name') == 'live' and record.get('executable'):
            return record['executable']
    raise RuntimeError('Live test executable not found; no request was sent.')


def check_environment(environment, name, secret):
    return dict(environment, **{name: secret, 'S1CODE_LIVE_BUDGET_REQUESTS': '1'})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['check', 'jev-check', 'task'], nargs='?', default='check')
    parser.add_argument('--jev-provider', choices=['typesafe', 'openrouter'], default='typesafe',
                        help='Jev credential issuer and endpoint (default: official TypeSafe)')
    parser.add_argument('--model', default='claude-opus-5', help='Claude generation model (default: claude-opus-5)')
    args = parser.parse_args()
    jev_name, jev_env, jev_test = {
        'typesafe': ('TypeSafe / official Jev', 'TYPESAFE_API_KEY', 'jev_live_contract'),
        'openrouter': ('OpenRouter Jev', 'OPENROUTER_API_KEY', 'openrouter_live_contract'),
    }[args.jev_provider]
    if not sys.stdin.isatty() or not sys.stdout.isatty():
        raise RuntimeError('Use your local interactive terminal; keys must not enter chat or a pipe.')
    environment = base_environment()
    if args.mode != "jev-check":
        print(f"Claude generation model: {args.model}")
    if args.mode in ['check', 'jev-check']:
        executable = prepare_checks(environment)
        budget = 1 if args.mode == 'jev-check' else 2
        print(f'LIVE CHECK: at most one {jev_name} request' + (' and one Claude request.' if budget == 2 else '.') + ' No tool actions execute.')
    else:
        subprocess.run(['cargo', 'build', '--locked', '--release'], cwd=ROOT, env=environment, check=True)
        executable = str(ROOT / 'target' / 'release' / 's1code')
        budget = 8
        print('LIVE TASK: at most 8 provider requests total, including retries, in a fresh parser fixture.')
        print('You approve each process and patch in the terminal. This is not the offline demo.')
    print('Request limits are not dollar limits. Set provider spending limits separately.')
    if input(f'Type RUN {budget} to authorize this paid run, or press Enter to stop: ').strip() != f'RUN {budget}':
        print('Stopped. No credentials collected and no provider requests sent.')
        return 0
    anthropic = None
    if args.mode != 'jev-check':
        anthropic = credential('Anthropic secret (hidden, not the apikey_ ID): ', 'claude')
        if not anthropic.startswith('sk-ant-api'):
            raise ValueError('Expected a Claude Console API secret starting sk-ant-api; no request sent.')
    jev_secret = credential(f'{jev_name} secret (hidden, full unmasked key): ', args.jev_provider)
    if jev_secret.startswith('sk-ant-'):
        raise ValueError('Anthropic keys cannot authenticate Jev; no request sent.')
    if args.jev_provider == 'typesafe' and jev_secret.startswith('sk-or-'):
        raise ValueError('Use the TypeSafe secret, or explicitly choose --jev-provider openrouter.')
    if args.mode in ['check', 'jev-check']:
        checks = [] if anthropic is None else [('ANTHROPIC_API_KEY', anthropic, 'claude_live_contract')]
        checks.append((jev_env, jev_secret, jev_test))
        for name, secret, test in checks:
            test_env = check_environment(environment, name, secret)
            if name == 'ANTHROPIC_API_KEY':
                test_env['S1CODE_LIVE_CLAUDE_MODEL'] = args.model
            result = subprocess.run([executable, test, '--exact', '--ignored', '--test-threads=1'],
                                    cwd=ROOT, env=test_env)
            if result.returncode:
                print('Live check failed. Stopped; no automatic retry or other-provider fallback.')
                return result.returncode
        print('Requested live contracts passed. This does not establish coding success or efficiency. Keep Jev results private.')
        return 0
    # Retain a private task/session for inspection and resume; never save the credentials.
    directory = pathlib.Path(tempfile.mkdtemp(prefix='s1code-live-'))
    workspace = directory / 'repo'
    workspace.mkdir(mode=0o700)
    for name in ['parser.py', 'test_parser.py']:
        shutil.copyfile(ROOT / 'fixtures' / 'demo' / name, workspace / name)
    subprocess.run(['git', 'init', '-q', str(workspace)], env=environment, check=True)
    print(f'Private live workspace/session: {directory}')
    print('Keys are not saved. Keep traces private; close the task view with q after it stops.')
    run_env = dict(environment, ANTHROPIC_API_KEY=anthropic, **{jev_env: jev_secret})
    return subprocess.run([
        executable, '--home', str(directory / 'data'), 'run',
        'Fix parse_count for whole signed integers, blanks and invalid text; run tests.',
        '--workspace', str(workspace), '--provider', 'claude', '--model', args.model, '--decision', 'jev',
        '--jev-provider', args.jev_provider, '--max-provider-requests', '8', '--max-generations', '6',
    ], env=run_env).returncode


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (EOFError, KeyboardInterrupt):
        print('\nStopped.', file=sys.stderr)
        sys.exit(130)
    except (ValueError, RuntimeError, getpass.GetPassWarning, subprocess.CalledProcessError) as error:
        print(f'S1Code setup: {error}', file=sys.stderr)
        sys.exit(1)
