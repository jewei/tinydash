"""Record tracked and untracked source without changing the user's Git index."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    target = args.directory.resolve()
    target.mkdir(parents=True, exist_ok=False)
    paths = subprocess.check_output(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=repo).decode().split('\0')
    manifest = {}
    for name in sorted(set(filter(None, paths))):
        source = repo/name
        if not source.is_file(): continue
        destination = target/name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
        manifest[name] = hashlib.sha256(source.read_bytes()).hexdigest()
    (target/'node_modules').symlink_to(repo/'node_modules', target_is_directory=True)
    record = {
        'commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(),
        'status': subprocess.check_output(['git', 'status', '--porcelain'], cwd=repo, text=True),
        'files': manifest,
    }
    target.with_suffix('.source.json').write_text(json.dumps(record, indent=2)+'\n')
    target.with_suffix('.patch').write_bytes(subprocess.check_output(['git', 'diff', 'HEAD', '--binary'], cwd=repo))
    print(target)

if __name__ == '__main__': main()
