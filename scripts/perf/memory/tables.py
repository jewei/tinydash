"""Write readable tables from the complete comparison summary."""
import argparse
import json
from pathlib import Path

LABELS={'baseline2':'Original','p1':'Settings release','p2':'Deferred copies',
        'p3a':'On-demand data URL','p3b':'Binary PNG trial (deferred)',
        'final':'Settings release trial (deferred)','icon-final':'Selected icon changes'}
ORDER=['baseline2/controlled','p1/controlled','p2/controlled','p3a/controlled',
       'p3b/controlled','baseline2/daily','p3b/daily']

def label(key):
    build,profile=key.split('/')
    return LABELS[build]+' / '+profile

def write(summary, source_name='summary.json'):
    configurations=summary['configurations']
    candidate = next((name for name in ['icon-final','final'] if name+'/controlled' in configurations),None)
    order = (['baseline2/controlled',candidate+'/controlled','baseline2/daily',candidate+'/daily']
             if candidate else ORDER)
    assert set(configurations)==set(order)
    assert all(value['complete'] for value in configurations.values())
    lines=['# Memory comparison tables','',
           'Each run starts a fresh application process. Each idle-state value is the median of 31 physical-footprint samples after a 30-second wait. Values include the host and every attributed helper. The final column shows the range of the five run values.','']
    for state,title in [('visible','Visible idle footprint'),('hidden','Hidden footprint'),
                        ('after_settings','Hidden footprint after 20 Settings cycles'),
                        ('peak_before_settings','Sampled peak for the common workload before Settings'),
                        ('peak','Sampled peak for the full workload')]:
        lines.extend([title+' in MiB.','',
            '| Build and profile | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Median | Range |',
            '| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |'])
        for key in order:
            item=configurations[key]['footprint'].get(state)
            if not item:continue
            lines.append('| '+label(key)+' | '+' | '.join(f'{value:.1f}' for value in item['values'])+
                f' | {item["median"]:.1f} | {item["min"]:.1f} to {item["max"]:.1f} |')
        lines.append('')
    if not candidate:
        lines.extend(['P2 and P3a omit the Settings cycle phase. Use the common-workload peak when comparing those steps.',''])
    lines.extend(['Sampling can miss short peaks.','',
        'Hidden host footprint in MiB. This table excludes helpers. It must not replace the total-application tables.','',
        '| Build and profile | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Median | Range |',
        '| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |'])
    for key in order:
        item=configurations[key]['components']['hidden']['host']
        lines.append('| '+label(key)+' | '+' | '.join(f'{value:.1f}' for value in item['values'])+
            f' | {item["median"]:.1f} | {item["min"]:.1f} to {item["max"]:.1f} |')
    lines.extend(['','Latency in milliseconds. The main launcher timings use native accessibility checks. Settings reopen timings end when the restored editing fields are ready. The first Settings visit also includes selection of an application and entry of the test draft. These timings do not measure exact display-pixel arrival.','',
        '| Build and profile | Operation | Samples | p50 | p95 | Range of run p50 values |',
        '| --- | --- | ---: | ---: | ---: | --- |'])
    for key in order:
        for group,title in [('apps','App search'),('arithmetic','Arithmetic'),('reopen','Warm launcher reopen'),
                            ('settings_first','First Settings visit and draft setup'),('settings_reopen','Settings reopen')]:
            item=configurations[key]['latency'].get(group)
            if not item:continue
            spread=item['run_p50']
            lines.append(f'| {label(key)} | {title} | {item["n"]} | {item["p50_ms"]:.1f} | {item["p95_ms"]:.1f} | {spread["min"]:.1f} to {spread["max"]:.1f} |')
    lines.extend(['','The following verdicts apply the plan\'s latency budgets and measured run-to-run spread. An inconclusive verdict is not a passed performance gate.','',
        '| Parent | Candidate | Profile | Operation | p50 change | p95 change | Verdict |',
        '| --- | --- | --- | --- | ---: | ---: | --- |'])
    for comparison in summary['comparisons']:
        for operation,item in comparison['latency'].items():
            lines.append(f'| {LABELS[comparison["parent"]]} | {LABELS[comparison["candidate"]]} | {comparison["profile"]} | {operation} | {item["p50_delta_ms"]:+.1f} | {item["p95_delta_ms"]:+.1f} | {item["verdict"]} |')
    lines.extend(['',f'All values come from `{source_name}`. Raw records retain process identities, environment data, individual latency samples, and failures. No helper or valid outlier was removed.',''])
    return '\n'.join(lines)

if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('summary',type=Path)
    parser.add_argument('output',type=Path)
    args=parser.parse_args()
    args.output.write_text(write(json.loads(args.summary.read_text()),args.summary.name))
