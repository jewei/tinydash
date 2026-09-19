"""Record only the test application's window during keyboard use and reopening."""
import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import time

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('memory_run',HERE/'run.py')
r=importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)
parser=argparse.ArgumentParser()
parser.add_argument('source',type=Path)
parser.add_argument('--control',type=Path,required=True)
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
r.CONTROL=args.control.resolve()
r.m.BASE=r.CONTROL
lock=(r.PROFILE/'.memory-benchmark-lock').open('a')
r.fcntl.flock(lock,r.fcntl.LOCK_EX|r.fcntl.LOCK_NB)
args.output.mkdir(parents=True,exist_ok=True)
pid,executable=r.launch(args.source.resolve(),'real',False,r.CONTROL/'files')
record={'binary_sha256':r.hashlib.sha256(executable.read_bytes()).hexdigest(),'checks':[]}
capture=None
try:
    r.until(lambda:r.command('status')['ready'] and not r.command('status')['scanning'])
    time.sleep(1)
    r.evaluate('localStorage.setItem("tinydash.appearance","light"); window.dispatchEvent(new StorageEvent("storage",{key:"tinydash.appearance",newValue:"light"})); return true;')
    r.command('show')
    r.until(lambda:r.evaluate('return document.activeElement?.getAttribute("role")==="combobox";'))
    r.m.ui('text',pid,'saf')
    r.until(lambda:r.evaluate('return !!document.querySelector(".preview-icon img")?.naturalWidth;'))
    windows=json.loads(subprocess.check_output([str(r.CONTROL/'native'),'windows',str(pid)]))
    window=max(windows,key=lambda value:value['kCGWindowBounds']['Width']*value['kCGWindowBounds']['Height'])
    record['window']=window
    identifier=str(window['kCGWindowNumber'])
    subprocess.run(['screencapture','-x','-o','-l'+identifier,str(args.output/'launcher.png')],check=True)
    capture=subprocess.Popen(['screencapture','-v','-V6','-l'+identifier,str(args.output/'reopen.mov')],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
    time.sleep(1)
    for query in ['term','12 * 8','saf']:
        r.command('hide')
        r.until(lambda:not r.m.ui('status',pid)['onscreen'])
        r.m.ui('show',pid)
        assert r.evaluate('return document.activeElement?.getAttribute("role")==="combobox";')
        tree=r.m.ui('text',pid,query)
        record['checks'].append({'query':query,'tree':tree})
        time.sleep(.3)
    text,_=capture.communicate(timeout=15)
    record['capture_output']=text
    record['capture_exit_code']=capture.returncode
    assert capture.returncode==0
    subprocess.run(['screencapture','-x','-o','-l'+identifier,str(args.output/'reopened.png')],check=True)
except Exception as error:
    record['error']=str(error)
    try: record['context']=r.evaluate('return {active:document.activeElement?.outerHTML,query:document.querySelector("input[role=combobox]")?.value,text:document.body.textContent.slice(0,2000)};')
    except Exception: pass
    raise
finally:
    if capture and capture.poll() is None:
        capture.terminate()
        capture.wait(timeout=5)
    if pid in r.m.processes():r.m.stop(pid)
    (args.output/'check.json').write_text(json.dumps(record,indent=2)+'\n')
