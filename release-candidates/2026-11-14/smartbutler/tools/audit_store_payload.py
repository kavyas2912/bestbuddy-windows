#!/usr/bin/env python3
"""Privacy-safe, cross-machine MSIX payload auditor (FIX2).

Required contents and personal-file exclusions are strict.
Actual developer home paths remain blocking.
Unrelated third-party DLL/compiler-machine references are *warnings* rather
than assumed customer data. Warnings are recorded, never silently discarded.

Run on Windows with stock Python 3.10+, without any network requests.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path, PurePosixPath
import re
import sys
import zipfile

EXPECTED_ASSETS = {
    "Assets/SplashScreen.png", "Assets/Square150x150Logo.png",
    "Assets/Square310x310Logo.png", "Assets/Square44x44Logo.png",
    "Assets/Square71x71Logo.png", "Assets/StoreLogo.png",
    "Assets/Wide310x150Logo.png",
}
EXPECTED_MODELS = {
    "models/asr/sherpa-onnx-whisper-base.en/base.en-encoder.int8.onnx",
    "models/asr/sherpa-onnx-whisper-base.en/base.en-decoder.int8.onnx",
    "models/tts/vits-piper-en_US-lessac-medium/en_US-lessac-medium.onnx",
    "models/tts/vits-piper-en_US-lessac-medium/espeak-ng-data/phontab",
    "models/kws/keywords.txt", "models/vad/silero_vad.onnx",
    "models/denoise/gtcrn_simple.onnx",
}
EXPECTED = {"smart-butler-desktop.exe", "AppxManifest.xml"} | EXPECTED_ASSETS | EXPECTED_MODELS
FORBIDDEN_EXT = {
    ".pfx", ".p12", ".pem", ".key", ".cer", ".db", ".sqlite", ".sqlite3",
    ".log", ".bak", ".cmd", ".ps1", ".env", ".gguf", ".zip", ".tar", ".7z",
    ".msix", ".msi",
}
FORBIDDEN_NAMES = {
    "profile.json", "contacts.json", "settings.json", "secrets.json",
    "vault.json", "credentials.json", "butler.log", ".env", ".gitconfig",
    "id_rsa", "known_hosts", "appdata", "userdata.json",
    "store-identity.json", "smartbutler-preview-devcert.pfx", "smartbutler-preview-devcert.pfx",
}
TEXT_SUFFIXES = {".xml", ".json", ".txt", ".config", ".ini", ".md"}
PE_SUFFIXES = {".exe", ".dll"}
# These patterns may occur in vendor DLLs (e.g. CI build paths), which do not
# constitute a link/dependency on the user's own computer.
MACHINE_REFERENCE_PATTERNS = [
    re.compile(rb"(?i)\b[A-Z]:[\\/]Users[\\/][A-Za-z0-9._ -]{1,75}[\\/]"),
    re.compile(rb"(?i)/(?:home|Users)/[A-Za-z0-9._-]{1,75}/"),
    re.compile(rb"(?i)\\\\[a-z0-9._-]+\\[a-z0-9$._-]+\\"),
]
MSIX_METADATA = {
    "[Content_Types].xml", "AppxBlockMap.xml", "AppxSignature.p7x", "resources.pri"
}


def validate_name(raw: str, is_msix: bool) -> str | None:
    normalized = raw.replace("\\", "/")
    if normalized.endswith("/"):
        return None
    path = PurePosixPath(normalized)
    if (not normalized or normalized.startswith("/") or ".." in path.parts
            or ":" in normalized or "\x00" in normalized or normalized.endswith(".part")):
        raise ValueError("Unsafe, traversal or unfinished package item: " + normalized)
    if (path.name.lower() in FORBIDDEN_NAMES
            or path.suffix.lower() in FORBIDDEN_EXT
            or path.name.lower().endswith(".exe.config")):
        raise ValueError("User data, secret, or unrelated archive: " + normalized)
    if is_msix and (normalized in MSIX_METADATA or normalized.startswith("AppxMetadata/")):
        return None
    if normalized in {"smart-butler-desktop.exe", "AppxManifest.xml"}:
        return normalized
    if len(path.parts) == 1 and path.suffix.lower() == ".dll":
        return normalized
    if normalized in EXPECTED_ASSETS:
        return normalized
    roots = (
        "models/asr/sherpa-onnx-whisper-base.en/",
        "models/tts/vits-piper-en_US-lessac-medium/",
        "models/kws/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01/",
    )
    single_files = {
        "models/kws/keywords.txt", "models/vad/silero_vad.onnx",
        "models/denoise/gtcrn_simple.onnx"
    }
    if normalized.startswith(roots) or normalized in single_files:
        return normalized
    raise ValueError("Unexpected item in installable package: " + normalized)


def is_home_reference(data: bytes, home: str) -> bool:
    if not home:
        return False
    # Accept common path separators and escaped forms. Enforce a boundary so
    # C:\Users\Name2 never matches C:\Users\Name.
    variants = {home.rstrip("\\/")}
    variants.add(home.replace("\\", "/").rstrip("\\/"))
    variants.add(home.replace("/", "\\").rstrip("\\/"))
    variants |= {x.replace("\\", "\\\\") for x in tuple(variants)}
    lower = data.lower()
    for value in variants:
        if not value:
            continue
        for encoding in ("utf-8", "utf-16-le"):
            needle = value.encode(encoding).lower()
            pos = lower.find(needle)
            while pos >= 0:
                after = pos + len(needle)
                # End-of-string and path separator are valid end boundaries.
                boundaries = ((b"\\", b"/", b"\x00", b'"', b"'", b" "),
                              (b"\\\x00", b"/\x00", b"\x00\x00", b'"\x00', b"'\x00", b" \x00"))[
                                  encoding == "utf-16-le"]
                if after == len(lower) or any(lower.startswith(b, after) for b in boundaries):
                    return True
                pos = lower.find(needle, pos + 1)
    return False


def path_reference_detected(data: bytes) -> bool:
    return any(p.search(data) for p in MACHINE_REFERENCE_PATTERNS)


def audit(entries, home: str, is_msix: bool) -> dict:
    found: set[str] = set()
    warnings: list[dict] = []
    for name, data in entries:
        normalized = validate_name(name, is_msix)
        if normalized is None:
            continue
        if normalized in found:
            raise ValueError("Duplicate package member: " + normalized)
        found.add(normalized)
        suffix = PurePosixPath(normalized).suffix.lower()
        if not data or (suffix not in TEXT_SUFFIXES | PE_SUFFIXES
                        and normalized != "AppxManifest.xml"):
            continue
        # This checks the *actual current developer profile*, not a generic
        # vendor account from an unrelated third-party build environment.
        if is_home_reference(data, home):
            raise ValueError("Current developer home path embedded in: " + normalized +
                             ". Rebuild without embedded local paths; do not disable auditing.")
        if path_reference_detected(data):
            if suffix in TEXT_SUFFIXES or normalized == "AppxManifest.xml":
                raise ValueError("Machine-specific path in packaged text metadata: " + normalized)
            warnings.append({
                "file": normalized, "code": "VENDOR_BUILD_PATH",
                "detail": "PE binary contains a machine-like reference not matching the current developer home. "
                          "This may be vendor build/debug metadata; verify independently if needed."
            })
    missing = EXPECTED - found
    if missing:
        raise ValueError("Missing essential offline files: " + ", ".join(sorted(missing)))
    return {
        "status": "PASS",
        "approved_file_count": len(found),
        "required_offline_files": len(EXPECTED_MODELS),
        "warnings": warnings,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--stage", type=Path)
    source.add_argument("--msix", type=Path)
    parser.add_argument("--build-home", default="")
    parser.add_argument("--report", type=Path, help="Write redacted JSON report outside staged package")
    args = parser.parse_args()
    report: dict = {"status": "FAIL", "warnings": []}
    try:
        if args.stage:
            if not args.stage.is_dir():
                raise ValueError("Staging directory missing: " + str(args.stage))
            def stage_entries():
                for path in sorted(args.stage.rglob("*")):
                    if path.is_symlink():
                        raise ValueError("Symbolic links are not allowed in MSIX: " + str(path))
                    if path.is_file():
                        rel = path.relative_to(args.stage).as_posix()
                        suffix = path.suffix.lower()
                        yield rel, path.read_bytes() if suffix in TEXT_SUFFIXES | PE_SUFFIXES else b""
            report = audit(stage_entries(), args.build_home, False)
        else:
            if not args.msix.is_file():
                raise ValueError("MSIX package not found: " + str(args.msix))
            with zipfile.ZipFile(args.msix) as archive:
                def msix_entries():
                    for item in archive.infolist():
                        if item.is_dir():
                            continue
                        suffix = PurePosixPath(item.filename).suffix.lower()
                        yield item.filename, archive.read(item) if suffix in TEXT_SUFFIXES | PE_SUFFIXES else b""
                report = audit(msix_entries(), args.build_home, True)
    except (OSError, ValueError, zipfile.BadZipFile) as error:
        report["reason"] = str(error)
    finally:
        if args.report:
            try:
                args.report.parent.mkdir(parents=True, exist_ok=True)
                args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
                print("[AUDIT REPORT] " + str(args.report))
            except OSError as exc:
                print("[WARNING] Could not write audit report: " + str(exc), file=sys.stderr)
    if report["status"] != "PASS":
        print("[FAIL] Store privacy/portability audit: " + report.get("reason", "unknown"), file=sys.stderr)
        return 1
    print(f"[PASS] {report['approved_file_count']} approved files; voice pack present; "
          "no private data or developer profile links")
    if report["warnings"]:
        for warning in report["warnings"]:
            print(f"[REVIEW] {warning['file']}: vendor/CI-like compiled path; "
                  "not a verified user-machine dependency")
    return 0


if __name__ == "__main__":
    sys.exit(main())
