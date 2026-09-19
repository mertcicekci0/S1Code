#!/usr/bin/env python3
"""Package the tested host binary with required notices and public build identity."""
import gzip
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parent.parent


def main():
    def command(*args):
        return subprocess.check_output(args, cwd=ROOT, text=True).strip()
    if command("git", "status", "--porcelain", "--untracked-files=all"):
        raise SystemExit("Binary packaging requires a clean committed tree")
    package = json.loads(command("cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"))["packages"][0]
    version = package["version"]
    host = next(line.split(": ", 1)[1] for line in command("rustc", "-vV").splitlines() if line.startswith("host: "))
    if host not in {"aarch64-apple-darwin", "x86_64-unknown-linux-gnu"}:
        raise SystemExit("Binary distribution is currently tested on macOS ARM64 and Linux x86_64 only")
    binary = ROOT / "target/release/s1code"
    if command(str(binary), "--version") != f"s1code {version}":
        raise SystemExit("Binary version differs from Cargo.toml; rebuild before packaging")
    data = binary.read_bytes()
    if str(ROOT).encode() in data or str(Path.home()).encode() in data:
        raise SystemExit("Binary contains a local build path; rebuild with sh scripts/build_release.sh")
    manifest = dict(version=version, target=host, commit=command("git", "rev-parse", "HEAD"),
                    binary_sha256=hashlib.sha256(data).hexdigest())
    files = {"s1code": data, "LICENSE": (ROOT / "LICENSE").read_bytes(),
             "THIRD_PARTY_LICENSES.txt": (ROOT / "target/notices/THIRD_PARTY_LICENSES.txt").read_bytes(),
             "manifest.json": (json.dumps(manifest, indent=2) + "\n").encode()}
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w", format=tarfile.USTAR_FORMAT) as archive:
        for name, content in sorted(files.items()):
            info = tarfile.TarInfo(name)
            info.size = len(content)
            info.mode = 0o755 if name == "s1code" else 0o644
            info.mtime = 0
            archive.addfile(info, io.BytesIO(content))
    output = ROOT / "target/dist"
    output.mkdir(parents=True, exist_ok=True)
    filename = f"s1code-{version}-{host}.tar.gz"
    compressed = gzip.compress(raw.getvalue(), mtime=0)
    (output / filename).write_bytes(compressed)
    (output / f"{host}.sha256").write_text(f"{hashlib.sha256(compressed).hexdigest()}  {filename}\n")
    print(json.dumps(manifest))


if __name__ == "__main__":
    main()
