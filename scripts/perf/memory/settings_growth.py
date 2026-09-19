"""Check whether host retention stabilizes across repeated Settings recreation."""
import argparse
import importlib.util
import json
from pathlib import Path
import time

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('memory_run',HERE/'run.py')
r=importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)

def run(source,output):
    record={'build':source.name,'diagnostic_only':True,'independent_runs':1,
            'environment':r.environment(),'groups':[],'cycles':[]}
    pid=None
    try:
        pid,binary=r.launch(source,'real',False,r.CONTROL/'files')
        record['binary_sha256']=r.hashlib.sha256(binary.read_bytes()).hexdigest()
        r.until(lambda:r.command('status')['ready'] and not r.command('status')['scanning'])
        r.until(lambda:r.evaluate('return !!window.__memoryBackend;'))
        r.evaluate('localStorage.setItem("tinydash.appearance","light"); window.dispatchEvent(new StorageEvent("storage",{key:"tinydash.appearance",newValue:"light"})); window.__memoryCountBytes=false; return true;')
        r.evaluate('return await window.__memoryQuery("saf");')
        r.evaluate('window.__memoryTraceEnabled=false; return true;')
        # The comparison uses accessibility before it opens Settings. Match
        # that condition because native accessibility can retain its own data.
        r.command('show')
        record['queries']=r.m.ui('queries',pid,10)
        record['reopen']=r.m.ui('summon',pid,20)
        r.command('hide')
        record['initial']=r.stage(pid,'before Settings cycles')
        for group in range(5):
            for cycle in range(20):
                started=time.monotonic()
                r.command('settings')
                r.until(lambda:r.evaluate('return !!document.querySelector(".settings-fields");','settings'))
                if group==0 and cycle==0:
                    # Match the application-editor workload used by the matrix.
                    r.evaluate('document.querySelectorAll("nav button")[2].click(); return true;','settings')
                    r.until(lambda:r.evaluate('return document.querySelector("select[size]")?.options.length>0;','settings'))
                    r.evaluate('const select=document.querySelector("select[size]"); select.value=select.options[0].value; select.dispatchEvent(new Event("change",{bubbles:true})); const area=document.querySelector("textarea"); area.value="growth draft"; area.dispatchEvent(new Event("input",{bubbles:true})); return true;','settings')
                else:
                    assert r.evaluate('return document.querySelector("textarea")?.value;','settings')=='growth draft'
                r.command('close',window='settings')
                r.until(lambda:all(window['label']!='settings' for window in r.command('status')['windows']))
                record['cycles'].append({'cycle':group*20+cycle+1,'ms':(time.monotonic()-started)*1000,
                                         'sample':r.sample(pid)})
            state=r.stage(pid,f'after {20*(group+1)} Settings cycles')
            record['groups'].append({'cycles':20*(group+1),'hidden':state})
            output.write_text(json.dumps(record,indent=2)+'\n')
        record['final_status']=r.command('status')
    except BaseException as error:
        record['error']=f'{type(error).__name__}: {error}'
        raise
    finally:
        if pid and pid in r.m.processes():r.m.stop(pid)
        output.write_text(json.dumps(record,indent=2)+'\n')

if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('source',type=Path)
    parser.add_argument('--control',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    r.CONTROL=args.control.resolve()
    r.m.BASE=r.CONTROL
    lock=(r.PROFILE/'.memory-benchmark-lock').open('a')
    r.fcntl.flock(lock,r.fcntl.LOCK_EX|r.fcntl.LOCK_NB)
    assert not args.output.exists(),args.output
    args.output.parent.mkdir(parents=True,exist_ok=True)
    run(args.source.resolve(),args.output)
