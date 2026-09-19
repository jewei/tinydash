import json
import math
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parent / 'results'


def stats(values):
    ordered = sorted(values)
    return {'n': len(values), 'median': statistics.median(ordered),
            'p95': ordered[math.ceil(0.95 * len(ordered)) - 1],
            'min': ordered[0], 'max': ordered[-1]}


summary = {}
for app in ['tinydash', 'bopop', 'tinycast']:
    runs = [json.loads(p.read_text()) for p in sorted(ROOT.glob(f'{app}-[123].json'))]
    if not runs:
        continue
    queries = [s for run in runs for s in run['queries']['samples'] if not s['warmup']]
    summon = [s for run in runs for s in run['summon']['samples'] if not s['warmup']]
    assert all(s['success'] and s['prior_result_absent'] for s in queries)
    assert all(s['success'] for s in summon)
    trials = []
    for run in runs:
        def total(sample, key):
            return sum(p[key] for p in sample['processes'])

        samples = run['idle_samples']
        first, last = samples[0], samples[-1]
        seconds = last['monotonic'] - first['monotonic']
        cpu_ns = sum(total(last, key) - total(first, key) for key in ['user_ns', 'system_ns'])
        trials.append({
            'trial': run['trial'], 'sample_seconds': seconds,
            'process_names': [Path(p['executable']).name for p in first['processes']],
            'idle_cpu_percent_one_core': cpu_ns / 1e9 / seconds * 100,
            'idle_interrupt_wakeups_per_second':
                (total(last, 'interrupt_wakeups') - total(first, 'interrupt_wakeups')) / seconds,
            'idle_footprint_mib': stats([total(s, 'footprint_bytes') / 2**20 for s in samples]),
            'idle_rss_mib': stats([total(s, 'rss_bytes') / 2**20 for s in samples]),
            'visible_footprint_mib': stats([total(s, 'footprint_bytes') / 2**20 for s in run['visible_samples']]),
            'host_footprint_mib': stats([next(p['footprint_bytes'] for p in s['processes'] if p['pid'] == run['pid']) / 2**20 for s in samples]),
        })
    summary[app] = {
        'trials': trials,
        'apps_ms': stats([s['ms'] for s in queries if s['group'] == 'apps']),
        'arithmetic_ms': stats([s['ms'] for s in queries if s['group'] == 'arithmetic']),
        'summon_ms': stats([s['ms'] for s in summon]),
        'median_trial_idle_footprint_mib': statistics.median(t['idle_footprint_mib']['median'] for t in trials),
        'median_trial_visible_footprint_mib': statistics.median(t['visible_footprint_mib']['median'] for t in trials),
        'median_trial_idle_rss_mib': statistics.median(t['idle_rss_mib']['median'] for t in trials),
        'median_trial_idle_cpu_percent_one_core': statistics.median(t['idle_cpu_percent_one_core'] for t in trials),
    }

(ROOT / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
for name, result in summary.items():
    print(name, len(result['trials']), 'trials', json.dumps({k: v for k, v in result.items() if k != 'trials'}))
