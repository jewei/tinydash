"""Add deterministic native failure injection to an isolated functional build."""
import argparse
from pathlib import Path

parser=argparse.ArgumentParser()
parser.add_argument('source',type=Path)
args=parser.parse_args()
source=args.source.resolve()
assert source != Path(__file__).resolve().parents[3] and not (source/'.git').exists()
probe=source/'src-tauri/src/memory_probe.rs'
value=probe.read_text()
value=value.replace('static CLIPBOARD:', 'pub static FAIL_SHORTCUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);\n\nstatic CLIPBOARD:',1)
old='        "clipboard" => {'
assert value.count(old)==1
value=value.replace(old,'''        "fail-shortcut" => {
            FAIL_SHORTCUT.store(value["enabled"].as_bool().unwrap(), std::sync::atomic::Ordering::Release);
            Ok(Value::Null)
        }
'''+old)
probe.write_text(value)
preferences=source/'src-tauri/src/launcher/preferences.rs'
value=preferences.read_text()
old='''    fn register(&self, shortcut: &str) -> Result<(), String> {
        self.0'''
assert value.count(old)==1
value=value.replace(old,'''    fn register(&self, shortcut: &str) -> Result<(), String> {
        if crate::memory_probe::FAIL_SHORTCUT.load(std::sync::atomic::Ordering::Acquire) {
            return Err("Could not restore the shortcut: injected native registration failure.".into());
        }
        self.0''')
preferences.write_text(value)
