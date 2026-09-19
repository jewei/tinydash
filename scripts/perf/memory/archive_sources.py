"""Preserve measured source and make review patches without changing Git state."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import shutil
import subprocess
import tarfile

REPO=Path(__file__).resolve().parents[3]
BUILDS=['baseline2','p1','p2','p3a','p3b']
OVERLAY_FILES={'src-tauri/src/memory_probe.rs','src/memory-hooks.ts','src/memory-minimal.ts'}

def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()

def replace(text,old,new):
    assert text.count(old)==1,old
    return text.replace(old,new)

def product_bytes(name,data,original_config):
    if name in OVERLAY_FILES:return None
    if name=='src-tauri/tauri.conf.json':
        # Keep the original formatting. Only the image policy changes in P3.
        config=json.loads(data)
        original=json.loads(original_config)
        return original_config.replace(original['app']['security']['csp'],config['app']['security']['csp']).encode()
    text=data.decode() if name.endswith(('.rs','.tsx','.ts')) else None
    if name=='src-tauri/src/lib.rs':
        text=replace(text,'mod memory_probe;\n','')
        text=replace(text,'\n            memory_probe::install(app.handle().clone());','')
        text=replace(text,'memory_probe::memory_reply,\n            launcher::launcher_ready,','launcher::launcher_ready,')
        text=replace(text,'''.build({
            let mut context = tauri::generate_context!();
            if std::env::var("TINYDASH_MEMORY_PAGE").as_deref() == Ok("minimal") {
                context.config_mut().app.windows[0].url = tauri::WebviewUrl::App("minimal.html".into());
            }
            context
        })''','.build(tauri::generate_context!())')
    elif name=='src-tauri/src/platform/macos.rs':
        text=replace(text,'''
    if std::env::var("TINYDASH_MEMORY_DAILY").as_deref() == Ok("1") {
        return Ok(crate::memory_probe::clipboard_fixture(previous));
    }''','')
    elif name=='src/bridge.ts':
        text=replace(text,'import { memorySearch } from "./memory-hooks";\n','')
        text=replace(text,'memorySearch(query, mode) as Promise<SearchResponse>','invoke<SearchResponse>("search", { query, mode })')
        text=replace(text,'\n// Isolated diagnostic access for native lifecycle tests.\nObject.assign(window, { __memoryBackend: backend });\n','')
    elif name=='src/index.tsx':
        text=replace(text,'import "./memory-hooks";\n','')
    return text.encode() if text is not None else data

def product_path(name):return name.startswith(('src/','src-tauri/','tests/'))

def files(directory):
    return {str(path.relative_to(directory)):digest(path) for path in directory.rglob('*') if path.is_file()}

def archive(root,output):
    output.mkdir(parents=True,exist_ok=True)
    review=root/'review-source'
    review.mkdir(exist_ok=False)
    original=review/'original'
    original.mkdir()
    baseline=json.loads((root/'baseline-source.json').read_text())
    content=subprocess.check_output(['git','archive',baseline['commit']],cwd=REPO)
    with tarfile.open(fileobj=io.BytesIO(content)) as source:
        source.extractall(original,filter='data')
    subprocess.run(['git','apply',str(root/'baseline.patch')],cwd=original,check=True)
    for name,expected in baseline['files'].items():
        target=original/name
        if not target.is_file() or digest(target)!=expected:
            source=next((root/build/name for build in ['baseline2','baseline']
                         if (root/build/name).is_file() and digest(root/build/name)==expected),None)
            assert source is not None,('Cannot recover original source',name)
            target.parent.mkdir(parents=True,exist_ok=True)
            shutil.copy2(source,target)
    assert files(original)==baseline['files'],'Original source must match all recorded file hashes'
    original_config=(original/'src-tauri/tauri.conf.json').read_text()
    metadata=output/'sources'
    metadata.mkdir(exist_ok=True)
    for filename in ['baseline-source.json','baseline.patch']:
        shutil.copy2(root/filename,metadata/filename)
    with tarfile.open(output/'measured-sources.tar.gz','w:gz',compresslevel=1) as bundle:
        for name in baseline['files']:
            bundle.add(original/name,arcname='original/'+name,recursive=False)
        for build in BUILDS:
            manifest=root/(build+'.build-measured.json')
            record=json.loads(manifest.read_text())
            shutil.copy2(manifest,metadata/manifest.name)
            product=review/build
            product.mkdir()
            for name,expected in record['files'].items():
                source=root/build/name
                assert digest(source)==expected,(build,name,'Measured source changed')
                bundle.add(source,arcname=build+'/'+name,recursive=False)
                if not product_path(name):continue
                data=product_bytes(name,source.read_bytes(),original_config)
                if data is None:continue
                target=product/name
                target.parent.mkdir(parents=True,exist_ok=True)
                target.write_bytes(data)
                target.chmod(source.stat().st_mode)
    expected={name:sha for name,sha in baseline['files'].items() if product_path(name)}
    assert files(review/'baseline2')==expected,'Removing the overlay must recover the original product exactly'
    verified=review/'verified'
    verified.mkdir()
    paths=subprocess.check_output(['git','ls-files','-z','--cached','--others','--exclude-standard'],cwd=REPO).decode().split('\0')
    for name in sorted(set(paths)):
        source=REPO/name
        if not product_path(name) or not source.is_file():continue
        target=verified/name
        target.parent.mkdir(parents=True,exist_ok=True)
        shutil.copy2(source,target)
    patches=output/'patches'
    patches.mkdir(exist_ok=True)
    check=review/'apply-check'
    shutil.copytree(review/'baseline2',check)
    before='baseline2'
    records=[]
    for index,after in enumerate(['p1','p2','p3a','p3b','verified'],1):
        result=subprocess.run(['git','diff','--no-index','--binary',before,after],cwd=review,capture_output=True)
        assert result.returncode in [0,1],result.stderr
        text=result.stdout.decode()
        lines=[]
        for line in text.splitlines(keepends=True):
            if line.startswith(('diff --git ','--- ','+++ ')):
                for side in ['a','b']:
                    for stage in [before,after]:line=line.replace(side+'/'+stage+'/',side+'/')
            lines.append(line)
        patch=patches/f'{index:02}-{before}-to-{after}.patch'
        patch.write_text(''.join(lines))
        if patch.stat().st_size:
            subprocess.run(['git','apply','--check',str(patch.resolve())],cwd=check,check=True)
            subprocess.run(['git','apply',str(patch.resolve())],cwd=check,check=True)
        assert files(check)==files(review/after),(before,after,'Patch must reproduce its complete product tree')
        records.append({'parent':before,'candidate':after,'patch':patch.name,
                        'files':files(review/after)})
        before=after
    (patches/'trees.json').write_text(json.dumps(records,indent=2)+'\n')
    print(output)

if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('root',type=Path)
    parser.add_argument('output',type=Path)
    args=parser.parse_args()
    archive(args.root.resolve(),args.output.resolve())
