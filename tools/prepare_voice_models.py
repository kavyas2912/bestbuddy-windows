#!/usr/bin/env python3
"""Prepare the verified BestBuddy voice pack for Windows and Android builds.

Uses only Python's standard library. Existing complete models win; a matching
local cache is reused before any network request. Downloads are staged and
validated before they replace an incomplete installation.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import tarfile
import tempfile
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen
from uuid import uuid4


HERE = Path(__file__).resolve().parent
MANIFEST = json.loads((HERE / "voice-model-manifest.json").read_text(encoding="utf-8"))


def digest(path: Path) -> str:
    sha = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            sha.update(block)
    return sha.hexdigest().upper()


def complete(path: Path, item: dict) -> bool:
    if item["type"] == "file":
        return path.is_file() and digest(path) == item["sha256"]
    if not path.is_dir():
        return False
    for name, expected in item["files"].items():
        file = path / name
        if not file.is_file() or digest(file) != expected:
            return False
    if item["archive_root"].startswith("vits-piper"):
        language_data = path / "espeak-ng-data" / "lang"
        if not language_data.is_dir() or not any(language_data.iterdir()):
            return False
    return True


def fetch(url: str, destination: Path) -> None:
    if not url.startswith("https://github.com/k2-fsa/sherpa-onnx/releases/download/"):
        raise ValueError("Unexpected model download host")
    destination.parent.mkdir(parents=True, exist_ok=True)
    partial = destination.with_name(destination.name + ".part")
    for attempt in range(3):
        offset = partial.stat().st_size if partial.exists() else 0
        headers = {"User-Agent": "BestBuddy-model-setup/0.6.12"}
        if offset:
            headers["Range"] = f"bytes={offset}-"
        try:
            with urlopen(Request(url, headers=headers), timeout=90) as response:
                append = offset > 0 and response.status == 206
                with partial.open("ab" if append else "wb") as output:
                    shutil.copyfileobj(response, output, 1024 * 1024)
            os.replace(partial, destination)
            return
        except HTTPError as error:
            if error.code == 416 and partial.exists():
                partial.rename(partial.with_name(partial.name + ".stale-" + uuid4().hex))
            if attempt == 2:
                raise
        except OSError:
            if attempt == 2:
                raise
        time.sleep(2 ** attempt)


def candidates(reuse_roots: list[Path], relative: Path):
    for root in reuse_roots:
        yield root / "models" / relative
        yield root / relative


def install_file(name: str, item: dict, models: Path, cache: Path, reuse: list[Path], offline: bool) -> None:
    relative = Path(item["path"])
    destination = models / relative
    if complete(destination, item):
        print(f"[ready] {name}")
        return
    for candidate in candidates(reuse, relative):
        if candidate != destination and complete(candidate, item):
            destination.parent.mkdir(parents=True, exist_ok=True)
            staged = destination.with_name(destination.name + ".stage-" + uuid4().hex)
            shutil.copy2(candidate, staged)
            os.replace(staged, destination)
            print(f"[reuse] {name}: {candidate}")
            return
    cached = cache / "downloads" / Path(item["url"]).name
    if not complete(cached, item):
        if offline:
            raise RuntimeError(f"{name} is missing; offline cache has no verified copy")
        fetch(item["url"], cached)
    if not complete(cached, item):
        raise RuntimeError(f"Downloaded {name} failed SHA-256 validation")
    destination.parent.mkdir(parents=True, exist_ok=True)
    staged = destination.with_name(destination.name + ".stage-" + uuid4().hex)
    shutil.copy2(cached, staged)
    os.replace(staged, destination)
    print(f"[ready] {name}: {destination}")


def extract_archive(archive: Path, item: dict, payload: Path) -> None:
    root = item["archive_root"]
    payload.mkdir(parents=True)
    extracted = 0
    with tarfile.open(archive, "r:bz2") as source:
        for member in source:
            name = member.name.removeprefix("./")
            parts = PurePosixPath(name).parts
            if not parts or parts[0] != root:
                continue
            if any(part in ("", ".", "..") for part in parts):
                raise RuntimeError(f"Unsafe archive path: {member.name}")
            relative = Path(*parts[1:])
            if not relative.parts:
                continue
            target = payload / relative
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            elif member.isfile():
                target.parent.mkdir(parents=True, exist_ok=True)
                stream = source.extractfile(member)
                if stream is None:
                    raise RuntimeError(f"Unreadable archive entry: {member.name}")
                with stream, target.open("wb") as output:
                    shutil.copyfileobj(stream, output, 1024 * 1024)
                extracted += 1
            else:
                raise RuntimeError(f"Unsupported archive entry: {member.name}")
    if extracted == 0:
        raise RuntimeError(f"Archive contains no {root} files")


def install_archive(name: str, item: dict, models: Path, cache: Path, reuse: list[Path], offline: bool) -> None:
    relative = Path(item["directory"])
    destination = models / relative
    if complete(destination, item):
        print(f"[ready] {name}")
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    for candidate in candidates(reuse, relative):
        if candidate != destination and complete(candidate, item):
            with tempfile.TemporaryDirectory(prefix=".buddy-stage-", dir=destination.parent) as temporary:
                staged = Path(temporary) / "payload"
                shutil.copytree(candidate, staged)
                if not complete(staged, item):
                    raise RuntimeError(f"Copied {name} did not validate")
                promote(staged, destination)
            print(f"[reuse] {name}: {candidate}")
            return
    archive = cache / "downloads" / Path(item["url"]).name
    for attempt in range(2):
        if not archive.is_file():
            if offline:
                raise RuntimeError(f"{name} is missing; no cached archive for offline setup")
            fetch(item["url"], archive)
        expected_archive = item.get("archive_sha256")
        if expected_archive and digest(archive) != expected_archive:
            problem = "archive SHA-256 mismatch"
        else:
            try:
                with tempfile.TemporaryDirectory(prefix=".buddy-stage-", dir=destination.parent) as temporary:
                    staged = Path(temporary) / "payload"
                    extract_archive(archive, item, staged)
                    if not complete(staged, item):
                        raise RuntimeError("required files or SHA-256 values do not match")
                    promote(staged, destination)
                print(f"[ready] {name}: {destination}")
                return
            except (OSError, RuntimeError, tarfile.TarError) as error:
                problem = str(error)
        if attempt == 1 or offline:
            raise RuntimeError(f"{name} archive invalid: {problem}")
        quarantined = archive.with_name(archive.name + ".bad-" + uuid4().hex)
        archive.rename(quarantined)
        print(f"[repair] {name}: invalid cache saved at {quarantined}; downloading again")


def promote(staged: Path, destination: Path) -> None:
    backup = None
    if destination.exists():
        backup = destination.with_name(destination.name + ".incomplete-" + uuid4().hex)
        destination.rename(backup)
    try:
        staged.rename(destination)
    except OSError:
        if backup is not None:
            backup.rename(destination)
        raise
    if backup is not None:
        print(f"[repair] Previous incomplete model preserved at {backup}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-root", type=Path, default=HERE.parent)
    parser.add_argument("--cache", type=Path, default=Path(os.getenv("BUDDY_RESOURCES", Path.home() / ".codex" / "buddy-resources")))
    parser.add_argument("--reuse-root", type=Path, action="append", default=[])
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--check-only", action="store_true")
    args = parser.parse_args()
    models = args.target_root.resolve() / "models"
    reuse = [path.resolve() for path in args.reuse_root]
    reuse.append(args.cache.resolve())
    for name, item in MANIFEST.items():
        if args.check_only:
            path = models / (item["directory"] if item["type"] == "archive" else item["path"])
            if not complete(path, item):
                raise RuntimeError(f"{name} model is missing or does not match its SHA-256 manifest: {path}")
            print(f"[verified] {name}")
        elif item["type"] == "archive":
            install_archive(name, item, models, args.cache.resolve(), reuse, args.offline)
        else:
            install_file(name, item, models, args.cache.resolve(), reuse, args.offline)
    keyword = models / "kws" / "keywords.txt"
    if not keyword.is_file() or keyword.stat().st_size < 10:
        if args.check_only:
            raise RuntimeError(f"KWS keyword file missing: {keyword}")
        keyword.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(HERE / "hey-buddy-keywords.txt", keyword)
    print("[verified] voice model pack and Hey Buddy keyword")


if __name__ == "__main__":
    main()
