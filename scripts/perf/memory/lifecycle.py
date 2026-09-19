"""Native Settings checks through the isolated release diagnostic overlay."""
import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('memory_run', HERE/'run.py')
r = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)

def settings(script): return r.evaluate(script, 'settings')
def open_settings():
    r.command('settings')
    r.until(lambda: settings('return !!document.querySelector(".settings-fields") && !!window.__memoryBackend;'))

def section(name):
    settings(f'[...document.querySelectorAll("nav button")].find(button=>button.textContent.trim()==={json.dumps(name)}).click(); return true;')

def checkbox():
    return settings('return [...document.querySelectorAll("input[role=switch]")].find(input=>input.getAttribute("aria-label")==="Clear the search each time").checked;')

def edit_checkbox():
    return settings('const input=[...document.querySelectorAll("input[role=switch]")].find(input=>input.getAttribute("aria-label")==="Clear the search each time"); input.click(); return input.checked;')

def close_settings():
    r.command('close', window='settings')
    r.until(lambda: not any(w['label']=='settings' for w in r.command('status')['windows']))

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('source', type=Path)
    parser.add_argument('--control', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--inject-shortcut-failure', action='store_true')
    args = parser.parse_args()
    r.CONTROL = args.control.resolve()
    r.m.BASE = r.CONTROL
    lock=(r.PROFILE/'.memory-benchmark-lock').open('a')
    r.fcntl.flock(lock,r.fcntl.LOCK_EX|r.fcntl.LOCK_NB)
    native = r.CONTROL/'native'
    if not native.exists():
        subprocess.run(['swiftc','-O',str(HERE/'native.swift'),'-o',str(native)],check=True)
    reset = r.reset_profile
    def profile(daily, fixture):
        reset(daily, fixture)
        path=r.PROFILE/'settings.json'
        value=json.loads(path.read_text())
        value['categoryShortcuts']=[{'mode':'apps','shortcut':'Control+Alt+Shift+KeyA'}]
        path.write_text(json.dumps(value)+'\n')
    r.reset_profile=profile
    pid, _ = r.launch(args.source.resolve(), 'real', False, r.CONTROL/'files')
    record = {'checks': [], 'started': time.strftime('%Y-%m-%dT%H:%M:%S%z')}
    def passed(name):
        record['checks'].append(name)
        print('PASS '+name, flush=True)
    try:
        r.until(lambda: r.command('status')['ready'])
        for cycle in range(20):
            open_settings()
            section('Shortcut')
            value = edit_checkbox()
            if cycle == 1:
                section('File search')
                settings('const select=document.querySelector("#folder-mode"); select.value="custom"; select.dispatchEvent(new Event("change",{bubbles:true})); return true;')
                # Test raw number input without invoking Save validation.
                settings('''const input=document.querySelector('input[aria-label="File limit"]'); input.value=""; input.dispatchEvent(new Event("input",{bubbles:true})); return true;''')
            elif cycle == 2:
                section('Search')
                r.until(lambda: settings('return document.querySelector("select[size]")?.options.length > 0;'))
                settings('const list=document.querySelector("select[size]"); list.value=list.options[0].value; list.dispatchEvent(new Event("change",{bubbles:true})); const text=document.querySelector("textarea"); text.focus(); text.value="  unfinished alias\\n\\n"; text.dispatchEvent(new Event("input",{bubbles:true})); return true;')
            elif cycle in [3,4]:
                # Restore valid settings before testing Save.
                settings('window.__memorySave=window.__memoryBackend.saveSettings; window.__memoryBackend.saveSettings=async(value)=>{await new Promise(resolve=>setTimeout(resolve,250)); '+('throw new Error("Injected Save failure");' if cycle==4 else 'return window.__memorySave(value);')+'}; [...document.querySelectorAll("button")].find(button=>button.textContent.trim()==="Save changes").click(); return true;')
            elif cycle == 5:
                settings('[...document.querySelectorAll("button")].find(button=>button.textContent.trim()==="Record new").click(); return true;')
                r.until(lambda: settings('return document.body.textContent.includes("Press your new shortcut");'))
            elif cycle in [6,7,8]:
                delay = 6000 if cycle == 7 else 250
                settings('window.__memoryCapture=window.__memoryBackend.captureSettingsSession; window.__memoryBackend.captureSettingsSession=async(...args)=>{'+('throw new Error("Injected capture failure");' if cycle==6 else f'await new Promise(resolve=>setTimeout(resolve,{delay})); return window.__memoryCapture(...args);')+'}; return true;')
                r.command('close', window='settings')
                if cycle == 8:
                    r.until(lambda: settings('return document.querySelector("main").inert;'))
                    r.command('settings')
                r.until(lambda: settings('return !document.querySelector("main").inert;'), timeout=10)
                assert any(w['label']=='settings' and w['visible'] for w in r.command('status')['windows'])
                if cycle == 7:
                    value = edit_checkbox()
                    time.sleep(1.5)
                    assert checkbox() == value
                settings('window.__memoryBackend.captureSettingsSession=window.__memoryCapture; return true;')
                passed(['capture failure preserves window','capture timeout rejects late state','reopen cancels pending close'][cycle-6])
            elif cycle == 9:
                section('Appearance')
                settings('''document.querySelector('input[name="appearance"][value="dark"]').click(); return true;''')
            elif cycle in [10,11]:
                section('Privacy')
                settings('window.__memoryBackend.importSettings=async()=>({settings:{...(await window.__memoryBackend.settings()).settings,clearQueryOnOpen:'+json.dumps(value)+'},appearance:"compact",ignoredKeys:["memoryTest"]}); [...document.querySelectorAll("button")].find(button=>button.textContent.trim()==="Import settings").click(); return true;')
                r.until(lambda: settings('return !!document.querySelector(".settings-import-preview");'))
                if cycle == 11:
                    settings('[...document.querySelectorAll("button")].find(button=>button.textContent.trim()==="Apply import").click(); window.__memorySave=window.__memoryBackend.saveSettings; window.__memoryBackend.saveSettings=async(value)=>{await new Promise(resolve=>setTimeout(resolve,250)); return window.__memorySave(value);}; [...document.querySelectorAll("button")].find(button=>button.textContent.trim()==="Save changes").click(); return true;')
            elif cycle == 13:
                settings('window.__memoryFinish=window.__memoryBackend.finishSettingsClose; window.__memoryBackend.finishSettingsClose=async()=>{throw new Error("Injected release failure");}; return true;')
                r.command('close',window='settings')
                r.until(lambda: settings('return !document.querySelector("main").inert && document.body.textContent.includes("Injected release failure");'))
                assert checkbox() == value
                settings('window.__memoryBackend.finishSettingsClose=window.__memoryFinish; return true;')
                passed('release failure keeps the live session')
            elif cycle == 14:
                section('File search')
                settings('const select=document.querySelector("#folder-mode"); select.value="default"; select.dispatchEvent(new Event("change",{bubbles:true})); return true;')
                raw = settings('const input=document.querySelector("#excluded-folders"); input.value="  "+input.value+"\\n\\n"; input.dispatchEvent(new Event("input",{bubbles:true})); return input.value;')
            elif cycle == 15:
                r.command('close',window='settings')
                r.command('settings')
                r.until(lambda: settings('return !!document.querySelector(".settings-fields") && !document.querySelector("main").inert;'))
                time.sleep(.3)
                assert any(w['label']=='settings' and w['visible'] for w in r.command('status')['windows'])
                assert checkbox() == value
                passed('immediate native reopen wins over closure')
            elif cycle == 16 and args.inject_shortcut_failure:
                settings('[...document.querySelectorAll("button")].find(button=>button.textContent.trim()==="Record new").click(); return true;')
                r.until(lambda: settings('return document.body.textContent.includes("Press your new shortcut");'))
                r.command('fail-shortcut',enabled=True)
                try:
                    r.command('close',window='settings')
                    r.until(lambda: settings('return !document.querySelector("main").inert && document.body.textContent.includes("Could not");'))
                    assert checkbox() == value
                    assert any(w['label']=='settings' and w['visible'] for w in r.command('status')['windows'])
                    passed('native shortcut registration failure prevents destruction and preserves edits')
                finally:
                    r.command('fail-shortcut',enabled=False)
            elif cycle == 17:
                if not settings('return [...document.querySelectorAll("button")].some(button=>button.textContent.trim()==="Discard");'):
                    edit_checkbox()
                settings('[...document.querySelectorAll("button")].find(button=>button.textContent.trim()==="Discard").click(); return true;')
                value=checkbox()
                assert settings('return (await window.__memoryBackend.settings()).settings.clearQueryOnOpen;') == value
                passed('Discard replaces the retained draft with saved values')
            elif cycle == 18:
                section('Privacy')
                scroll = settings('const area=document.querySelector(".settings-scroll"); area.scrollTop=200; return area.scrollTop;')
                assert scroll > 0
            close_settings()
            if cycle == 12:
                r.evaluate('const info=await window.__memoryBackend.settings(); await window.__memoryBackend.saveSettings({...info.settings,fileSearchLimit:42000}); return true;')
            open_settings()
            if cycle == 1:
                assert settings('''return document.querySelector('input[aria-label="File limit"]').value;''') == ''
                settings('''const input=document.querySelector('input[aria-label="File limit"]'); input.value="50000"; input.dispatchEvent(new Event("input",{bubbles:true})); return true;''')
                settings('const select=document.querySelector("#folder-mode"); select.value="off"; select.dispatchEvent(new Event("change",{bubbles:true})); return true;')
                passed('invalid raw number survives destruction')
            elif cycle == 2:
                assert settings('return document.querySelector("textarea").value;') == '  unfinished alias\n\n'
                passed('open child editor and raw alias survive destruction')
            if cycle == 4:
                assert settings('return document.body.textContent.includes("Injected Save failure");')
                passed('failed Save state survives destruction')
            elif cycle == 9:
                assert settings('return document.documentElement.dataset.appearance;') == 'dark'
                passed('immediately saved appearance survives destruction')
            elif cycle == 10:
                assert settings('return !!document.querySelector(".settings-import-preview") && document.body.textContent.includes("memoryTest");')
                settings('[...document.querySelectorAll(".settings-import-preview button")].find(button=>button.textContent.trim()==="Cancel").click(); return true;')
                passed('unapplied import preview survives destruction')
            elif cycle == 11:
                assert settings('return document.documentElement.dataset.appearance;') == 'compact'
                assert settings('return localStorage.getItem("tinydash.appearance");') == 'compact'
                passed('close waits for imported appearance Save continuation')
            elif cycle == 12:
                section('File search')
                settings('const select=document.querySelector("#folder-mode"); select.value="custom"; select.dispatchEvent(new Event("change",{bubbles:true})); return true;')
                assert settings('''return document.querySelector('input[aria-label="File limit"]').value;''') == '42000'
                settings('const select=document.querySelector("#folder-mode"); select.value="off"; select.dispatchEvent(new Event("change",{bubbles:true})); return true;')
                passed('external settings merge with the local editing session')
            elif cycle == 14:
                assert settings('return document.querySelector("#excluded-folders").value;') == raw
                settings('const select=document.querySelector("#folder-mode"); select.value="off"; select.dispatchEvent(new Event("change",{bubbles:true})); return true;')
                passed('raw formatting survives unchanged saved values')
            elif cycle == 18:
                r.until(lambda: settings('return document.querySelector(".settings-scroll").scrollTop;') == scroll)
                passed('section and scroll position survive destruction')
            section('Shortcut')
            assert checkbox() == value
            close_settings()
            # Tauri removes the window before the WindowServer close animation
            # ends. The shared UI helper must see a fully hidden application
            # before it decides whether to send the global shortcut.
            r.until(lambda: not r.m.ui('status', pid)['onscreen'])
            r.m.ui('show', pid)
            assert any(w['label']=='main' and w['visible'] for w in r.command('status')['windows'])
            assert r.evaluate('return document.activeElement?.getAttribute("role") === "combobox";')
            r.command('hide')
            r.until(lambda: not r.m.ui('status', pid)['onscreen'])
            subprocess.run([str(native),'category'],check=True)
            r.until(lambda: any(w['label']=='main' and w['visible'] for w in r.command('status')['windows']))
            r.until(lambda: r.evaluate('''return document.querySelector('nav[aria-label="Search categories"] button[aria-pressed="true"]')?.textContent.includes("Apps");'''))
            r.command('hide')
            if cycle in [0,9,19]:
                record.setdefault('process_counts',[]).append({'cycle':cycle+1,'members':r.m.members(pid)})
            passed(f'cycle {cycle+1}: draft, native destruction, shortcut, focus')
        record['final_processes'] = r.m.members(pid)
    except Exception as error:
        record['error'] = str(error)
        raise
    finally:
        if pid in r.m.processes(): r.m.stop(pid)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(record, indent=2)+'\n')

if __name__ == '__main__': main()
