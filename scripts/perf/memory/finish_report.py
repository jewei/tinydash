"""Write the final comparison only after every selected native check completes."""
import argparse
import json
from pathlib import Path

from diagnostics import summarize as diagnostics
from summarize import summarize
from tables import write as tables


def finish(root, output, report):
    summary = summarize(root/'icon-comparison', False, True, 'icon-final')
    details = diagnostics(root, 'icon-final')
    stress = json.loads((root/'smoke/settings-retained-icon-final.json').read_text())
    assert stress.get('passed') and not stress.get('error') and len(stress['cycles']) == 1000
    smoke_records = list((root/'icon-final-smoke').glob('*.json'))
    assert len(smoke_records) == 4
    for path in smoke_records:
        assert not json.loads(path.read_text()).get('error'), path
    (output/'selected-summary.json').write_text(json.dumps(summary, indent=2)+'\n')
    (output/'selected-tables.md').write_text(tables(summary, 'selected-summary.json'))
    (output/'diagnostics.json').write_text(json.dumps(details, indent=2)+'\n')
    configurations = summary['configurations']
    text = report.read_text()
    text = text.replace('The final icon-only comparison is in progress.',
                        'The final icon-only comparison is complete.')
    text = text.replace('The final icon-only comparison is complete; the sustained-use and capacity checks are in progress.',
                        'The final comparison and both diagnostics are complete.')
    text = text.replace('The final native comparison is in progress.',
                        'The final native comparison is complete. The retained Settings window passed 1,000 open and close cycles with its draft preserved.')
    text = text.replace('P4 will use the repeated-use diagnostic to decide whether another frontend change is justified.',
                        'P4 adds no further frontend change. No specific retained allocation was established as unnecessary in this iteration.')
    lines = ['', 'The selected comparison uses 20 fresh processes: five per build and profile. Both builds use the original Settings hide behavior. The following values are medians of five run medians. Full per-run values and ranges are in the [selected comparison tables](benchmarks/2026-09-19/optimization/selected-tables.md).', '',
             '| Profile | State | Baseline, MiB | Selected, MiB | Change, MiB |',
             '| --- | --- | ---: | ---: | ---: |']
    for profile in ['controlled', 'daily']:
        old = configurations['baseline2/'+profile]
        new = configurations['icon-final/'+profile]
        for state, label in [('visible', 'Visible'), ('hidden', 'Hidden'),
                             ('after_settings', 'After Settings'), ('peak', 'Sampled peak')]:
            before, after = old['footprint'][state]['median'], new['footprint'][state]['median']
            lines.append(f'| {profile} | {label} | {before:.1f} | {after:.1f} | {after-before:+.1f} |')
        before = old['components']['hidden']['host']['median']
        after = new['components']['hidden']['host']['median']
        lines.append(f'| {profile} | Hidden host only | {before:.1f} | {after:.1f} | {after-before:+.1f} |')
    lines += ['', 'These are differences between complete builds. Do not add them to isolated Settings, minimal-page, or icon-trial savings. A sampled peak can miss a short allocation. Host-only values exclude WebKit and other helpers.', '',
              'Selected-build hidden memory by process group:', '',
              '| Profile | Host, MiB | Web content, MiB | Web GPU, MiB | Web network, MiB | Other helpers, MiB |',
              '| --- | ---: | ---: | ---: | ---: | ---: |']
    for profile in ['controlled', 'daily']:
        row = configurations['icon-final/'+profile]['components']['hidden']
        values = [row[name]['median'] for name in ['host', 'web_content', 'web_gpu', 'web_network', 'other_helpers']]
        lines.append('| '+profile+' | '+' | '.join(f'{value:.1f}' for value in values)+' |')
    lines += ['', 'These component medians need not add to the median total. A large WebKit process group identifies where memory is held. It does not establish that all that memory is required by the engine.', '',
              '| Profile | Operation | Baseline p50 / p95, ms | Selected p50 / p95, ms | Performance gate |',
              '| --- | --- | ---: | ---: | --- |']
    footprint_notes = []
    for comparison in summary['comparisons']:
        profile = comparison['profile']
        old = configurations['baseline2/'+profile]
        new = configurations['icon-final/'+profile]
        for operation, label in [('apps', 'App search'), ('arithmetic', 'Arithmetic'), ('reopen', 'Warm reopen')]:
            before, after = old['latency'][operation], new['latency'][operation]
            verdict = comparison['latency'][operation]['verdict']
            lines.append(f'| {profile} | {label} | {before["p50_ms"]:.1f} / {before["p95_ms"]:.1f} | {after["p50_ms"]:.1f} / {after["p95_ms"]:.1f} | {verdict} |')
        for state in ['hidden', 'after_settings']:
            value = comparison['footprint'][state]
            verdict = 'exceeds' if value['resolves_range'] else 'does not exceed'
            footprint_notes += ['', f'The {profile} {state.replace("_", " ")} median change ({value["delta_mib"]:+.1f} MiB) {verdict} the larger same-build range ({value["same_build_range_mib"]:.1f} MiB).']
    lines += footprint_notes
    lines += ['', 'An inconclusive performance gate is not a pass. The summary records the observed deltas, budgets, and independent-run spread. It retains valid outliers and every attributed helper.', '',
              'The sustained-use check uses one process per build. Each process completes ten groups of 100 queries and 20 warm reopen operations. The table compares the hidden sample after group 1 with the sample after group 10. It is a growth diagnostic, not an independent-launch savings estimate.', '',
              '| Build | After 100 queries, MiB | After 1,000 queries, MiB | Change, MiB |',
              '| --- | ---: | ---: | ---: |']
    for build, label in [('baseline2', 'Baseline'), ('icon-final', 'Selected')]:
        groups = details['repeated'][build]['groups']
        before, after = groups[0]['total_mib'], groups[-1]['total_mib']
        lines.append(f'| {label} | {before:.1f} | {after:.1f} | {after-before:+.1f} |')
    groups = details['repeated']['icon-final']['groups']
    later = [group['total_mib'] for group in groups[1:]]
    lines += ['', f'The selected build had a high first group. Groups 2 through 10 ranged from {min(later):.1f} to {max(later):.1f} MiB. This later range does not establish a savings estimate or prove that all unused allocations were released.']
    before, after = groups[0]['process_mib'], groups[-1]['process_mib']
    lines += ['', 'Selected-build process changes over the same interval:', '',
              '| Process | After 100 queries, MiB | After 1,000 queries, MiB | Change, MiB |',
              '| --- | ---: | ---: | ---: |']
    for name in sorted(set(before) | set(after)):
        start, end = before.get(name, 0), after.get(name, 0)
        lines.append(f'| {name} | {start:.1f} | {end:.1f} | {end-start:+.1f} |')
    lines += ['', 'The raw records identify each process group and its retained footprint. They do not identify individual JavaScript objects or prove that the app is free of leaks. No further JavaScript or rendering change is justified by these records alone.', '',
              'The capacity check uses one separate process per build, 100,000 files, 500 unpinned clipboard entries, and 101 pinned entries. Pinned entries remain outside the unpinned-history limit. Each added test entry contains 16 KiB of synthetic text.', '',
              '| Build | Hidden, MiB | After file refresh, MiB | Sampled peak, MiB |',
              '| --- | ---: | ---: | ---: |']
    for build, label in [('baseline2', 'Baseline'), ('icon-final', 'Selected')]:
        row = details['capacity'][build]
        lines.append(f'| {label} | {row["hidden"]["total_mib"]:.1f} | {row["after_refresh"]["total_mib"]:.1f} | {row["sampled_peak_mib"]:.1f} |')
    lines += ['', 'These maximum-setting values are separate from the controlled and daily-use comparisons. Detailed icon, repeated-use, and capacity results are in [diagnostics.json](benchmarks/2026-09-19/optimization/diagnostics.json).', '']
    marker = '\nThe selected comparison uses 20 fresh processes:'
    assert marker not in text, 'The final report has already been appended.'
    report.write_text(text.rstrip()+'\n'+'\n'.join(lines))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('root', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('report', type=Path)
    args = parser.parse_args()
    finish(args.root.resolve(), args.output.resolve(), args.report.resolve())
