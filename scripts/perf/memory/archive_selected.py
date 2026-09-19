"""Archive the selected build and three applicable review patches."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile

from archive_sources import files, product_bytes, product_path


def write_patch(directory, before, after, path):
    result = subprocess.run(['git', 'diff', '--no-index', '--binary', before, after],
                            cwd=directory, capture_output=True)
    assert result.returncode in (0, 1), result.stderr
    lines = []
    for line in result.stdout.decode().splitlines(keepends=True):
        if line.startswith(('diff --git ', '--- ', '+++ ')):
            for side in ['a', 'b']:
                for stage in [before, after]:
                    line = line.replace(side+'/'+stage+'/', side+'/')
        lines.append(line)
    path.write_text(''.join(lines))


def archive(root, output):
    review = root/'selected-review'
    review.mkdir(exist_ok=False)
    output.mkdir(parents=True, exist_ok=True)
    source = root/'final'
    record = json.loads((root/'final.build-measured.json').read_text())
    original_config = (root/'review-source/original/src-tauri/tauri.conf.json').read_text()
    final = review/'selected'
    final.mkdir()
    with tarfile.open(output/'selected-source.tar.gz', 'w:gz', compresslevel=1) as bundle:
        for name, expected in record['files'].items():
            path = source/name
            assert hashlib.sha256(path.read_bytes()).hexdigest() == expected, name
            bundle.add(path, arcname='final/'+name, recursive=False)
            if product_path(name):
                content = product_bytes(name, path.read_bytes(), original_config)
                if content is None: continue
                target = final/name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(content)
                target.chmod(path.stat().st_mode)
    shutil.copy2(root/'final.build-measured.json', output/'final.build-measured.json')
    for name in ['baseline2', 'p1', 'p2']:
        shutil.copytree(root/'review-source'/name, review/name)
    # P1 must include the dependency fix that makes repeated destruction safe.
    for name in ['p1', 'p2']:
        tree = review/name
        shutil.copytree(source/'src-tauri/vendor', tree/'src-tauri/vendor')
        manifest = tree/'src-tauri/Cargo.toml'
        text = manifest.read_text()
        selected = (source/'src-tauri/Cargo.toml').read_text()
        start = selected.index('# Tao 0.35.3 retains')
        manifest.write_text(text.rstrip()+'\n\n'+selected[start:])
        lock = tree/'src-tauri/Cargo.lock'
        text = lock.read_text()
        old = ('name = "tao"\nversion = "0.35.3"\n'
               'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
               'checksum = "d1c93047acf68669466a34690ac58cca7010bd1b201e1ec86f1fd0a75d3dd4a9"\n')
        assert text.count(old) == 1
        lock.write_text(text.replace(old, 'name = "tao"\nversion = "0.35.3"\n'))
    # Include the counter's lint-safe form in the copy-reduction patch.
    target = review/'p2/src-tauri/src/providers/apps.rs'
    text = target.read_text()
    selected = (final/'src-tauri/src/providers/apps.rs').read_text()
    start = text.index('    pub fn icon_payload(')
    end = text.index('\n}\n', start)
    selected_start = selected.index('    pub fn icon_payload(')
    selected_end = selected.index('\n}\n', selected_start)
    target.write_text(text[:start]+selected[selected_start:selected_end]+text[end:])
    patches = output/'selected-patches'
    patches.mkdir()
    check = review/'apply-check'
    shutil.copytree(review/'baseline2', check)
    before = 'baseline2'
    records = []
    for index, after in enumerate(['p1', 'p2', 'selected'], 1):
        patch = patches/f'{index:02}-{before}-to-{after}.patch'
        write_patch(review, before, after, patch)
        subprocess.run(['git', 'apply', '--check', str(patch.resolve())], cwd=check, check=True)
        subprocess.run(['git', 'apply', str(patch.resolve())], cwd=check, check=True)
        assert files(check) == files(review/after), (before, after)
        records.append({'parent': before, 'candidate': after,
                        'patch': patch.name, 'files': files(review/after)})
        before = after
    (patches/'trees.json').write_text(json.dumps(records, indent=2)+'\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('root', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    archive(args.root.resolve(), args.output.resolve())
