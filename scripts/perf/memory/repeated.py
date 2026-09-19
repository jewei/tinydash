"""Long-use diagnostic. One process per build; do not infer precise savings."""
import argparse
import importlib.util
import json
from pathlib import Path
import threading
import time

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('memory_run',HERE/'run.py')
r=importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)

def run(source,output):
    record={'build':source.name,'diagnostic_only':True,'independent_runs':1,'groups':[],
            'environment':r.environment(),'samples':[]}
    pid=None
    stop=threading.Event()
    sampler=None
    try:
        pid,binary=r.launch(source,'real',False,r.CONTROL/'files')
        record['binary_sha256']=r.hashlib.sha256(binary.read_bytes()).hexdigest()
        r.until(lambda:r.command('status')['ready'] and not r.command('status')['scanning'])
        time.sleep(10)
        r.evaluate('localStorage.setItem("tinydash.appearance","light"); window.dispatchEvent(new StorageEvent("storage",{key:"tinydash.appearance",newValue:"light"})); window.__memoryCountBytes=false; return true;')
        queries=[('saf','Safari'),('term','Terminal'),('12 * 8','96'),('123 + 456','579')]
        for query,_ in queries*2:r.evaluate('return await window.__memoryQuery('+json.dumps(query)+');')
        def sampling():
            while not stop.wait(1):
                try:record['samples'].append(r.sample(pid))
                except Exception as error:record.setdefault('sampling_errors',[]).append(str(error))
        sampler=threading.Thread(target=sampling)
        sampler.start()
        for group in range(10):
            # Opening clears the query and starts a normal refresh. Wait for
            # that response before measuring the first explicit query.
            r.evaluate('window.__memoryTraceEnabled=true; window.__memoryTrace.length=0; return true;')
            r.command('show')
            r.until(lambda:r.evaluate('return window.__memoryTrace.some(row=>row.args.query==="" && row.args.mode==="all");'))
            opening=r.evaluate('return window.__memoryTrace;')
            for query,expected in queries*25:
                trace=r.evaluate('return await window.__memoryQuery('+json.dumps(query)+');')
                assert len(trace)==1 and expected in trace[0]['titles'],trace
            r.evaluate('window.__memoryTraceEnabled=false; window.__memoryTrace.length=0; return true;')
            reopened=r.m.ui('summon',pid,20)
            r.command('hide')
            state=r.stage(pid,f'group {group+1}')
            dom=r.evaluate('return {nodes:document.querySelectorAll("*").length,images:document.images.length};')
            record['groups'].append({'group':group+1,'queries':100,'opening_refresh':opening,
                                     'reopen':reopened,'hidden':state,'dom':dom})
            output.write_text(json.dumps(record,indent=2)+'\n')
    except BaseException as error:
        record['error']=f'{type(error).__name__}: {error}'
        raise
    finally:
        stop.set()
        if sampler:sampler.join()
        if pid and pid in r.m.processes():r.m.stop(pid)
        output.write_text(json.dumps(record,indent=2)+'\n')

if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('root',type=Path)
    parser.add_argument('--control',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--builds',default='baseline2,p3b')
    args=parser.parse_args()
    r.CONTROL=args.control.resolve()
    r.m.BASE=r.CONTROL
    lock=(r.PROFILE/'.memory-benchmark-lock').open('a')
    r.fcntl.flock(lock,r.fcntl.LOCK_EX|r.fcntl.LOCK_NB)
    args.output.mkdir(parents=True,exist_ok=True)
    for name in args.builds.split(','):
        output=args.output/f'{name}-repeated.json'
        assert not output.exists(),output
        run(args.root.resolve()/name,output)
