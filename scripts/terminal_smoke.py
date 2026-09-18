#!/usr/bin/env python3
"""Exercise the actual TUI in a disposable PTY, including narrow resize and approvals."""
import fcntl,json,os,pathlib,pty,select,signal,struct,subprocess,tempfile,termios,time
binary=pathlib.Path('target/debug/s1code').resolve()
base=pathlib.Path(tempfile.mkdtemp(prefix='s1code-terminal-'));home=base/'data';root=base/'repo'
master,slave=pty.openpty()
fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',24,50,0,0))
env=dict(os.environ);env['TERM']='xterm-256color'
process=subprocess.Popen([str(binary),'--home',str(home),'demo','--offline','--workspace',str(root)],stdin=slave,stdout=slave,stderr=slave,env=env,start_new_session=True)
os.close(slave);deadline=time.monotonic()+25;approved=set();output=bytearray();resized=False;done=False;next_close=0
try:
 while process.poll() is None and time.monotonic()<deadline:
  if select.select([master],[],[],0.05)[0]:
   try:output.extend(os.read(master,65536))
   except OSError:break
  checkpoints=list((home/'sessions').glob('*/checkpoint.json'))
  if checkpoints:
   try:session=json.loads(checkpoints[0].read_text())
   except (OSError,json.JSONDecodeError):continue
   pending=session.get('pending')
   if pending and pending['id'] not in approved:
    # Allow the UI to consume the event before sending its approval key.
    time.sleep(0.12);os.write(master,b'y');approved.add(pending['id'])
   if not resized and len(approved)>=1:
    fcntl.ioctl(master,termios.TIOCSWINSZ,struct.pack('HHHH',32,110,0,0));os.kill(process.pid,signal.SIGWINCH);os.write(master,b'2341');resized=True
   # The durable checkpoint can precede the last frame. Retry closing after
   # cleanup rather than assuming the first key arrives in the completed UI.
   if session['status']=='completed' and time.monotonic()>=next_close:
    os.write(master,b'q');done=True;next_close=time.monotonic()+0.25
 process.wait(timeout=3)
 assert process.returncode==0 and done and len(approved)==3
 assert b'OFFLINE SIMULATION' in output and b'\x1b[?1049l' in output
 assert b'python3 -m unittest -v' in output and b'Approve once' in output
 assert b'"policy_version"' not in output and b'"candidate"' not in output
 assert b'gpt-4.1' not in output
 print(json.dumps({'tui_completed':done,'approvals':len(approved),'resize_tested':[50,110],'terminal_restored':True,'simulation_label_visible':True,'human_approval_card':True,'default_json_dump':False}))
except Exception:
 # Fixture-only evidence helps distinguish a UI failure from PTY driver timing.
 (base/'terminal-output.bin').write_bytes(output)
 print(json.dumps({'fixture':str(base),'approvals':len(approved),'close_sent':done}))
 raise
finally:
 if process.poll() is None:process.kill();process.wait()
 os.close(master)
