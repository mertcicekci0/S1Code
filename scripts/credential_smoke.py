#!/usr/bin/env python3
"""Check hidden credential entry using invalid fixture input; never touch Keychain."""
import fcntl
import os
import pathlib
import pty
import select
import signal
import subprocess
import sys
import termios
import time

binary = pathlib.Path('target/debug/s1code').resolve()
env = {k: v for k, v in os.environ.items() if k in ('PATH', 'HOME', 'LANG', 'TMPDIR')}
if sys.platform != 'darwin':
    result = subprocess.run([str(binary), 'auth', 'set', 'claude'], env=env,
                            capture_output=True, timeout=10)
    assert result.returncode != 0 and b'requires macOS Keychain' in result.stderr
    print('PASS: platform limitation is explicit; no credentials stored.')
    sys.exit(0)

master, slave = pty.openpty()

def controlling_terminal():
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)

process = subprocess.Popen([str(binary), 'auth', 'set', 'claude'], env=env,
                           stdin=slave, stdout=slave, stderr=slave,
                           preexec_fn=controlling_terminal)
os.close(slave)
output = bytearray()
fixture = b'invalid-placeholder-not-a-provider-key'
sent = False
try:
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if select.select([master], [], [], 0.05)[0]:
            try:
                chunk = os.read(master, 65536)
                if not chunk:
                    break
                output.extend(chunk)
            except OSError:
                break
        if (not sent and b'stored in macOS Keychain' in output
                and not termios.tcgetattr(master)[3] & termios.ECHO):
            os.write(master, fixture + b'\n')
            sent = True
        if process.poll() is not None:
            # Drain the last error message before checking it.
            while select.select([master], [], [], 0)[0]:
                try:
                    chunk = os.read(master, 65536)
                    if not chunk:
                        break
                    output.extend(chunk)
                except OSError:
                    break
            break
    assert sent and process.wait(timeout=1) != 0
    assert fixture not in output
    assert b'no key was saved' in output
    assert termios.tcgetattr(master)[3] & termios.ECHO
    print('PASS: credential input hidden, invalid provider rejected, terminal echo restored; no Keychain mutation or inference.')
finally:
    if process.poll() is None:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
    os.close(master)
