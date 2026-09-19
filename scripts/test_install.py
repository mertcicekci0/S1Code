#!/usr/bin/env python3
"""Offline release installer invariants. Never execute a downloaded payload."""
import hashlib
import gzip
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import install

VERSION = "0.3.0-rc.8"
HOST = "aarch64-apple-darwin"


def fixture(extra=None, omit=None, manifest_update=None):
    binary = b"test payload, not executable"
    manifest = dict(version=VERSION, target=HOST, commit="a" * 40,
                    binary_sha256=hashlib.sha256(binary).hexdigest())
    manifest.update(manifest_update or {})
    files = {"s1code": binary, "LICENSE": b"license", "THIRD_PARTY_LICENSES.txt": b"notices",
             "manifest.json": json.dumps(manifest).encode()}
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:gz") as archive:
        for name, data in files.items():
            if name == omit:
                continue
            entry = tarfile.TarInfo(name)
            entry.size = len(data)
            archive.addfile(entry, io.BytesIO(data))
        if extra is not None:
            archive.addfile(extra)
    data = output.getvalue()
    checksums = f"{hashlib.sha256(data).hexdigest()}  s1code-{VERSION}-{HOST}.tar.gz\n".encode()
    return data, checksums


class Installation(unittest.TestCase):
    def test_installs_and_updates_atomically_with_licenses(self):
        data, sums = fixture()
        contents = install.validate(data, sums, VERSION, HOST)
        with tempfile.TemporaryDirectory() as directory:
            prefix = Path(directory)
            destination = install.install(contents, prefix, VERSION)
            self.assertEqual(destination.read_bytes(), contents["s1code"])
            self.assertEqual(destination.stat().st_mode & 0o777, 0o755)
            self.assertEqual((prefix / "share/s1code" / VERSION / "LICENSE").read_bytes(), b"license")
            contents["s1code"] = b"updated"
            install.install(contents, prefix, VERSION)
            self.assertEqual(destination.read_bytes(), b"updated")

    def test_tampering_and_duplicate_checksums_fail(self):
        data, sums = fixture()
        for archive, checksum in [(data + b"changed", sums), (data, sums + sums), (data, b""),
                                  (data, sums.replace(HOST.encode(), b"different-platform"))]:
            with self.subTest(checksum=checksum), self.assertRaises(ValueError):
                install.validate(archive, checksum, VERSION, HOST)

    def test_unsafe_archive_forms_fail(self):
        for name, kind in [("../escape", tarfile.REGTYPE), ("/absolute", tarfile.REGTYPE),
                           ("s1code", tarfile.REGTYPE), ("link", tarfile.SYMTYPE),
                           ("LICENSE", tarfile.LNKTYPE), ("device", tarfile.CHRTYPE)]:
            entry = tarfile.TarInfo(name)
            entry.type = kind
            entry.linkname = "s1code"
            data, sums = fixture(extra=entry)
            with self.subTest(name=name, kind=kind), self.assertRaises(ValueError):
                install.validate(data, sums, VERSION, HOST)

    def test_missing_notices_and_wrong_manifest_fail(self):
        for name in install.FILES:
            with self.subTest(missing=name), self.assertRaises(ValueError):
                install.validate(*fixture(omit=name), VERSION, HOST)
        for update in [dict(target="other"), dict(version="1.2.3"), dict(commit="unknown"),
                       dict(binary_sha256="a" * 64)]:
            with self.subTest(update=update), self.assertRaises(ValueError):
                install.validate(*fixture(manifest_update=update), VERSION, HOST)

    def test_size_limits_and_unsupported_platform(self):
        with patch.object(install, "MAX_CONTENT", 1), self.assertRaises(ValueError):
            install.validate(*fixture(), VERSION, HOST)
        with patch.object(install.platform, "system", return_value="Windows"), self.assertRaises(ValueError):
            install.target()

    def test_decompression_is_bounded_before_parsing_tar_headers(self):
        data = gzip.compress(b"a" * 20_000)
        sums = f"{hashlib.sha256(data).hexdigest()}  s1code-{VERSION}-{HOST}.tar.gz\n".encode()
        with patch.object(install, "MAX_CONTENT", 1024), self.assertRaisesRegex(ValueError, "Unpacked release"):
            install.validate(data, sums, VERSION, HOST)

    def test_malformed_manifest_types_fail_without_traceback(self):
        for value in [None, 12, [], {}]:
            with self.subTest(commit=value), self.assertRaises(ValueError):
                install.validate(*fixture(manifest_update=dict(commit=value)), VERSION, HOST)

    def test_failure_keeps_existing_binary_and_cleans_staging(self):
        contents = install.validate(*fixture(), VERSION, HOST)
        with tempfile.TemporaryDirectory() as directory:
            prefix = Path(directory)
            destination = install.install(contents, prefix, VERSION)
            contents["s1code"] = b"new"
            with patch.object(install.os, "replace", side_effect=OSError("failure")), self.assertRaises(OSError):
                install.install(contents, prefix, VERSION)
            self.assertNotEqual(destination.read_bytes(), b"new")
            self.assertEqual(list((prefix / "bin").iterdir()), [destination])

    def test_symlink_destination_is_never_followed(self):
        contents = install.validate(*fixture(), VERSION, HOST)
        with tempfile.TemporaryDirectory() as directory:
            prefix = Path(directory)
            (prefix / "bin").mkdir()
            outside = prefix / "outside"
            outside.write_bytes(b"keep")
            (prefix / "bin/s1code").symlink_to(outside)
            with self.assertRaises(ValueError):
                install.install(contents, prefix, VERSION)
            self.assertEqual(outside.read_bytes(), b"keep")


if __name__ == "__main__":
    unittest.main()
