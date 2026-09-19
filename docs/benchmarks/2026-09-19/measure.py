import ctypes
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time

BASE = Path(__file__).resolve().parent
RESULTS = BASE / 'results'
APPS = {
    'tinydash': (BASE / 'TinyDashPerf.app', 'tinydash'),
    'bopop': (BASE / 'BopopPerf.app', 'Bopop'),
    'tinycast': (BASE / 'tinycast-build/Build/Products/Release/Tinycast.app', 'Tinycast'),
}


class RUsage(ctypes.Structure):
    _fields_ = [('uuid', ctypes.c_uint8 * 16)] + [
        (name, ctypes.c_uint64) for name in (
            'user_ns', 'system_ns', 'idle_wakeups', 'interrupt_wakeups', 'pageins',
            'wired_bytes', 'resident_bytes', 'footprint_bytes', 'start_abstime', 'exit_abstime',
            'child_user_ns', 'child_system_ns', 'child_idle_wakeups', 'child_interrupt_wakeups',
            'child_pageins', 'child_elapsed_abstime', 'disk_read_bytes', 'disk_written_bytes',
        )
    ]


LIB = ctypes.CDLL('/usr/lib/libSystem.B.dylib', use_errno=True)
LIB.proc_pid_rusage.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.POINTER(RUsage)]
LIB.proc_pid_rusage.restype = ctypes.c_int
LIB.responsibility_get_pid_responsible_for_pid.argtypes = [ctypes.c_int]
LIB.responsibility_get_pid_responsible_for_pid.restype = ctypes.c_int


def processes():
    rows = subprocess.check_output(['ps', '-axo', 'pid,comm'], text=True).splitlines()[1:]
    return {int(p): name for p, name in (line.strip().split(None, 1) for line in rows)}


def members(pid):
    return {p: name for p, name in processes().items()
            if p == pid or LIB.responsibility_get_pid_responsible_for_pid(p) == pid}


def sample(pid):
    rows = []
    for p, name in members(pid).items():
        info = RUsage()
        if LIB.proc_pid_rusage(p, 2, ctypes.byref(info)) != 0:
            raise OSError(ctypes.get_errno(), f'Cannot read process {p}')
        rows.append(dict(pid=p, executable=name, user_ns=info.user_ns,
                         system_ns=info.system_ns, idle_wakeups=info.idle_wakeups,
                         interrupt_wakeups=info.interrupt_wakeups,
                         rss_bytes=info.resident_bytes, footprint_bytes=info.footprint_bytes))
    return dict(monotonic=time.monotonic(), processes=rows)


def ui(command, pid, *args):
    result = subprocess.run([str(BASE / 'ui'), command, str(pid), *map(str, args)],
                            text=True, capture_output=True, timeout=180)
    try:
        value = json.loads(result.stdout)
    except Exception:
        raise RuntimeError(f'UI {command}: {result.stdout} {result.stderr}')
    if result.returncode:
        raise RuntimeError(json.dumps(value))
    return value


def launch(name):
    bundle, executable = APPS[name]
    path = str((bundle / 'Contents/MacOS' / executable).resolve())
    assert not any(name == path for name in processes().values()), 'Test app is already running'
    start = time.monotonic()
    subprocess.run(['open', '-n', str(bundle)], check=True)
    for _ in range(200):
        found = [pid for pid, name in processes().items() if name == path]
        if found:
            return found[0], (time.monotonic() - start) * 1000
        time.sleep(0.05)
    raise RuntimeError('Could not locate launched test process')


def stop(pid):
    os.kill(pid, signal.SIGTERM)
    for _ in range(100):
        if pid not in processes():
            return
        time.sleep(0.05)
    raise RuntimeError(f'Process {pid} did not exit')


def size_report():
    sizes = {}
    for name, (bundle, executable) in APPS.items():
        path = bundle / 'Contents/MacOS' / executable
        # Count each regular file once. Do not count framework symlink targets twice.
        files = [p for p in bundle.rglob('*') if p.is_file() and not p.is_symlink()]
        sizes[name] = dict(executable_bytes=path.stat().st_size,
                           bundle_file_bytes=sum(p.stat().st_size for p in files),
                           executable_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                           file_description=subprocess.check_output(['file', str(path)], text=True).strip())
    (RESULTS / 'sizes.json').write_text(json.dumps(sizes, indent=2) + '\n')


def measure(name, trial):
    print(f'{name} trial {trial}: launch', flush=True)
    pid, discovery_ms = launch(name)
    data = dict(app=name, trial=trial, pid=pid, process_discovery_ms=discovery_ms,
                launch_timestamp=time.strftime('%Y-%m-%dT%H:%M:%S%z'))
    try:
        time.sleep(10)
        assert LIB.responsibility_get_pid_responsible_for_pid(pid) == pid, 'Attribution is not isolated'
        data['initial_show'] = ui('show', pid)
        time.sleep(1)
        print(f'{name} trial {trial}: query and summon samples', flush=True)
        data['queries'] = ui('queries', pid, 10)
        data['summon'] = ui('summon', pid, 10)
        data['visible_samples'] = []
        for _ in range(5):
            data['visible_samples'].append(sample(pid))
            time.sleep(1)
        data['hidden_check'] = ui('hide', pid)
        print(f'{name} trial {trial}: hidden, settle 30 seconds', flush=True)
        time.sleep(30)
        print(f'{name} trial {trial}: idle samples for 30 seconds', flush=True)
        data['idle_samples'] = []
        next_sample = time.monotonic()
        for _ in range(31):
            time.sleep(max(0, next_sample - time.monotonic()))
            data['idle_samples'].append(sample(pid))
            next_sample += 1
        assert len({tuple(x['pid'] for x in s['processes']) for s in data['idle_samples']}) == 1
        data['final_hidden_status'] = ui('status', pid)
        assert not data['final_hidden_status']['onscreen']
        print(f'{name} trial {trial}: done', flush=True)
    except Exception as error:
        data['error'] = str(error)
        raise
    finally:
        (RESULTS / f'{name}-{trial}.json').write_text(json.dumps(data, indent=2) + '\n')
        stop(pid)
        time.sleep(2)
        data['remaining_responsible_processes_after_exit'] = members(pid)
        (RESULTS / f'{name}-{trial}.json').write_text(json.dumps(data, indent=2) + '\n')


if __name__ == '__main__':
    size_report()
    order = [['tinydash', 'bopop', 'tinycast'], ['bopop', 'tinycast', 'tinydash'],
             ['tinycast', 'tinydash', 'bopop']]
    for trial, names in enumerate(order, 1):
        for name in names:
            measure(name, trial)
