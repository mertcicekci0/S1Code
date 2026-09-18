#!/usr/bin/env python3
"""OFFLINE protocol/PTY test: same-process Codex follow-ups and safe resume. No inference."""
import fcntl
import json
import os
import pathlib
import pty
import select
import struct
import subprocess
import tempfile
import termios
import time

binary = pathlib.Path('target/debug/s1code').resolve()
with tempfile.TemporaryDirectory(prefix='s1code-conversation-') as directory:
    base = pathlib.Path(directory)
    fakebin = base / 'bin'
    fakebin.mkdir()
    report = base / 'requests.jsonl'
    stub = fakebin / 'codex'
    stub.write_text('''#!/usr/bin/env python3
import json,sys,pathlib
if '--version' in sys.argv:
    print('codex-cli 0.153.3');sys.exit()
report=pathlib.Path(__file__).parent.parent/'requests.jsonl'
def send(v): print(json.dumps(v),flush=True)
turn=0
for line in sys.stdin:
    r=json.loads(line);m=r.get('method');p=r.get('params',{})
    with report.open('a') as f: f.write(json.dumps(r)+'\\n')
    if 'id' not in r: continue
    result={}
    if m=='account/read': result={'account':{'type':'chatgpt'}}
    if m in ['thread/start','thread/resume']:
        result={'thread':{'id':'fixture-thread'},'approvalPolicy':p['approvalPolicy'],'approvalsReviewer':'user','sandbox':{'type':'readOnly','networkAccess':False},'cwd':p['cwd']}
    if m=='thread/read': result={'thread':{'turns':[{'id':'turn-2','status':'completed','items':[]}]}}
    if m=='turn/start':
        turn+=1;result={'turn':{'id':f'turn-{turn}'}}
    send({'id':r['id'],'result':result})
    if m=='turn/start':
        send({'method':'item/completed','params':{'threadId':'fixture-thread','turnId':f'turn-{turn}','item':{'id':f'message-{turn}','type':'agentMessage','text':f'OFFLINE fixture reply {turn}; no inference.'}}})
        send({'method':'turn/completed','params':{'threadId':'fixture-thread','turn':{'id':f'turn-{turn}','status':'completed'}}})
''')
    stub.chmod(0o700)
    env = {k: v for k, v in os.environ.items() if not any(x in k for x in ['KEY', 'TOKEN', 'SECRET', 'PASSWORD'])}
    env.update(TERM='xterm-256color', PATH=str(fakebin)+os.pathsep+env.get('PATH', ''))
    def launch(args):
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 100, 0, 0))
        proc = subprocess.Popen([str(binary), '--home', str(base/'data'), *args], cwd=base, env=env,
                                stdin=slave, stdout=slave, stderr=slave, start_new_session=True)
        os.close(slave)
        return master, proc
    def wait_input(master, proc, count):
        end = time.monotonic()+15
        while time.monotonic()<end:
            if select.select([master], [], [], 0.05)[0]:
                try: os.read(master, 65536)
                except OSError: break
            files = list((base/'data').glob('**/events.jsonl'))
            if files:
                events=[json.loads(x) for x in files[0].read_text().splitlines()]
                if sum(e['kind']=='input_ready' for e in events)>=count:
                    time.sleep(0.15)
                    return
            if proc.poll() is not None: break
        raise AssertionError('conversation did not become ready')
    def finish(master, proc):
        end=time.monotonic()+10
        while proc.poll() is None and time.monotonic()<end:
            if select.select([master],[],[],0.05)[0]:
                try: os.read(master,65536)
                except OSError: break
        proc.wait(timeout=1)
    master, proc = launch(['run', 'hello fixture', '--mode', 'codex', '--workspace', str(base)])
    try:
        wait_input(master, proc, 1)
        os.write(master, b'now second fixture\r')
        wait_input(master, proc, 2)
        os.write(master, b'\x1b')
        finish(master, proc)
        assert proc.returncode == 0
    finally:
        if proc.poll() is None: proc.kill()
        os.close(master)
    requests = [json.loads(x) for x in report.read_text().splitlines()]
    assert sum(r.get('method')=='initialize' for r in requests)==1
    assert sum(r.get('method')=='thread/start' for r in requests)==1
    turns = [r for r in requests if r.get('method')=='turn/start']
    assert len(turns)==2 and all(r['params']['threadId']=='fixture-thread' for r in turns)
    assert turns[1]['params']['input'][0]['text'].startswith('now second fixture')
    session_file = next((base/'data').glob('**/checkpoint.json'))
    session = json.loads(session_file.read_text())
    assert session['status']=='awaiting_input' and session['metrics']['delegations']==2
    master, proc = launch(['resume', session['id']])
    try:
        wait_input(master, proc, 3)
        os.write(master, b'\x1b')
        finish(master, proc)
        assert proc.returncode==0
    finally:
        if proc.poll() is None: proc.kill()
        os.close(master)
    requests = [json.loads(x) for x in report.read_text().splitlines()]
    assert sum(r.get('method')=='turn/start' for r in requests)==2, 'resume resubmitted a turn'
    assert sum(r.get('method')=='thread/resume' for r in requests)==1
print('PASS: OFFLINE two turns, one runtime/thread, clean close, resume without resubmission; no inference.')
