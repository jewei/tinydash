"""Inspect host retention. Profiler results are separate from benchmark results."""
import argparse
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('memory_run', HERE/'run.py')
r = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)


def inspect(pid, output, label):
    state = r.stage(pid, label)
    tools = {}
    for name, args in [
        ('heap', ['heap', '-s', '--noContent', str(pid)]),
        ('vmmap', ['vmmap', '-summary', str(pid)]),
        ('leaks', ['leaks', '--noContent',
                   '--outputGraph='+str(output/(label+'.memgraph')), str(pid)]),
    ]:
        result = subprocess.run(args, capture_output=True, text=True, timeout=120)
        (output/(label+'-'+name+'.txt')).write_text(result.stdout+result.stderr)
        tools[name] = result.returncode
    heap = (output/(label+'-heap.txt')).read_text()
    windows = sum(int(match[1]) for match in re.finditer(
        r'^\s+(\d+)\s+\d+\s+[\d.]+\s+(?:NSKVONotifying_)?TaoWindow\s+ObjC',
        heap, re.MULTILINE))
    return {'state': state, 'profiler_exit_codes': tools,
            'native_tao_windows': windows}


def run(source, output, assert_bounded=False):
    record = {'diagnostic_only': True, 'independent_runs': 1,
              'profilers_attached': True, 'states': {}}
    pid = None
    try:
        pid, binary = r.launch(source, 'real', False, r.CONTROL/'files')
        record.update(pid=pid, binary_sha256=r.hashlib.sha256(binary.read_bytes()).hexdigest())
        r.until(lambda: r.command('status')['ready'] and not r.command('status')['scanning'])
        r.until(lambda: r.evaluate('return !!window.__memoryBackend;'))
        r.evaluate('localStorage.setItem("tinydash.appearance","light"); window.dispatchEvent(new StorageEvent("storage",{key:"tinydash.appearance",newValue:"light"})); window.__memoryCountBytes=false; return true;')
        r.evaluate('return await window.__memoryQuery("saf");')
        r.evaluate('window.__memoryTraceEnabled=false; return true;')
        r.command('show')
        r.m.ui('queries', pid, 10)
        r.m.ui('summon', pid, 20)
        r.command('hide')
        record['states']['initial'] = inspect(pid, output, 'initial')
        for group in range(2):
            for cycle in range(20):
                r.command('settings')
                r.until(lambda: r.evaluate('return !!document.querySelector(".settings-fields");', 'settings'))
                if group == 0 and cycle == 0:
                    r.evaluate('document.querySelectorAll("nav button")[2].click(); return true;', 'settings')
                    r.until(lambda: r.evaluate('return document.querySelector("select[size]")?.options.length>0;', 'settings'))
                    r.evaluate('const select=document.querySelector("select[size]"); select.value=select.options[0].value; select.dispatchEvent(new Event("change",{bubbles:true})); const area=document.querySelector("textarea"); area.value="allocation draft"; area.dispatchEvent(new Event("input",{bubbles:true})); return true;', 'settings')
                else:
                    assert r.evaluate('return document.querySelector("textarea")?.value;', 'settings') == 'allocation draft'
                r.command('close', window='settings')
                r.until(lambda: all(w['label'] != 'settings' for w in r.command('status')['windows']))
            label = 'after-'+str((group+1)*20)
            record['states'][label] = inspect(pid, output, label)
            (output/'result.json').write_text(json.dumps(record, indent=2)+'\n')
        record['final_status'] = r.command('status')
        if assert_bounded:
            counts = [value['native_tao_windows'] for value in record['states'].values()]
            assert all(1 <= count <= 2 for count in counts), counts
            record['bounded_native_windows'] = True
    except BaseException as error:
        record['error'] = f'{type(error).__name__}: {error}'
        raise
    finally:
        if pid and pid in r.m.processes(): r.m.stop(pid)
        (output/'result.json').write_text(json.dumps(record, indent=2)+'\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('source', type=Path)
    parser.add_argument('--control', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--assert-bounded', action='store_true')
    args = parser.parse_args()
    r.CONTROL = args.control.resolve()
    r.m.BASE = r.CONTROL
    with (r.PROFILE/'.memory-benchmark-lock').open('a') as lock:
        print('Waiting for the current native workload.', flush=True)
        r.fcntl.flock(lock, r.fcntl.LOCK_EX)
        assert not args.output.exists(), args.output
        args.output.mkdir(parents=True)
        run(args.source.resolve(), args.output, args.assert_bounded)
