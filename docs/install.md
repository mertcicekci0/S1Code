# Install, update and remove

S1Code is a Rust executable. npm is not required and no official npm or crates.io
package has been published. Source installation is available now. Binary assets
are prepared by the release workflow; only use a version whose assets actually
appear on the [releases page](https://github.com/mertcicekci0/S1Code/releases).

## Source

Requires Git and Rust via rustup. The checkout selects Rust 1.94.0.

```sh
git clone https://github.com/mertcicekci0/S1Code.git
cd S1Code
cargo install --path . --locked
s1code doctor
```

For a published version, check out its exact tag before installing. Cargo installs
to its configured bin directory, normally `~/.cargo/bin`. Add that directory to
PATH if your rustup setup has not already done so. To update, review the changelog,
fetch/check out the desired version and repeat the install command.

## Verified binary installation

The prepared targets are Apple Silicon macOS (built on macOS 15) and Linux x86_64
(Ubuntu 24.04, glibc 2.39+). Intel macOS, ARM Linux, older glibc and musl binaries
are not provided; use source builds where supported. Windows is unsupported.

After a version is published, download its `install.py` asset, inspect it and run
it with Python 3.10 or newer. Example for the prepared candidate:

```sh
curl --proto '=https' --tlsv1.2 -fL \
  https://github.com/mertcicekci0/S1Code/releases/download/v0.3.0-rc.8/install.py \
  -o install-s1code.py
# Read install-s1code.py before executing it.
python3 install-s1code.py --version 0.3.0-rc.8
~/.local/bin/s1code doctor
```

An absent release returns a download error; the installer does not substitute a
different version or build from unreviewed source. There is no `curl | sh` step.
The script does not require sudo, change shell profiles, collect credentials or
start a model request. Add `~/.local/bin` to PATH yourself if needed. Check
`command -v s1code` and `s1code --version` if an older Cargo installation takes
precedence. Use `--prefix DIRECTORY` for a different user-owned installation root.

The installer checks SHA-256, target/version metadata, archive structure and
required licenses before replacing the binary. Notices are retained under
`PREFIX/share/s1code/VERSION/`. Downloads use HTTPS. These checks detect corruption
and packaging errors; a checksum from the same release is not an independent
signature or protection against a compromised publisher. macOS binaries are not
Developer ID signed or notarized; if macOS blocks execution, use the source build
instead of globally weakening Gatekeeper. No automatic updater runs.

On macOS, a newly built or replaced unsigned binary can trigger a Keychain access
prompt for an existing saved key. Allow the official local binary when macOS asks;
you should not need to paste the key again. A timeout sends no provider request.
S1Code does not weaken Keychain access controls to avoid this OS prompt.

For an offline installation, download the target `.tar.gz` and `SHA256SUMS` assets
on another machine, then use:

```sh
python3 install-s1code.py --version 0.3.0-rc.8 \
  --archive s1code-0.3.0-rc.8-aarch64-apple-darwin.tar.gz --checksums SHA256SUMS
```

Keep the previous release archive if you want binary rollback. Installing a prior
version does not migrate session formats backwards; back up your data first and
check [storage migration](architecture.md). Saved sessions are not deleted on update.

## Remove

For Cargo, use `cargo uninstall s1code`. For binary installation, remove only
`PREFIX/bin/s1code` and `PREFIX/share/s1code` from the prefix you selected. Neither
method removes sessions or keys. `s1code doctor` reports the storage location;
review it before deleting personal history. `s1code auth remove PROVIDER` removes
that native provider's saved macOS key. Managed client accounts are separate.

Approved repository tests and scripts are not sandboxed. Read [SECURITY](../SECURITY.md)
and [provider setup](providers.md) before running on a real repository.
