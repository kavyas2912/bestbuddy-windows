"""Restore the checksum-pinned distribution assets without replacing existing files.

Python 3.10+, standard library only. Downloads are build-time operations; installed
apps use the included resources. --verify performs no downloads or mutations.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import tarfile
import urllib.request
import zipfile


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def inside(root, relative):
    path = (root / relative).resolve()
    if not path.is_relative_to(root.resolve()) or path == root.resolve():
        raise ValueError('Resource path escapes its root: ' + relative)
    return path


def install(source, target, expected):
    if digest(source) != expected.lower():
        raise ValueError('Resource checksum mismatch: ' + str(source))
    if target.exists():
        if digest(target) != expected.lower():
            raise ValueError('Preserving differing existing file: ' + str(target))
        return
    target.parent.mkdir(parents=True, exist_ok=True)
    try:
        os.link(source, target)
    except OSError:
        # Exclusive creation also protects existing files on another filesystem.
        with source.open('rb') as src, target.open('xb') as dst:
            shutil.copyfileobj(src, dst)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path)
    parser.add_argument('--verify', action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    manifest = json.loads((root / 'DEFAULT_RESOURCES.json').read_text(encoding='utf-8'))
    cache = args.cache or root / '.resource-cache'
    checked = 0
    for row in manifest['files']:
        outputs = row.get('files', [row])
        missing = False
        for output in outputs:
            target = inside(root, output['target'])
            if target.exists():
                if digest(target) != output['sha256'].lower():
                    raise ValueError('Preserving differing existing file: ' + str(target))
                checked += 1
            else:
                missing = True
                if args.verify:
                    raise FileNotFoundError(target)
        if not missing:
            continue
        cache.mkdir(parents=True, exist_ok=True)
        if 'source' in row:
            source = inside(root, row['source'])
        else:
            url = row['url']
            if not url.startswith('https://'):
                raise ValueError('Only public HTTPS resource URLs are portable')
            identifier = row.get('sha256') or hashlib.sha256(url.encode()).hexdigest()
            source = cache / identifier
            if not source.exists():
                temporary = source.with_suffix('.downloading')
                request = urllib.request.Request(url, headers={'User-Agent': 'BestBuddy-resource-builder/1'})
                with urllib.request.urlopen(request, timeout=90) as src, temporary.open('wb') as dst:
                    size = 0
                    while block := src.read(1024 * 1024):
                        size += len(block)
                        if size > 2_500_000_000:
                            raise ValueError('Resource exceeds download limit')
                        dst.write(block)
                if row.get('sha256') and digest(temporary) != row['sha256'].lower():
                    raise ValueError('Downloaded resource checksum mismatch')
                temporary.rename(source)
        if row.get('sha256') and digest(source) != row['sha256'].lower():
            raise ValueError('Cached resource checksum mismatch: ' + str(source))
        if row.get('archive'):
            archive = zipfile.ZipFile(source) if row['archive'] == 'zip' else tarfile.open(source)
            with archive:
                for output in outputs:
                    # Never extract the archive: read only declared regular members,
                    # then verify each member before installing within the app tree.
                    member = output['member']
                    if row['archive'] == 'zip':
                        stream = archive.open(member)
                    else:
                        info = archive.getmember(member)
                        if not info.isfile():
                            raise ValueError('Resource archive member is not a regular file')
                        stream = archive.extractfile(info)
                    extracted = cache / output['sha256'].lower()
                    if not extracted.exists():
                        temporary = extracted.with_suffix('.extracting')
                        with stream, temporary.open('wb') as dst:
                            shutil.copyfileobj(stream, dst)
                        if digest(temporary) != output['sha256'].lower():
                            raise ValueError('Archive member checksum mismatch: ' + member)
                        temporary.rename(extracted)
                    else:
                        stream.close()
                    install(extracted, inside(root, output['target']), output['sha256'])
                    checked += 1
        else:
            install(source, inside(root, row['target']), row['sha256'])
            checked += 1
    for relative, content in manifest.get('generated_files', {}).items():
        target = inside(root, relative)
        if target.exists():
            if json.loads(target.read_text(encoding='utf-8')) != content:
                raise ValueError('Preserving differing resource configuration: ' + relative)
        elif args.verify:
            raise FileNotFoundError(target)
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            with target.open('x', encoding='utf-8') as stream:
                json.dump(content, stream, indent=2)
    print(f'{manifest["platform"]}: verified {checked} included resource files')


if __name__ == '__main__':
    main()
