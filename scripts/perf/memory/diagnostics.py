"""Summarize icon and sustained-use checks without treating N=1 as replication."""
import argparse
import json
from pathlib import Path
import statistics

from summarize import latency, spread, total


def state(samples):
    names = sorted({Path(p['executable']).name for s in samples for p in s['processes']})
    return {
        'total_mib': statistics.median(map(total, samples)),
        'process_mib': {name: statistics.median(
            sum(p['footprint_bytes'] for p in s['processes']
                if Path(p['executable']).name == name)/2**20 for s in samples)
            for name in names},
    }


def read(path):
    data = json.loads(path.read_text())
    assert not data.get('error'), (path, data.get('error'))
    return data


def summarize(root, candidate):
    result = {'icons': {}, 'settings_growth': {}, 'repeated': {}, 'capacity': {}}
    for build in ['p3a', 'p3b']:
        rows = []
        for trial in range(1, 6):
            data = read(root/'icon-results'/f'{build}-icons-{trial}.json')
            assert len(data['warm']) == 100 and data['hidden_images_released']
            row = {'trial': trial, 'file': f'{build}-icons-{trial}.json',
                   'after_warm_visible': state([data['after_warm_visible_sample']]),
                   'hidden': state(data['hidden']['samples']),
                   'sampled_peak_mib': max(map(total, data['samples'])),
                   'unique_representation_bytes': data['unique_representation_bytes'],
                   'unique_icon_requests': len(data['eviction']),
                   'missing_icons': sum('error' in item for item in data['eviction'])}
            for phase in ['cold', 'warm', 'after_eviction']:
                row[phase] = {name: latency([item[field] for item in data[phase]])
                              for name, field in [('text', 'textMs'), ('decoded_icons', 'decodedIconMs')]}
            row['delayed'] = {key: data['delayed_icon_check'][key]
                              for key in ['textMs', 'decodedIconMs']}
            rows.append(row)
        result['icons'][build] = {
            'independent_runs': 5, 'runs': rows,
            'hidden_mib': spread([row['hidden']['total_mib'] for row in rows]),
            'sampled_peak_mib': spread([row['sampled_peak_mib'] for row in rows]),
            'latency': {phase: {name: {
                'run_p50_ms': spread([row[phase][name]['p50_ms'] for row in rows]),
                'run_p95_ms': spread([row[phase][name]['p95_ms'] for row in rows]),
            } for name in ['text', 'decoded_icons']}
                       for phase in ['cold', 'warm', 'after_eviction']},
        }
    growth = read(root/'settings-growth.json')
    result['settings_growth'] = {'independent_runs': 1,
        'initial': state(growth['initial']['samples']),
        'groups': [{'cycles': group['cycles'], **state(group['hidden']['samples'])}
                   for group in growth['groups']]}
    if candidate == 'final':
        growth = read(root/'settings-growth-fixed.json')
        result['settings_growth_fixed'] = {'independent_runs': 1,
            'initial': state(growth['initial']['samples']),
            'groups': [{'cycles': group['cycles'], **state(group['hidden']['samples'])}
                       for group in growth['groups']]}
        allocations = read(root/'settings-allocations-fixed/result.json')
        result['native_windows_fixed'] = {name: value['native_tao_windows']
                                          for name, value in allocations['states'].items()}
    suffix = '-'+candidate if candidate in ['final','icon-final'] else ''
    for build in ['baseline2', candidate]:
        data = read(root/('repeated-results'+suffix)/f'{build}-repeated.json')
        assert len(data['groups']) == 10
        result['repeated'][build] = {'independent_runs': 1,
            'groups': [{'group': group['group'], 'dom': group['dom'],
                        **state(group['hidden']['samples'])} for group in data['groups']],
            'sampled_peak_mib': max(map(total, data['samples']))}
        data = read(root/('capacity-results'+suffix)/f'{build}-capacity.json')
        result['capacity'][build] = {'independent_runs': 1,
            'clipboard_rows': data['clipboard_rows'],
            'hidden': state(data['hidden']['samples']),
            'after_refresh': state(data['after_refresh']['samples']),
            'sampled_peak_mib': max(map(total, data['samples']))}
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('root', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--candidate', choices=['p3b', 'final', 'icon-final'], default='icon-final')
    args = parser.parse_args()
    args.output.write_text(json.dumps(summarize(args.root, args.candidate), indent=2)+'\n')
