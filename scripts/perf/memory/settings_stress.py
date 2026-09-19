"""Native Settings stress diagnostic; no memory or latency claim."""
import argparse
import fcntl
import hashlib
import importlib.util
import json
from pathlib import Path
import time

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('memory_run',HERE/'run.py')
r=importlib.util.module_from_spec(spec);spec.loader.exec_module(r)

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('source',type=Path)
    parser.add_argument('--control',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--cycles',type=int,default=300)
    parser.add_argument('--close-delay-ms',type=int,default=0)
    parser.add_argument('--clear-layers',action='store_true')
    parser.add_argument('--retain-settings',action='store_true')
    args=parser.parse_args()
    assert not args.output.exists(),args.output
    r.CONTROL=args.control.resolve();r.m.BASE=r.CONTROL
    lock=(r.PROFILE/'.memory-benchmark-lock').open('a');fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    data={'diagnostic_only':True,'cycles':[],'close_delay_ms':args.close_delay_ms,
          'clear_layers':args.clear_layers,'retain_settings':args.retain_settings,
          'environment':r.environment()}
    pid=None
    try:
        pid,binary=r.launch(args.source.resolve(),'real',True,r.CONTROL/'files')
        data['pid']=pid;data['binary_sha256']=hashlib.sha256(binary.read_bytes()).hexdigest()
        r.until(lambda:r.command('status')['ready'] and not r.command('status')['scanning'])
        r.until(lambda:r.evaluate('const result=await window.__memoryBackend.search("","all"); return !result.files.indexing && result.files.total===50000;'),timeout=120)
        time.sleep(10)
        r.evaluate('window.__memoryCountBytes=false; return true;')
        for query in ['saf','term','12 * 8','123 + 456']*25:r.evaluate('return await window.__memoryQuery('+json.dumps(query)+');')
        r.evaluate('window.__memoryTraceEnabled=false;return true;')
        data['queries']=r.m.ui('queries',pid,10)
        data['reopen']=r.m.ui('summon',pid,20)
        r.command('hide')
        for cycle in range(args.cycles):
            r.command('settings')
            r.until(lambda:r.evaluate('return !!document.querySelector(".settings-fields");','settings'))
            if args.close_delay_ms or args.clear_layers:
                r.evaluate('const original=window.__TAURI_INTERNALS__.invoke; window.__TAURI_INTERNALS__.invoke=async function(command,args,...rest) { if(command==="finish_settings_close") {'+
                    ('document.body.style.display="none";' if args.clear_layers else '')+
                    'await new Promise(resolve=>setTimeout(resolve,'+str(args.close_delay_ms)+')); } return original.call(this,command,args,...rest); };return true;','settings')
            if cycle==0:
                r.evaluate('document.querySelectorAll("nav button")[2].click();return true;','settings')
                r.until(lambda:r.evaluate('return document.querySelector("select[size]")?.options.length>0;','settings'))
                r.evaluate('const select=document.querySelector("select[size]");select.value=select.options[0].value;select.dispatchEvent(new Event("change",{bubbles:true}));const area=document.querySelector("textarea");area.value="memory draft";area.dispatchEvent(new Event("input",{bubbles:true}));return true;','settings')
            else:
                assert r.evaluate('return document.querySelector("textarea")?.value;','settings')=='memory draft'
            r.command('close',window='settings')
            if args.retain_settings:
                r.until(lambda:any(w['label']=='settings' and not w['visible'] for w in r.command('status')['windows']))
            else:
                r.until(lambda:all(w['label']!='settings' for w in r.command('status')['windows']))
            data['cycles'].append({'cycle':cycle+1,'status':r.command('status')})
            args.output.write_text(json.dumps(data,indent=2)+'\n')
            if (cycle+1)%25==0:print('Completed Settings stress cycles:',cycle+1,flush=True)
        data['passed']=True
    except BaseException as error:
        data['error']=f'{type(error).__name__}: {error}'
        raise
    finally:
        if pid and pid in r.m.processes():r.m.stop(pid)
        data['app_log']=(r.CONTROL/'app.log').read_text(errors='replace')
        args.output.write_text(json.dumps(data,indent=2)+'\n')

if __name__=='__main__':main()
