"""Temporary release memory experiment. Uses only the isolated test identity."""
import argparse
import json
import statistics
import subprocess
import time
from pathlib import Path

import measure as m

BASE = Path(__file__).resolve().parent
BUNDLE = BASE / 'source/src-tauri/target/release/bundle/macos/TinyDashMemoryProbe.app'
m.APPS['probe'] = (BUNDLE, 'tinydash')


def action(name):
    command_id = time.monotonic_ns()
    path = BASE / 'command.json'
    temporary = BASE / 'command.tmp'
    temporary.write_text(json.dumps({'id': command_id, 'action': name}))
    temporary.replace(path)
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        try:
            ack = json.loads((BASE / 'ack.json').read_text())
            if ack['id'] == command_id:
                assert ack['error'] is None, ack
                return ack
        except (FileNotFoundError, json.JSONDecodeError):
            pass
        time.sleep(.02)
    raise RuntimeError('Command timed out: ' + name)


def stage(data, name, settle=30):
    pid = data['pid']
    print(name + ': wait ' + str(settle) + ' seconds', flush=True)
    time.sleep(settle)
    samples = []
    for _ in range(5):
        samples.append(m.sample(pid))
        time.sleep(1)
    status = m.ui('status', pid)
    assert not status['onscreen'], status
    members = {tuple(p['pid'] for p in s['processes']) for s in samples}
    assert len(members) == 1, 'Process membership changed during sampling'
    total = statistics.median(sum(p['footprint_bytes'] for p in s['processes']) / 2**20 for s in samples)
    row = {'samples': samples, 'status': status, 'median_footprint_mib': total, 'settle_seconds': settle}
    data['stages'][name] = row
    data['result_path'].write_text(json.dumps({k: v for k, v in data.items() if k != 'result_path'}, indent=2) + '\n')
    print(name + ': ' + str(round(total, 2)) + ' MiB', flush=True)
    print([(Path(p['executable']).name, round(p['footprint_bytes'] / 2**20, 2)) for p in samples[-1]['processes']], flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('mode', choices=['lifecycle', 'no-icons', 'growth', 'baseline'])
    args = parser.parse_args()
    marker = BASE / 'no-icons'
    if args.mode == 'no-icons':
        marker.touch()
    else:
        marker.unlink(missing_ok=True)
    pid, _ = m.launch('probe')
    data = {'mode': args.mode, 'pid': pid, 'stages': {}, 'result_path': BASE / ('results/' + args.mode + '.json')}
    try:
        assert m.LIB.responsibility_get_pid_responsible_for_pid(pid) == pid
        time.sleep(10)
        m.ui('show', pid)
        data['queries'] = m.ui('queries', pid, 10)
        data['summon'] = m.ui('summon', pid, 10)
        m.ui('hide', pid)
        stage(data, 'baseline')
        action('snapshot')
        time.sleep(1)
        data['dom'] = json.loads((BASE / 'snapshot.json').read_text())
        if args.mode != 'no-icons':
            data['icons'] = json.loads((BASE / 'icons.json').read_text())
        if args.mode == 'lifecycle':
            action('settings')
            time.sleep(5)
            action('hide-settings')
            stage(data, 'settings_hidden')
            action('destroy-settings')
            stage(data, 'settings_destroyed')
            action('blank')
            stage(data, 'main_blank')
            action('destroy')
            stage(data, 'main_destroyed')
            started = time.monotonic()
            action('show')
            m.ui('show', pid)
            data['recreation_probe_ms_including_control_transport'] = (time.monotonic() - started) * 1000
            data['recreated_queries'] = m.ui('queries', pid, 3)
            m.ui('hide', pid)
            stage(data, 'main_recreated')
        elif args.mode == 'growth':
            for cycle in range(1, 4):
                m.ui('show', pid)
                data['cycle_queries_' + str(cycle)] = m.ui('queries', pid, 25)
                data['cycle_summon_' + str(cycle)] = m.ui('summon', pid, 20)
                m.ui('hide', pid)
                stage(data, 'cycle_' + str(cycle))
    except Exception as error:
        data['error'] = str(error)
        raise
    finally:
        data['result_path'].write_text(json.dumps({k: v for k, v in data.items() if k != 'result_path'}, indent=2) + '\n')
        m.stop(pid)
        marker.unlink(missing_ok=True)


if __name__ == '__main__':
    main()
