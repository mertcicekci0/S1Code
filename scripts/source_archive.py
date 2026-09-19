#!/usr/bin/env python3
"""Create a reproducible source archive from a clean, scanned Git commit."""
import gzip
import hashlib
import io
import json
import pathlib
import subprocess
import sys
import tarfile

ROOT = pathlib.Path(__file__).resolve().parent.parent


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT)


def main():
    if git('status', '--porcelain', '--untracked-files=all').strip():
        sys.exit('Commit or preserve pending changes before packaging; the archive must match a reviewed commit.')
    subprocess.run([sys.executable, 'scripts/release_check.py'], cwd=ROOT, check=True)
    package = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1'], cwd=ROOT))['packages'][0]
    version = package['version']
    prefix = f's1code-{version}/'
    source = git('archive', '--format=tar', f'--prefix={prefix}', 'HEAD')
    with tarfile.open(fileobj=io.BytesIO(source)) as archive:
        members = archive.getmembers()
        for member in members:
            path = pathlib.PurePosixPath(member.name)
            if path.is_absolute() or '..' in path.parts or member.issym() or member.islnk():
                sys.exit('Unsupported archive path or link; no package written.')
            if set(path.parts) & {'target', 'private', 'eval-results', '.git', '.env'}:
                sys.exit('Private artifact in source archive; no package written.')
        names = {member.name for member in members}
        if not all(prefix + name in names for name in ['LICENSE', 'Cargo.lock', 'README.md']):
            sys.exit('Required source/license files are absent; no package written.')
    output = ROOT / 'target' / 'dist'
    output.mkdir(parents=True, exist_ok=True)
    filename = f's1code-{version}-src.tar.gz'
    compressed = io.BytesIO()
    with gzip.GzipFile(fileobj=compressed, mode='wb', filename='', mtime=0) as stream:
        stream.write(source)
    data = compressed.getvalue()
    checksum = hashlib.sha256(data).hexdigest()
    (output / filename).write_bytes(data)
    (output / 'SHA256SUMS').write_text(f'{checksum}  {filename}\n')
    manifest = dict(version=version, commit=git('rev-parse', 'HEAD').decode().strip(),
                    archive=filename, sha256=checksum, files=sum(member.isfile() for member in members))
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(json.dumps(manifest, indent=2))


if __name__ == '__main__':
    main()
