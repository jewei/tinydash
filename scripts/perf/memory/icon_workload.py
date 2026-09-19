"""Measure text readiness, decoded icon arrival, and cache eviction separately."""
import argparse
import fcntl
import importlib.util
import json
from pathlib import Path
import threading
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('memory_run', HERE/'run.py')
r = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)

INSTALL = '''
window.__memoryIconTrace=[];
window.__memoryOriginalIcon=window.__memoryBackend.appIcon;
window.__memoryBackend.appIcon=async(...args)=>{
  const start=performance.now();
  let bytes;
  try { bytes=await window.__memoryOriginalIcon(...args); }
  catch(error) { window.__memoryIconTrace.push({key:args[0],pixels:args[1],error:String(error)}); throw error; }
  window.__memoryIconTrace.push({key:args[0],pixels:args[1],ms:performance.now()-start,
    bytes:typeof bytes === "string" ? bytes.length : bytes.byteLength,
    format:typeof bytes === "string" ? "data-url" : Object.prototype.toString.call(bytes)});
  if(window.__memoryIconTrace.length>1024) window.__memoryIconTrace.shift();
  return bytes;
};
return true;
'''

def show_query(query):
    return r.evaluate('''
await window.__memoryQuery("zzzxqvbenchmarknomatch");
window.__memoryIconTrace=[];
const start=performance.now();
const trace=await window.__memoryQuery('''+json.dumps(query)+''');
const deadline=start+5000;
const title=trace[0]?.titles[0];
while(![...document.querySelectorAll(".result-title")].some(node=>node.textContent?.trim()===title)) {
  if(performance.now()>deadline) throw new Error("Expected text result did not appear");
  await new Promise(resolve=>requestAnimationFrame(resolve));
}
const textMs=performance.now()-start;
let avatars=[];
do {
  avatars=[...document.querySelectorAll(".app-avatar")].filter(node=>{
    const bounds=node.getBoundingClientRect();
    const clip=node.closest("ul")?.getBoundingClientRect();
    return bounds.width && bounds.top<Math.min(innerHeight,clip?.bottom??innerHeight) && bounds.bottom>Math.max(0,clip?.top??0);
  });
  if(avatars.length && avatars.every(node=>{const image=node.querySelector("img");return image?.complete && image.naturalWidth>0;})) break;
  await new Promise(resolve=>requestAnimationFrame(resolve));
} while(performance.now()<deadline);
if(!avatars.length || avatars.some(node=>!node.querySelector("img")?.naturalWidth)) throw new Error("A visible native icon did not arrive");
await Promise.all(avatars.map(node=>node.querySelector("img").decode()));
return {textMs,decodedIconMs:performance.now()-start,trace,loads:window.__memoryIconTrace,
  images:avatars.map(node=>({cssWidth:node.getBoundingClientRect().width,pixels:node.querySelector("img").naturalWidth,source:node.querySelector("img").src.slice(0,32)})),scale:devicePixelRatio};
''')

