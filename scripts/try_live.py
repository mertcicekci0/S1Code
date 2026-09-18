#!/usr/bin/env python3
"""Explicitly budgeted, hidden-input Claude/OpenRouter checks. Never stores keys."""
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


def credential(label):
    # Never let getpass fall back to echoed input when no usable terminal exists.
    with warnings.catch_warnings():
        warnings.simplefilter('error', getpass.GetPassWarning)
        value = getpass.getpass(label).strip()
    if not value or any(c.isspace() for c in value):
        raise ValueError('Empty/invalid key. No request was sent.')
    if value.startswith('apikey_'):
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
    parser.add_argument('mode', choices=['check', 'task'], nargs='?', default='check')
    args = parser.parse_args()
    if not sys.stdin.isatty() or not sys.stdout.isatty():
        raise RuntimeError('Use your local interactive terminal; keys must not enter chat or a pipe.')
    environment = base_environment()
    if args.mode == 'check':
        executable = prepare_checks(environment)
        budget = 2
        print('LIVE CHECK: at most one Claude request and one OpenRouter Jev request. No tool actions execute.')
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
    anthropic = credential('Anthropic secret (hidden, not the apikey_ ID): ')
    if not anthropic.startswith('sk-ant-api'):
        raise ValueError('Expected a Claude Console API secret starting sk-ant-api; no request sent.')
    router = credential('OpenRouter secret (hidden): ')
    if router.startswith('sk-ant-'):
        raise ValueError('Anthropic keys cannot authenticate OpenRouter; no request sent.')
    if args.mode == 'check':
        for name, secret, test in [
            ('ANTHROPIC_API_KEY', anthropic, 'claude_live_contract'),
            ('OPENROUTER_API_KEY', router, 'openrouter_live_contract'),
        ]:
            result = subprocess.run([executable, test, '--exact', '--ignored', '--test-threads=1'],
                                    cwd=ROOT, env=check_environment(environment, name, secret))
            if result.returncode:
                print('Live check failed. Stopped; no automatic retry or other-provider fallback.')
                return result.returncode
        print('Both live contracts passed. This does not establish coding success or efficiency. Keep Jev results private.')
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
    run_env = dict(environment, ANTHROPIC_API_KEY=anthropic, OPENROUTER_API_KEY=router)
    return subprocess.run([
        executable, '--home', str(directory / 'data'), 'run',
        'Fix parse_count for whole signed integers, blanks and invalid text; run tests.',
        '--workspace', str(workspace), '--provider', 'claude', '--decision', 'jev',
        '--jev-provider', 'openrouter', '--max-provider-requests', '8', '--max-generations', '6',
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
