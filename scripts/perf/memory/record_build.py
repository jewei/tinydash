"""Record the exact prepared source and binary after a successful build."""
import argparse
import hashlib
import json
from pathlib import Path


def record(source):
    original = json.loads(source.with_suffix('.source.json').read_text())
    names = set(original['files']) | {
        'src-tauri/src/memory_probe.rs', 'src/memory-hooks.ts',
        'src/memory-minimal.ts', 'minimal.html',
    }
    files = {name: hashlib.sha256((source/name).read_bytes()).hexdigest()
             for name in sorted(names) if (source/name).is_file()}
    binary = source/'src-tauri/target/release/bundle/macos/TinyDashMemoryOptimization.app/Contents/MacOS/tinydash'
    return {'commit': original['commit'], 'files': files,
            'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
            'tree_sha256': hashlib.sha256(json.dumps(files, sort_keys=True).encode()).hexdigest(),
            'build_command': 'bun run tauri build --bundles app',
            'probe_note': 'Response size audit disabled for the measured workload. All tracing disabled for native latency.'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    args.output.write_text(json.dumps(record(args.source.resolve()), indent=2)+'\n')
