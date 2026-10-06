# Release Process

TermSense uses maintainer-created semantic version tags and a tag-only GitHub Actions release workflow.

Normal pushes and pull requests do not publish releases.

## 1. Choose the version

TermSense follows semantic versioning in the usual form:

```text
MAJOR.MINOR.PATCH
```

Examples:

- patch: `0.1.1`
- minor: `0.2.0`
- major: `1.0.0`

For the v0.x series, minor versions may still contain meaningful product-boundary changes.

## 2. Update release metadata

Before tagging:

1. update `Cargo.toml` version;
2. update `Cargo.lock` if Cargo changes it;
3. update `CHANGELOG.md`;
4. add/update `RELEASE_NOTES_vX.Y.Z.md`;
5. update README status/examples when needed.

The tag must exactly match Cargo version:

```text
Cargo.toml version = 0.1.1
Git tag            = v0.1.1
```

The workflow rejects mismatches.

## 3. Run local verification

Run:

```bash
cargo fmt --check
cargo test
bash scripts/check-local.sh
bash scripts/release-readiness.sh
```

Interactive changes should also be tested manually in Bash.

For Bash/Readline changes, verify at least:

- live suggestions;
- Up/Down;
- Tab;
- Enter;
- Right Arrow;
- Esc;
- Ctrl+Space;
- normal command execution after dismissal;
- one quoted-path case;
- one nested context such as `sudo git che`.

## 4. Commit and push main

The release commit should be clean and already present on `main`.

Confirm:

```bash
git status
git pull --ff-only origin main
```

## 5. Create the annotated tag

Example:

```bash
git tag -a v0.1.1 -m "TermSense v0.1.1"
git push origin v0.1.1
```

The tag push triggers:

```text
.github/workflows/release.yml
```

## 6. Automated release job

The workflow:

1. checks out the tag;
2. prepares stable Rust + rustfmt;
3. verifies tag/version identity;
4. runs the full release-readiness gate;
5. builds the Debian package;
6. creates a Linux amd64 binary tarball;
7. generates `SHA256SUMS`;
8. publishes a GitHub Release.

When `RELEASE_NOTES_vX.Y.Z.md` exists, it becomes the release body.

## 7. Verify the published release

After the workflow succeeds, verify:

- release is not draft/prerelease unless intended;
- tag targets the intended commit;
- `.deb` asset exists;
- Linux tarball exists;
- `SHA256SUMS` exists;
- release notes are correct;
- README latest-release badge resolves correctly.

## Failed release

If the tag-triggered workflow fails:

- do not silently move a published version tag after users may have fetched it;
- fix the repository first;
- for an unpublished accidental tag, delete/recreate only when the situation is clearly controlled;
- otherwise prefer a new patch version.

Release history should remain auditable.
