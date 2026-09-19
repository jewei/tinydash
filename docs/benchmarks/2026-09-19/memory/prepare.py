"""Prepare the isolated, debug-only copy used by the memory investigation."""
import argparse
import io
import json
import shutil
import subprocess
import tarfile
from pathlib import Path

COMMIT = 'bccaa245284cc22a7bc0c96d3083dace3a1950ca'
IDENTIFIER = 'dev.tinydash.memoryprobe.20260919'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path, help='A new temporary directory')
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    repo = here.parents[3]
    base = args.directory.expanduser().resolve()
    profile = Path.home() / 'Library/Application Support' / IDENTIFIER
    if profile.exists():
        raise SystemExit('The isolated test profile already exists: ' + str(profile))
    if not (repo / 'node_modules').is_dir():
        raise SystemExit('Install the repository frontend dependencies first.')
    base.mkdir(parents=True, exist_ok=False)
    source = base / 'source'
    source.mkdir()
    archive = subprocess.check_output(['git', 'archive', COMMIT], cwd=repo)
    with tarfile.open(fileobj=io.BytesIO(archive)) as handle:
        handle.extractall(source, filter='data')
    (source / 'node_modules').symlink_to(repo / 'node_modules', target_is_directory=True)
    for name in ['probe.py', 'probe.patch']:
        shutil.copyfile(here / name, base / name)
    for name in ['measure.py', 'ui.swift']:
        shutil.copyfile(here.parent / name, base / name)
    subprocess.run(['git', 'apply', '--check', str(base / 'probe.patch')], cwd=source, check=True)
    subprocess.run(['git', 'apply', str(base / 'probe.patch')], cwd=source, check=True)
    profile.mkdir(parents=True, exist_ok=False)
    (profile / 'settings.json').write_text(json.dumps({
        'fileSearchRoots': [],
        'fileWatchEnabled': False,
        'currencyRatesEnabled': False,
        'clipboardHistoryEnabled': False,
        'shortcut': 'Control+Alt+Shift+Space',
    }) + '\n')
    (base / 'results').mkdir()
    (base / 'profile-path.txt').write_text(str(profile) + '\n')
    print(base)


if __name__ == '__main__':
    main()
