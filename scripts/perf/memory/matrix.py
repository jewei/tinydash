"""Alternate frozen builds across fresh processes. Never compare concurrent apps."""
import argparse
import fcntl
import importlib.util
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('memory_run', HERE/'run.py')
r = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('root', type=Path)
    parser.add_argument('--control', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--runs', type=int, default=5)
    parser.add_argument('--first-trial', type=int, default=1)
    parser.add_argument('--builds', help='Override the build list for a follow-up comparison')
    parser.add_argument('--profiles', default='controlled,daily')
    parser.add_argument('--smoke', action='store_true')
    args = parser.parse_args()
    if args.runs < 1 or args.first_trial < 1:
        parser.error('Run count and first trial must be positive.')
    r.CONTROL = args.control.resolve()
    r.CONTROL.mkdir(parents=True, exist_ok=True)
    r.m.BASE = r.CONTROL
    lock = (r.PROFILE/'.memory-benchmark-lock').open('a')
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    args.output.mkdir(parents=True, exist_ok=True)
    if not (r.CONTROL/'ui').exists():
        subprocess.run(['swiftc','-O',str(r.REPO/'docs/benchmarks/2026-09-19/ui.swift'),'-o',str(r.CONTROL/'ui')],check=True)
    fixture = r.CONTROL/'files'
    if 'daily' in args.profiles:
        for folder in range(100):
            directory = fixture/f'folder-{folder:03}'
            directory.mkdir(parents=True, exist_ok=True)
            for entry in range(500):
                file = directory/f'document-{entry:03}.txt'
                if not file.exists(): file.write_text('TinyDash memory fixture\n')
    for trial in range(args.first_trial, args.first_trial+args.runs):
        for profile in args.profiles.split(','):
            builds = (args.builds.split(',') if args.builds else
                      ['baseline2','p1','p2','p3a','p3b'] if profile == 'controlled' else ['baseline2','p3b'])
            if trial % 2 == 0: builds.reverse()
            for name in builds:
                label = name + ('-matrix-smoke' if args.smoke else '')
                r.run(args.root.resolve()/name,label,'real',profile,trial,args.output,fixture,args.smoke,
                      skip_settings=name in ['p2','p3a'])

if __name__ == '__main__': main()
