#!/usr/bin/env python3
"""Offline PTY check: task entry, setup errors, explicit handoff, demo, return home."""
import fcntl
import json
import os
import pathlib
import pty
import re
import select
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

binary = pathlib.Path('target/debug/s1code').resolve()
with tempfile.TemporaryDirectory(prefix='s1code-home-') as directory:
    base = pathlib.Path(directory)
    fakebin = base / 'bin'
    fakebin.mkdir()
    report = base / 'handoff.json'
    stub = fakebin / 'claude'
    stub.write_text('#!/usr/bin/env python3\nimport json,os,sys\n'
                    f'open({str(report)!r},"w").write(json.dumps({{"args":sys.argv[1:],"cwd":os.getcwd(),"other_keys":[k for k in ["OPENAI_API_KEY","OPENROUTER_API_KEY","TYPESAFE_API_KEY"] if k in os.environ]}}))\n'
                    'print("OFFLINE HANDOFF FIXTURE; no Claude inference",flush=True)\n')
    stub.chmod(0o700)
    env = dict(os.environ)
    for key in ['ANTHROPIC_API_KEY','ANTHROPIC_AUTH_TOKEN','CLAUDE_CODE_OAUTH_TOKEN','OPENAI_API_KEY','TYPESAFE_API_KEY','OPENROUTER_API_KEY']:
        env.pop(key,None)
    env.update(TERM='xterm-256color',PATH=str(fakebin)+os.pathsep+env.get('PATH',''),
               S1CODE_KEYCHAIN='off',
               OPENAI_API_KEY='fixture-only-secret',OPENROUTER_API_KEY='fixture-only-secret')
    master, slave = pty.openpty()
    fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',30,100,0,0))
    process = subprocess.Popen([str(binary),'--home',str(base/'data')],cwd=base,env=env,
                               stdin=slave,stdout=slave,stderr=slave,start_new_session=True)
    os.close(slave)
    output = bytearray()
    def pump(duration=0.1):
        stop = time.monotonic()+duration
        while time.monotonic()<stop:
            if select.select([master],[],[],0.02)[0]:
                try: output.extend(os.read(master,65536))
                except OSError: return
    def seen(needle,start=0):
        plain=re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]",b"",bytes(output[start:]))
        return re.sub(rb"\s+",b"",needle) in re.sub(rb"\s+",b"",plain)
    def wait_for(needle,start=0,timeout=10):
        stop=time.monotonic()+timeout
        while not seen(needle,start) and time.monotonic()<stop:
            pump()
        assert seen(needle,start), f'missing terminal marker {needle!r}'
    def send(text):
        mark=len(output)
        os.write(master,text.encode())
        return mark
    try:
        wait_for(b'What would you like to build or fix?')
        assert b'OFFLINE SIMULATION' not in output
        send('/provider claude\r')
        wait_for(b'ANTHROPIC_API_KEY')
        mark=send('Fix the parser\r')
        wait_for(b'ANTHROPIC_API_KEY missing',mark)
        assert not list((base/'data'/'sessions').glob('*/checkpoint.json'))
        mark=send('/claude-code\r')
        wait_for(b'Returned to task entry',mark)
        handoff=json.loads(report.read_text())
        assert handoff=={'args':[],'cwd':str(base.resolve()),'other_keys':[]}
        fcntl.ioctl(master,termios.TIOCSWINSZ,struct.pack('HHHH',30,50,0,0))
        os.kill(process.pid,signal.SIGWINCH)
        pump(0.1)
        fcntl.ioctl(master,termios.TIOCSWINSZ,struct.pack('HHHH',32,110,0,0))
        os.kill(process.pid,signal.SIGWINCH)
        pump(0.1)
        send('/demo\r')
        approved=set()
        approval_attempts={}
        deadline=time.monotonic()+25
        completed=None
        while time.monotonic()<deadline:
            pump()
            paths=list((base/'data'/'sessions').glob('*/checkpoint.json'))
            if not paths: continue
            try: session=json.loads(paths[0].read_text())
            except (OSError,json.JSONDecodeError): continue
            pending=session.get('pending')
            if pending and time.monotonic() >= approval_attempts.get(pending['id'], 0):
                send('y')
                approved.add(pending['id'])
                approval_attempts[pending['id']] = time.monotonic() + 0.5
            if session['status']=='completed':
                completed=session
                break
        assert completed and len(approved)==3
        journal=[json.loads(line) for line in (paths[0].parent/'events.jsonl').read_text().splitlines()]
        acknowledged=[e['data']['candidate'] for e in journal if e['kind']=='approved']
        assert len(acknowledged)==3 and set(acknowledged)==approved
        pump(0.3)
        mark=send('q')
        deadline=time.monotonic()+5
        while not seen(b'Task view closed',mark) and time.monotonic()<deadline:
            pump(0.25)
            if not seen(b'Task view closed',mark): send('q')
        wait_for(b'Task view closed',mark)
        send('\x15/exit\r')
        deadline=time.monotonic()+5
        while process.poll() is None and time.monotonic()<deadline: pump()
        assert process.wait(timeout=3)==0
        assert b'\x1b[?1049l' in output
        assert b'fixture-only-secret' not in output
        assert b'OFFLINE SIMULATION' in output
        print(json.dumps({'home':True,'missing_key_error':True,'provider_selection':True,
                          'handoff_fixture':True,'handoff_filters_other_keys':True,
                          'demo_approvals':3,'returns_home':True,'terminal_restored':True}))
        workspace=pathlib.Path(completed['workspace'])
        if workspace.parent==pathlib.Path(tempfile.gettempdir()).resolve() and workspace.name.startswith('s1code-demo-'):
            shutil.rmtree(workspace)
    except Exception:
        # No real credentials or user source enter this fixture.
        print(repr(bytes(output[-3000:])),file=sys.stderr)
        raise
    finally:
        if process.poll() is None:
            os.killpg(process.pid,signal.SIGKILL)
            process.wait()
        os.close(master)
