"""Apply a diagnostic overlay to an isolated source snapshot. Never edit the working app."""
import argparse
import json
import shutil
from pathlib import Path

IDENTIFIER = 'dev.tinydash.memoryoptimization.20260919'

def replace(path, old, new):
    text = path.read_text()
    if text.count(old) != 1:
        raise RuntimeError(f'Expected one overlay location in {path}: {old}')
    path.write_text(text.replace(old, new))

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('source', type=Path)
    args = parser.parse_args()
    source = args.source.resolve()
    here = Path(__file__).resolve().parent
    repo = here.parents[2]
    if source == repo or (source / '.git').exists():
        raise SystemExit('Use an isolated source copy without .git.')
    shutil.copy2(here/'probe.rs', source/'src-tauri/src/memory_probe.rs')
    shutil.copy2(here/'memory-hooks.ts', source/'src/memory-hooks.ts')
    shutil.copy2(here/'minimal.ts', source/'src/memory-minimal.ts')
    (source/'minimal.html').write_text('<!doctype html><html><head><meta charset="utf-8"><title>TinyDash minimal control</title></head><body><input aria-label="Minimal control"><script type="module" src="/src/memory-minimal.ts"></script></body></html>\n')
    replace(source/'src/index.tsx', 'import { render }', 'import "./memory-hooks";\nimport { render }')
    replace(source/'src/bridge.ts', 'import { invoke }', 'import { memorySearch } from "./memory-hooks";\nimport { invoke }')
    replace(source/'src/bridge.ts', 'invoke<SearchResponse>("search", { query, mode })', 'memorySearch(query, mode) as Promise<SearchResponse>')
    with (source/'src/bridge.ts').open('a') as bridge:
        bridge.write('\n// Isolated diagnostic access for native lifecycle tests.\nObject.assign(window, { __memoryBackend: backend });\n')
    replace(source/'vite.config.ts', 'build: { target:', 'build: { rollupOptions: { input: ["index.html", "minimal.html"] }, target:')
    lib = source/'src-tauri/src/lib.rs'
    replace(lib, 'mod currency;', 'mod memory_probe;\nmod currency;')
    replace(lib, 'launcher::scan_apps(app.handle());', 'launcher::scan_apps(app.handle());\n            memory_probe::install(app.handle().clone());')
    replace(lib, 'launcher::launcher_ready,', 'memory_probe::memory_reply,\n            launcher::launcher_ready,')
    replace(source/'src-tauri/src/platform/macos.rs', 'pub fn clipboard_snapshot(previous: Option<u64>) -> anyhow::Result<Option<(u64, Option<String>)>> {', '''pub fn clipboard_snapshot(previous: Option<u64>) -> anyhow::Result<Option<(u64, Option<String>)>> {
    if std::env::var("TINYDASH_MEMORY_DAILY").as_deref() == Ok("1") {
        return Ok(crate::memory_probe::clipboard_fixture(previous));
    }''')
    replace(lib, '.build(tauri::generate_context!())', '''.build({
            let mut context = tauri::generate_context!();
            if std::env::var("TINYDASH_MEMORY_PAGE").as_deref() == Ok("minimal") {
                context.config_mut().app.windows[0].url = tauri::WebviewUrl::App("minimal.html".into());
            }
            context
        })''')
    config = source/'src-tauri/tauri.conf.json'
    value = json.loads(config.read_text())
    value['productName'] = 'TinyDashMemoryOptimization'
    value['identifier'] = IDENTIFIER
    config.write_text(json.dumps(value, indent=2)+'\n')
    print(source)

if __name__ == '__main__':
    main()
