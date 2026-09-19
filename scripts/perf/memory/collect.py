"""Collect raw evidence and logs after all native measurement jobs finish."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import tarfile


def collect(root, output):
    output.mkdir(parents=True, exist_ok=True)
    folders = ['results', 'comparison', 'comparison-before-trace-fix', 'smoke',
               'icon-results', 'settings-allocations', 'settings-allocations-fixed',
               'final-comparison', 'settings-crash', 'icon-final-smoke',
               'icon-comparison', 'repeated-results-icon-final-before-reopen-fix',
               'repeated-results-icon-final', 'capacity-results-icon-final']
    raw = []
    for name in folders:
        assert (root/name).is_dir(), name
        raw.extend(path for path in (root/name).rglob('*') if path.is_file())
    raw.extend(root/name for name in ['settings-growth.json', 'settings-growth-fixed.json',
               'tao-0.35.3.crate', 'run_final_checks.py', 'run_remaining.py',
               'disk-cleanup.json', 'disk-cleanup-final-cache.json',
               'disk-cleanup-completed-checks.json', 'final-native-app.log',
               'repeat_cleanup_pair.py', 'run_ordered_checks.py',
               'run_icon_validation.py', 'run_icon_native.py', 'run_icon_remaining.py',
               'settings-release-report-before-deferral.md'])
    with tarfile.open(output/'raw-records.tar.gz', 'w:gz', compresslevel=1) as bundle:
        for path in sorted(raw):
            bundle.add(path, arcname=str(path.relative_to(root)), recursive=False)
    logs = sorted(set(root.glob('*build*.log')) | set(Path('/tmp').glob('tinydash-memory*.log')))
    with tarfile.open(output/'logs.tar.gz', 'w:gz', compresslevel=1) as bundle:
        for path in logs:
            bundle.add(path, arcname=('build/' if path.parent == root else 'checks/')+path.name,
                       recursive=False)
    here = Path(__file__).resolve().parent
    repo = here.parents[2]
    with tarfile.open(output/'tools.tar.gz', 'w:gz', compresslevel=1) as bundle:
        for path in sorted(here.iterdir()):
            if path.is_file():
                bundle.add(path, arcname='scripts/perf/memory/'+path.name, recursive=False)
        for name in ['measure.py', 'ui.swift']:
            path = repo/'docs/benchmarks/2026-09-19'/name
            bundle.add(path, arcname='docs/benchmarks/2026-09-19/'+name, recursive=False)
    # Keep the small final diagnostics readable without extracting the archive.
    for name in ['settings-growth-fixed.json', 'final.build-measured.json',
                 'icon-final.build-measured.json', 'disk-cleanup.json',
                 'disk-cleanup-final-cache.json', 'disk-cleanup-completed-checks.json']:
        shutil.copy2(root/name, output/name)
    files = {str(path.relative_to(output)): hashlib.sha256(path.read_bytes()).hexdigest()
             for path in sorted(output.rglob('*')) if path.is_file() and path.name != 'SHA256.json'}
    (output/'SHA256.json').write_text(json.dumps(files, indent=2)+'\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('root', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    collect(args.root.resolve(), args.output.resolve())
