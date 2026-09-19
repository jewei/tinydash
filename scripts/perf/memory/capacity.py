"""Maximum-setting diagnostic, separate from daily-use savings estimates."""
import argparse
import importlib.util
import json
from pathlib import Path
import sqlite3
import threading
import time

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('memory_run',HERE/'run.py')
r=importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)

def run(source,fixture,output):
    record={'build':source.name,'diagnostic_only':True,'independent_runs':1,
            'file_limit':100000,'unpinned_clipboard_limit':500,'new_pins':100,'samples':[]}
    pid=None
    stop=threading.Event()
    sampler=None
    try:
        pid,binary=r.launch(source,'real',True,fixture)
        record['binary_sha256']=r.hashlib.sha256(binary.read_bytes()).hexdigest()
        r.until(lambda:r.command('status')['ready'] and not r.command('status')['scanning'])
        r.until(lambda:r.evaluate('const result=await window.__memoryBackend.search("","all"); return !result.files.indexing && result.files.total===100000;'),timeout=120)
        r.evaluate('window.__memoryTraceEnabled=false; return true;')
        def sampling():
            while not stop.wait(.25):
                try:record['samples'].append(r.sample(pid))
                except Exception as error:record.setdefault('sampling_errors',[]).append(str(error))
        sampler=threading.Thread(target=sampling)
        sampler.start()
        for index in range(600):
            prefix=f'capacity fixture {index:03} '
            r.command('clipboard',text=prefix+'x'*(16384-len(prefix)))
            # Normal activation wakes the production clipboard monitor. Hide
            # again immediately; the test does not use the system clipboard.
            r.command('show')
            r.command('hide')
            # Duplicate seed capture can advance SQLite's AUTOINCREMENT.
            # Find the new item by its unique synthetic prefix, not an ID guess.
            identifier=r.until(lambda:r.evaluate(
                'const result=await window.__memoryBackend.search("","clipboard"); '
                'return result.results.find(row=>row.id===result.preferredSelectionId '
                '&& row.title.startsWith('+json.dumps(prefix)+'))?.id;'))
            if index<100:
                r.evaluate('await window.__memoryBackend.setPinned('+json.dumps(identifier)+',"clipboard",true); return true;')
            if (index+1)%100==0:print(f'{source.name}: captured {index+1} bounded entries',flush=True)
        with sqlite3.connect('file:'+str(r.PROFILE/'tinydash.sqlite3')+'?mode=ro',uri=True) as db:
            rows=db.execute('SELECT pinned,COUNT(*),SUM(length(CAST(content AS BLOB))) FROM clipboard_history GROUP BY pinned').fetchall()
        record['clipboard_rows']=[{'pinned':bool(pinned),'count':count,'text_bytes':size} for pinned,count,size in rows]
        assert dict((bool(pinned),count) for pinned,count,_ in rows)=={False:500,True:101},rows
        record['hidden']=r.stage(pid,'maximum configured files and history')
        r.evaluate('await window.__memoryBackend.refreshFiles(); return true;')
        r.until(lambda:r.evaluate('const result=await window.__memoryBackend.search("","all"); return !result.files.indexing && result.files.total===100000;'),timeout=120)
        record['after_refresh']=r.stage(pid,'capacity refresh')
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
    fixture=r.CONTROL/'capacity-files'
    for folder in range(100):
        directory=fixture/f'folder-{folder:03}'
        directory.mkdir(parents=True,exist_ok=True)
        for index in range(1000):
            path=directory/f'document-{index:04}.txt'
            if not path.exists():path.write_text('Capacity fixture\n')
    reset=r.reset_profile
    def profile(daily,fixture):
        reset(daily,fixture)
        path=r.PROFILE/'settings.json'
        value=json.loads(path.read_text())
        value.update(fileSearchLimit=100000,clipboardHistoryLimit=500)
        path.write_text(json.dumps(value)+'\n')
    r.reset_profile=profile
    args.output.mkdir(parents=True,exist_ok=True)
    for name in args.builds.split(','):
        output=args.output/f'{name}-capacity.json'
        assert not output.exists(),output
        run(args.root.resolve()/name,fixture,output)
