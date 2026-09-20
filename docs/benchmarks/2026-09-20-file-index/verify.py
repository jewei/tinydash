"""Verify saved measurement evidence without extracting it or changing the checkout."""

import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import subprocess
import tarfile


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def check(condition, message):
    if not condition:
        raise SystemExit(message)


def members(archive):
    files = {}
    for member in archive.getmembers():
        path = PurePosixPath(member.name)
        check(not path.is_absolute() and ".." not in path.parts, "Unsafe archive path")
        if member.isdir():
            continue
        check(member.isfile(), f"Unexpected archive member: {member.name}")
        check(member.name not in files, f"Duplicate archive member: {member.name}")
        files[member.name] = archive.extractfile(member).read()
    return files


def main():
    manifest = json.loads((HERE / "manifest.json").read_text())
    for name, expected in manifest["files_sha256"].items():
        check(sha256((HERE / name).read_bytes()) == expected, f"File hash differs: {name}")

    with tarfile.open(HERE / "raw-results.tar.gz") as archive:
        raw = members(archive)
    check(set(raw) == set(manifest["raw_record_sha256"]), "Raw record list differs")
    for name, expected in manifest["raw_record_sha256"].items():
        check(sha256(raw[name]) == expected, f"Raw record hash differs: {name}")

    with tarfile.open(HERE / "measured-sources.tar.gz") as archive:
        sources = members(archive)
    for name, expected in manifest["measurement_script_sha256"].items():
        check(sha256(sources[name]) == expected, f"Script hash differs: {name}")

    base = subprocess.check_output(
        ["git", "-C", str(REPO), "archive", manifest["baseline_commit"]]
    )
    with tarfile.open(fileobj=io.BytesIO(base)) as archive:
        baseline = members(archive)
    for variant, build in manifest["native_builds"].items():
        prepared = dict(baseline)
        prefix = f"overlays/{variant}/"
        prepared.update(
            (name.removeprefix(prefix), data)
            for name, data in sources.items()
            if name.startswith(prefix)
        )
        for name, expected in build["source_sha256"].items():
            check(name in prepared, f"Missing source: {variant}/{name}")
            check(sha256(prepared[name]) == expected, f"Source hash differs: {variant}/{name}")
        print(f"{variant}: {len(build['source_sha256'])} source hashes match")

    saved = json.loads((HERE / "native-summary.json").read_text())
    check(saved == json.loads(raw["native-summary.json"]), "Native summary values differ")
    saved = json.loads((HERE / "provider-summary.json").read_text())
    check(saved == json.loads(raw["provider/summary.json"]), "Provider summary values differ")
    print(f"Verified {len(raw)} raw records, archives, scripts, sources, and summaries")


if __name__ == "__main__":
    main()