def run(source, label, trial, output, smoke):
    path=output/f'{label}-icons-{trial}.json'
    if path.exists(): raise RuntimeError(f'Refusing to replace {path}')
    record={'build':label,'trial':trial,'smoke_only':smoke,'started':time.strftime('%Y-%m-%dT%H:%M:%S%z'),
            'environment':r.environment(),'samples':[]}
    pid=None
    stop=threading.Event()
    sampler=None
    try:
        pid,executable=r.launch(source,'real',False,r.CONTROL/'files')
        record['binary_sha256']=r.hashlib.sha256(executable.read_bytes()).hexdigest()
        r.until(lambda:r.command('status')['ready'] and not r.command('status')['scanning'])
        r.until(lambda:r.evaluate('return !!window.__memoryBackend && !!window.__memoryQuery;'))
        r.evaluate('localStorage.setItem("tinydash.appearance","light"); window.dispatchEvent(new StorageEvent("storage",{key:"tinydash.appearance",newValue:"light"})); return true;')
        r.evaluate(INSTALL)
        def sampling():
            while not stop.wait(1):
                try:record['samples'].append(r.sample(pid))
                except Exception as error:record.setdefault('sampling_errors',[]).append(str(error))
        sampler=threading.Thread(target=sampling)
        sampler.start()
        record['cold']=[show_query(query) for query in ['saf','term']]
        record['warm']=[show_query(query) for _ in range(1 if smoke else 50) for query in ['saf','term']]
        record['after_warm_visible_sample']=r.sample(pid)
        if not smoke:
            catalog=r.evaluate('return (await window.__memoryBackend.appCatalog()).map(row=>({id:row.id,title:row.title}));')
            record['eviction']=[]
            total=0
            for app in catalog:
                found=r.evaluate('const result=await window.__memoryBackend.search('+json.dumps(app['title'])+',"apps"); return result.results.find(row=>row.id==='+json.dumps(app['id'])+')?.icon;')
                if not found: continue
                for pixels in [72,128,192,256]:
                    result=r.evaluate('''const start=performance.now();try {
const data=await window.__memoryBackend.appIcon('''+json.dumps(found)+','+str(pixels)+','+json.dumps(f'eviction:{len(record["eviction"])}')+''');
return {ms:performance.now()-start,bytes:typeof data==="string"?data.length:data.byteLength};
}catch(error){return {error:String(error)};}''')
                    record['eviction'].append({'id':app['id'],'pixels':pixels,**result})
                    total+=result.get('bytes',0)
            assert total>2*1024*1024, 'The native workload must exceed the cache byte budget'
            record['unique_representation_bytes']=total
            record['after_eviction']=[show_query(query) for query in ['saf','term']]
        # Delay the production image path and verify that text remains usable.
        r.evaluate('window.__memoryBackend.appIcon=async(...args)=>{await new Promise(resolve=>setTimeout(resolve,500));return window.__memoryOriginalIcon(...args);}; return true;')
        delayed=show_query('Activity Monitor')
        assert delayed['textMs']<delayed['decodedIconMs']-250, delayed
        record['delayed_icon_check']=delayed
        r.command('hide')
        r.until(lambda:r.evaluate('return !document.querySelector(".app-avatar img");'))
        record['hidden_images_released']=True
        record['hidden']=r.stage(pid,'icons after eviction, hidden',smoke)
        print(f'{label} icon trial {trial} passed',flush=True)
    except Exception as error:
        record['error']=str(error)
        try:
            record['failure_context']=r.evaluate('return {trace:window.__memoryIconTrace,query:document.querySelector("input[role=combobox]")?.value,avatars:[...document.querySelectorAll(".app-avatar")].map(node=>({rect:node.getBoundingClientRect().toJSON(),html:node.outerHTML.slice(0,500)})),text:document.body.textContent.slice(0,2000)};')
            record['native_status']=r.command('status')
        except Exception as context_error: record['context_error']=str(context_error)
        raise
    finally:
        stop.set()
        if sampler:sampler.join()
        if pid and pid in r.m.processes(): r.m.stop(pid)
        path.write_text(json.dumps(record,indent=2)+'\n')

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('root',type=Path)
    parser.add_argument('--control',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--runs',type=int,default=5)
    parser.add_argument('--smoke',action='store_true')
    parser.add_argument('--builds',default='p3a,p3b')
    args=parser.parse_args()
    r.CONTROL=args.control.resolve()
    r.m.BASE=r.CONTROL
    lock=(r.PROFILE/'.memory-benchmark-lock').open('a')
    fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    args.output.mkdir(parents=True,exist_ok=True)
    for trial in range(1,args.runs+1):
        builds=args.builds.split(',')
        if trial%2==0: builds.reverse()
        for name in builds:
            run(args.root.resolve()/name,name+('-smoke' if args.smoke else ''),trial,args.output,args.smoke)

if __name__=='__main__': main()
