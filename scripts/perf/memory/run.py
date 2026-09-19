"""macOS release comparison. Uses an isolated app identity and keeps every run."""
import argparse
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import socket
import statistics
import subprocess
import threading
import time

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
spec = importlib.util.spec_from_file_location('measure', REPO/'docs/benchmarks/2026-09-19/measure.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
IDENTIFIER = 'dev.tinydash.memoryoptimization.20260919'
PROFILE = Path.home()/'Library/Application Support'/IDENTIFIER
CONTROL = None

def command(op, **values):
    with socket.socket(socket.AF_UNIX) as stream:
        stream.settimeout(25)
        stream.connect(str(CONTROL/'control.sock'))
        stream.sendall((json.dumps(dict(op=op, **values))+'\n').encode())
        response = json.loads(stream.makefile().readline())
    if 'error' in response:
        raise RuntimeError(response['error'])
    result = response['ok']
    if op == 'eval':
        if 'error' in result:
            raise RuntimeError(result['error'])
        return result['value']
    return result

def evaluate(script, window='main'):
    return command('eval', script=script, window=window)

def until(check, timeout=30):
    deadline = time.monotonic()+timeout
    last = None
    while time.monotonic() < deadline:
        try:
            if value := check(): return value
        except Exception as error:
            last = error
        time.sleep(.05)
    raise RuntimeError(f'Condition timed out: {last}')

def environment():
    result = {'disk_free_bytes': shutil.disk_usage(PROFILE.parent).free}
    for name, args in [('os', ['sw_vers']), ('pressure', ['memory_pressure', '-Q']),
                       ('swap', ['sysctl', 'vm.swapusage']), ('thermal', ['pmset', '-g', 'therm'])]:
        result[name] = subprocess.run(args, text=True, capture_output=True).stdout.strip()
    return result

def sample(pid):
    for attempt in range(3):
        try: return m.sample(pid)
        except OSError:
            if attempt == 2: raise

def stage(pid, name, smoke=False):
    print(f'{pid} {name}: settle and sample', flush=True)
    time.sleep(.1 if smoke else 30)
    rows = []
    for _ in range(1 if smoke else 31):
        rows.append(sample(pid))
        time.sleep(.01 if smoke else 1)
    total = [sum(p['footprint_bytes'] for p in row['processes'])/2**20 for row in rows]
    result = {'samples': rows, 'median_mib': statistics.median(total), 'range_mib': [min(total), max(total)]}
    print(f'{pid} {name}: {result["median_mib"]:.2f} MiB', flush=True)
    return result

def reset_profile(daily, fixture):
    PROFILE.mkdir(parents=True, exist_ok=True)
    if not (PROFILE/'.memory-benchmark-owner').exists():
        raise RuntimeError('The test profile is not marked as owned by this benchmark.')
    for name in ['settings.json', 'tinydash.sqlite3', 'tinydash.sqlite3-journal',
                 'tinydash.sqlite3-wal', 'tinydash.sqlite3-shm']:
        (PROFILE/name).unlink(missing_ok=True)
    (PROFILE/'settings.json').write_text(json.dumps({
        'shortcut': 'Control+Alt+Shift+Space', 'hideOnBlur': False,
        'fileSearchRoots': [str(fixture)] if daily else [],
        'fileWatchEnabled': daily, 'fileSearchLimit': 50000,
        'currencyRatesEnabled': False, 'clipboardHistoryEnabled': daily,
        'clipboardHistoryDecided': True,
    })+'\n')
    # Seed through the production database API before startup in the diagnostic overlay.

def launch(source, page, daily, fixture):
    bundle = source/'src-tauri/target/release/bundle/macos/TinyDashMemoryOptimization.app'
    executable = (bundle/'Contents/MacOS/tinydash').resolve()
    assert not any(path.endswith('/TinyDashMemoryOptimization.app/Contents/MacOS/tinydash')
                   for path in m.processes().values()), 'Another isolated benchmark app is running'
    reset_profile(daily, fixture)
    (CONTROL/'app.log').write_text('')
    (CONTROL/'control.sock').unlink(missing_ok=True)
    subprocess.run(['open', '-n', str(bundle), '--env', f'TINYDASH_MEMORY_CONTROL={CONTROL}',
                    '--env', f'TINYDASH_MEMORY_PAGE={page}', '--env', f'TINYDASH_MEMORY_DAILY={int(daily)}',
                    '--stdout', str(CONTROL/'app.log'), '--stderr', str(CONTROL/'app.log')], check=True)
    pid = until(lambda: next((pid for pid, path in m.processes().items() if path == str(executable)), None))
    assert m.LIB.responsibility_get_pid_responsible_for_pid(pid) == pid, 'Process attribution is not isolated'
    return pid, executable

def run(source, label, page, profile, trial, output, fixture, smoke, skip_settings=False):
    data = dict(build=label, page=page, profile=profile, trial=trial,
                smoke_only=smoke, timestamp=time.strftime('%Y-%m-%dT%H:%M:%S%z'), environment=environment())
    path = output/f'{label}-{profile}-{page}-{trial}.json'
    if path.exists(): raise RuntimeError(f'Refusing to replace a run: {path}')
    pid = None
    stop_sampling = threading.Event()
    sampler = None
    try:
        pid, executable = launch(source, page, profile == 'daily', fixture)
        data['pid'] = pid
        data['executable_sha256'] = hashlib.sha256(executable.read_bytes()).hexdigest()
        until(lambda: (s := command('status'))['ready'] and not s['scanning'])
        until(lambda: evaluate('return typeof window.__memoryQuery === "function" && !!document.querySelector("input");'))
        if page == 'minimal': until(lambda: evaluate('return window.__memoryReady;'))
        # Size audits are functional checks. Avoid the extra strings during
        # production footprint sampling as well as native latency checks.
        evaluate(f'window.__memoryCountBytes={json.dumps(smoke)}; return true;')
        data['response_size_audit'] = smoke
        evaluate('localStorage.setItem("tinydash.appearance","light"); window.dispatchEvent(new StorageEvent("storage",{key:"tinydash.appearance",newValue:"light"})); return true;')
        catalog = evaluate('return (await window.__TAURI_INTERNALS__.invoke("app_catalog")).map(row=>({id:row.id,title:row.title}));')
        data['application_corpus'] = catalog
        data['corpus_sha256'] = hashlib.sha256(json.dumps(catalog,sort_keys=True).encode()).hexdigest()
        if profile == 'daily':
            until(lambda: evaluate('const result=await window.__TAURI_INTERNALS__.invoke("search",{query:"",mode:"all"}); return !result.files.indexing && result.files.total === 50000;'), timeout=120)
        time.sleep(1 if smoke else 10)
        data['page_identity'] = evaluate('return {url: location.href, minimal: !!window.__memoryMinimal, marker: document.body.dataset.page ?? "launcher", scale: devicePixelRatio, width: innerWidth, height: innerHeight, appearance:localStorage.getItem("tinydash.appearance")};')
        assert data['page_identity']['minimal'] == (page == 'minimal')
        assert ('minimal.html' in data['page_identity']['url']) == (page == 'minimal')
        data['workload_samples'] = []
        def sampling():
            while not stop_sampling.wait(1):
                try: data['workload_samples'].append(sample(pid))
                except Exception as error: data.setdefault('sampling_errors', []).append(str(error))
        sampler = threading.Thread(target=sampling)
        sampler.start()
        if profile == 'daily':
            command('clipboard', text='memory fixture 098 '+'x'*16365)
            until(lambda: evaluate('const result=await window.__TAURI_INTERNALS__.invoke("search",{query:"",mode:"clipboard"}); return result.preferredSelectionId === "clipboard:99";'))
            data['clipboard_capture_verified'] = True
            original = fixture/'folder-000/document-000.txt'
            renamed = fixture/'folder-000/renamed-memory.txt'
            original.rename(renamed)
            try:
                until(lambda: evaluate('const result=await window.__TAURI_INTERNALS__.invoke("search",{query:"renamed-memory",mode:"files"}); return result.results.some(row=>row.title==="renamed-memory.txt");'), timeout=60)
                data['file_watch_update_verified'] = True
            finally:
                renamed.rename(original)
            time.sleep(2)
        data['backend_queries'] = []
        queries = [('saf', 'Safari'), ('term', 'Terminal'), ('12 * 8', '96'), ('123 + 456', '579')]
        for iteration in range(-2, 1 if smoke else 25):
            for query, expected in queries:
                trace = evaluate(f'return await window.__memoryQuery({json.dumps(query)});')
                assert len(trace) == 1 and trace[0]['args'] == {'query': query, 'mode': 'all'}, trace
                assert expected in trace[0]['titles'], trace
                data['backend_queries'].append(dict(iteration=iteration, query=query, trace=trace))
        evaluate('return await window.__memoryQuery("");')
        data['visible'] = stage(pid, 'visible', smoke)
        command('hide')
        data['hidden'] = stage(pid, 'hidden', smoke)
        if page == 'real':
            command('show')
            # The native latency loop must not time diagnostic serialization.
            evaluate('window.__memoryTraceEnabled=false; window.__memoryTrace.length=0; return true;')
            data['queries'] = m.ui('queries', pid, 1 if smoke else 10)
            data['reopen'] = m.ui('summon', pid, 1 if smoke else 20)
            data['latency_trace_disabled'] = True
            command('hide')
            data['settings_started_monotonic'] = time.monotonic()
            data['settings_cycles'] = []
            data['settings_open_ms'] = []
            for cycle in range(0 if skip_settings else (1 if smoke else 20)):
                opened = time.monotonic()
                command('settings')
                until(lambda: evaluate('return !!document.querySelector(".settings-fields");', 'settings'))
                # A raw editing value must survive both retained and released windows.
                if cycle == 0:
                    evaluate('document.querySelectorAll("nav button")[2].click(); return true;', 'settings')
                    until(lambda: evaluate('return document.querySelector("select[size]")?.options.length > 0;', 'settings'))
                    evaluate('const select=document.querySelector("select[size]"); select.value=select.options[0].value; select.dispatchEvent(new Event("change",{bubbles:true})); const area=document.querySelector("textarea"); area.value="memory draft"; area.dispatchEvent(new Event("input",{bubbles:true})); return true;', 'settings')
                else:
                    value = evaluate('return document.querySelector("textarea")?.value;', 'settings')
                    assert value == 'memory draft', value
                data['settings_open_ms'].append((time.monotonic()-opened)*1000)
                command('close', window='settings')
                until(lambda: not any(w['label']=='settings' and w['visible'] for w in command('status')['windows']))
                data['settings_cycles'].append(command('status'))
            if not skip_settings: data['after_settings'] = stage(pid, 'after_settings', smoke)
        data['final_status'] = command('status')
        data['environment_after'] = environment()
    except BaseException as error:
        data['error'] = f'{type(error).__name__}: {error}'
        raise
    finally:
        stop_sampling.set()
        if sampler: sampler.join()
        if pid:
            if pid in m.processes(): m.stop(pid)
            time.sleep(.5)
            data['remaining_helpers'] = m.members(pid)
        data['app_log'] = (CONTROL/'app.log').read_text(errors='replace')
        path.write_text(json.dumps(data, indent=2)+'\n')
        print(path, flush=True)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('source', type=Path)
    parser.add_argument('--label', required=True)
    parser.add_argument('--control', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--pages', default='real,minimal')
    parser.add_argument('--profiles', default='controlled')
    parser.add_argument('--runs', type=int, default=5)
    parser.add_argument('--smoke', action='store_true')
    parser.add_argument('--skip-settings', action='store_true')
    args = parser.parse_args()
    global CONTROL
    CONTROL = args.control.resolve()
    if len(str(CONTROL/'control.sock').encode()) >= 104:
        raise SystemExit('Use a short control directory, for example /tmp/tinydash-memory-control.')
    CONTROL.mkdir(parents=True, exist_ok=True)
    if not PROFILE.exists():
        PROFILE.mkdir(parents=True)
        (PROFILE/'.memory-benchmark-owner').write_text(IDENTIFIER+'\n')
    lock = (PROFILE/'.memory-benchmark-lock').open('a')
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    args.output.mkdir(parents=True, exist_ok=True)
    ui = CONTROL/'ui'
    if not ui.exists():
        subprocess.run(['swiftc', '-O', str(REPO/'docs/benchmarks/2026-09-19/ui.swift'), '-o', str(ui)], check=True)
    m.BASE = CONTROL
    fixture = CONTROL/'files'
    if 'daily' in args.profiles:
        for folder in range(100):
            directory = fixture/f'folder-{folder:03}'
            directory.mkdir(parents=True, exist_ok=True)
            for entry in range(500):
                file = directory/f'document-{entry:03}.txt'
                if not file.exists(): file.write_text('TinyDash memory fixture\n')
    for trial in range(1, args.runs+1):
        pages = args.pages.split(',')
        if trial % 2 == 0: pages.reverse()
        for profile in args.profiles.split(','):
            for page in pages:
                run(args.source.resolve(), args.label, page, profile, trial, args.output, fixture, args.smoke, args.skip_settings)

if __name__ == '__main__':
    main()
