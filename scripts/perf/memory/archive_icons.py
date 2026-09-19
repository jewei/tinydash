"""Archive the icon candidate and preserve the failed Settings trials."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile

from archive_selected import write_patch
from archive_sources import files, product_bytes, product_path


def archive(root, output):
    review = root/'icon-review'
    review.mkdir(exist_ok=False)
    original_config = (root/'review-source/original/src-tauri/tauri.conf.json').read_text()
    # Keep the earlier selected source and patches: their Settings release failed.
    for before, after in [('selected-source.tar.gz', 'deferred-settings-source.tar.gz'),
                          ('selected-patches', 'deferred-settings-patches')]:
        assert not (output/after).exists(), after
        (output/before).rename(output/after)
    for name in ['ordered', 'icon-final']:
        record = json.loads((root/(name+'.build-measured.json')).read_text())
        source = root/name
        archive_name = 'ordered-settings-source.tar.gz' if name == 'ordered' else 'selected-source.tar.gz'
        product = review/'selected'
        if name == 'icon-final':
            product.mkdir()
        with tarfile.open(output/archive_name, 'w:gz', compresslevel=1) as bundle:
            for path, expected in record['files'].items():
                file = source/path
                assert hashlib.sha256(file.read_bytes()).hexdigest() == expected, (name, path)
                bundle.add(file, arcname=name+'/'+path, recursive=False)
                if name != 'icon-final' or not product_path(path):
                    continue
                data = product_bytes(path, file.read_bytes(), original_config)
                if data is None:
                    continue
                target = product/path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
                target.chmod(file.stat().st_mode)
        shutil.copy2(root/(name+'.build-measured.json'), output/(name+'.build-measured.json'))
    shutil.copytree(root/'review-source/baseline2', review/'baseline2')
    shutil.copytree(review/'baseline2', review/'p2')
    patch = output/'patches/02-p1-to-p2.patch'
    subprocess.run(['git', 'apply', '--check', str(patch.resolve())], cwd=review/'p2', check=True)
    subprocess.run(['git', 'apply', str(patch.resolve())], cwd=review/'p2', check=True)
    # Use the counter's final lint-safe form in the independently reviewable P2.
    target = review/'p2/src-tauri/src/providers/apps.rs'
    text = target.read_text()
    selected = (review/'selected/src-tauri/src/providers/apps.rs').read_text()
    start = text.index('    pub fn icon_payload(')
    end = text.index('\n}\n', start)
    selected_start = selected.index('    pub fn icon_payload(')
    selected_end = selected.index('\n}\n', selected_start)
    target.write_text(text[:start]+selected[selected_start:selected_end]+text[end:])
    patches = output/'selected-patches'
    patches.mkdir()
    check = review/'apply-check'
    shutil.copytree(review/'baseline2', check)
    records = []
    before = 'baseline2'
    for index, after in enumerate(['p2', 'selected'], 1):
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
