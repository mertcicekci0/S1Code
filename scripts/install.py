#!/usr/bin/env python3
"""Install an explicitly selected S1Code release; Python 3.10+, no dependencies."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import re
import sys
import tarfile
import tempfile
import urllib.request

REPOSITORY = "https://github.com/mertcicekci0/S1Code/releases/download"
FILES = {"s1code", "LICENSE", "THIRD_PARTY_LICENSES.txt", "manifest.json"}
MAX_ARCHIVE = 64 * 1024 * 1024
MAX_CONTENT = 128 * 1024 * 1024


def target():
    supported = {("Darwin", "arm64"): "aarch64-apple-darwin",
                 ("Linux", "x86_64"): "x86_64-unknown-linux-gnu"}
    found = supported.get((platform.system(), platform.machine()))
    if not found:
        raise ValueError("No binary for this platform; use the documented Cargo source installation")
    return found


class HttpsOnly(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if not newurl.startswith("https://"):
            raise ValueError("Refusing non-HTTPS download redirect")
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def download(url, limit):
    with urllib.request.build_opener(HttpsOnly).open(url, timeout=60) as response:
        data = response.read(limit + 1)
    if len(data) > limit:
        raise ValueError("Release download exceeds size limit")
    return data


def read_file(path, limit):
    with Path(path).open("rb") as stream:
        data = stream.read(limit + 1)
    if len(data) > limit:
        raise ValueError("Release file exceeds size limit")
    return data


def validate(archive, checksums, version, host):
    filename = f"s1code-{version}-{host}.tar.gz"
    matching = [line.split() for line in checksums.decode("ascii").splitlines()
                if len(line.split()) == 2 and line.split()[1] == filename]
    if len(matching) != 1 or not re.fullmatch(r"[0-9a-f]{64}", matching[0][0]):
        raise ValueError("Release checksum is absent, duplicated, or malformed")
    if hashlib.sha256(archive).hexdigest() != matching[0][0]:
        raise ValueError("Release checksum mismatch; nothing installed")
    contents = {}
    size = 0
    # Read only the exact regular files we ship. No extractall, paths, links or devices.
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as bundle:
        for member in bundle:
            if member.name not in FILES or member.name in contents or not member.isfile():
                raise ValueError("Unexpected archive entry; nothing installed")
            size += member.size
            if member.size < 0 or size > MAX_CONTENT:
                raise ValueError("Unpacked release exceeds size limit")
            contents[member.name] = bundle.extractfile(member).read(member.size + 1)
            if len(contents[member.name]) != member.size:
                raise ValueError("Incomplete archive entry")
    if set(contents) != FILES:
        raise ValueError("Release is missing required binary or license files")
    manifest = json.loads(contents["manifest.json"])
    if (not isinstance(manifest, dict) or manifest.get("version") != version or manifest.get("target") != host
            or not re.fullmatch(r"[0-9a-f]{40}", manifest.get("commit", ""))
            or manifest.get("binary_sha256") != hashlib.sha256(contents["s1code"]).hexdigest()):
        raise ValueError("Release manifest does not match requested version, platform or binary")
    if not contents["s1code"] or not contents["LICENSE"] or not contents["THIRD_PARTY_LICENSES.txt"]:
        raise ValueError("Release binary or notices are empty")
    return contents


def install(contents, prefix, version):
    bin_dir = prefix / "bin"
    notice_dir = prefix / "share" / "s1code" / version
    destination = bin_dir / "s1code"
    if destination.is_symlink() or (destination.exists() and not destination.is_file()):
        raise ValueError("Installation destination must be a regular file, not a link or directory")
    # Licenses are installed before replacing the executable. An error leaves the old
    # executable intact; the temporary binary is on the same filesystem as its target.
    bin_dir.mkdir(parents=True, exist_ok=True)
    if notice_dir.is_symlink():
        raise ValueError("Refusing linked notice directory")
    notice_dir.mkdir(parents=True, exist_ok=True)
    for name in FILES - {"s1code"}:
        path = notice_dir / name
        if path.is_symlink():
            raise ValueError("Refusing linked notice file")
        path.write_bytes(contents[name])
    with tempfile.NamedTemporaryFile(dir=bin_dir, prefix=".s1code-", delete=False) as stream:
        temporary = Path(stream.name)
        try:
            stream.write(contents["s1code"])
            stream.flush()
            os.fsync(stream.fileno())
            temporary.chmod(0o755)
            os.replace(temporary, destination)
        finally:
            temporary.unlink(missing_ok=True)
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True, help="Exact version, without v (no moving latest alias)")
    parser.add_argument("--prefix", type=Path, default=Path.home() / ".local")
    parser.add_argument("--archive", type=Path, help="Use an already downloaded release archive")
    parser.add_argument("--checksums", type=Path, help="Required with --archive")
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-rc\.[0-9]+)?", args.version):
        parser.error("Use an exact numeric version, optionally with -rc.N")
    if bool(args.archive) != bool(args.checksums):
        parser.error("--archive and --checksums must be supplied together")
    host = target()
    filename = f"s1code-{args.version}-{host}.tar.gz"
    if args.archive:
        archive = read_file(args.archive, MAX_ARCHIVE)
        checksums = read_file(args.checksums, 16384)
    else:
        base = f"{REPOSITORY}/v{args.version}"
        checksums = download(f"{base}/SHA256SUMS", 16384)
        archive = download(f"{base}/{filename}", MAX_ARCHIVE)
    contents = validate(archive, checksums, args.version, host)
    destination = install(contents, args.prefix.expanduser().resolve(), args.version)
    print(f"Installed S1Code {args.version}: {destination}")
    print("Add the installation bin directory to PATH if needed, then run s1code doctor.")
    print("Checksums detect corruption; they are not an independent signature. No shell profile changed.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, tarfile.TarError) as error:
        sys.exit(f"S1Code installation failed: {error}")
