# Releasing uQ

## Versions

- The workspace `version` in `Cargo.toml` is the version. `packaging/uq/PKGBUILD`'s `pkgver`
  follows it; `tests/shell/version_test.sh` (run by CI) fails when they differ or when a
  `.SRCINFO` is stale.
- Tags are `vX.Y.Z` or, for release candidates, `vX.Y.Z-rc.N`. pacman forbids `-` in `pkgver`,
  so a release candidate's `pkgver` is `X.Y.ZrcN` (`1.0.0-rc.1` → `1.0.0rc1`, which `vercmp`
  sorts before `1.0.0`). The PKGBUILD maps it back with `_tag=${pkgver/rc/-rc.}`.
- Anything below 1.0.0, and every release candidate, is published as a GitHub pre-release.
- `uq-hyprland-pin` has its own `pkgver`: the lowest Hyprland it allows. Bump it only with the
  pin itself, and set `_pin` in `packaging/uq/PKGBUILD` to the same version: `uq` depends on
  `uq-hyprland-pin>=$_pin`, so an updated `uq` can't install beside an older pin
  (`tests/shell/version_test.sh` checks the two agree).

## Release gate (every tag)

From [the road to 1.0](superpowers/plans/2026-11-09-uq-road-to-1.0.md):

1. CI is green on `main`: fmt, clippy, cargo tests, QML tests, shell tests and `verify_config`.
2. The smoke run passed in CI or the VM.
3. The milestone's live checklist (`docs/notes/`) is complete, with no open Critical or
   Important issues.
4. A clean-chroot package build passed, plus a fresh-user install:
   `scripts/build-local-pkg.sh --keep --nobuild` leaves a build directory you can run
   `extra-x86_64-build` in, then `pacman -U` the result and run `uqctl setup` as a new user.
5. The CHANGELOG is written, the version is bumped everywhere, and the Hyprland pin is
   re-verified against current Arch and Manjaro
   (`cargo test -p uqd --test verify_config -- --ignored`).
6. Tag → release workflow → GitHub Release → AUR push (below).

## Cutting a release

1. Move the `[Unreleased]` notes in `CHANGELOG.md` under `## [X.Y.Z] - YYYY-MM-DD` and leave an
   empty `[Unreleased]` section above it.
2. `scripts/bump-version.sh X.Y.Z` (or `X.Y.Z-rc.N`). It updates `Cargo.toml`, `Cargo.lock`,
   `pkgver`, resets `pkgrel=1` and regenerates `packaging/uq/.SRCINFO`.
3. `bash tests/shell/version_test.sh`, then commit (`release: vX.Y.Z`) and merge to `main` with
   CI green.
4. Tag the merge commit and push the tag:

   ```bash
   git tag -a vX.Y.Z -m "uQ X.Y.Z"
   git push origin vX.Y.Z
   ```

5. `.github/workflows/release.yml` runs the full CI, checks the tag matches `Cargo.toml`, builds
   `uq-X.Y.Z.tar.gz` with `git archive`, builds both packages from it in an Arch container, and
   publishes the GitHub Release with the CHANGELOG section as notes and the tarball, its
   `.sha256` and both `.pkg.tar.zst` as assets. A tag that doesn't match the version fails the
   run before anything is published; fix it by deleting the tag, bumping, and tagging again.

## Updating the AUR

The AUR packages build from the release asset `uq-X.Y.Z.tar.gz`, not GitHub's
`/archive/` tarball: those are generated on demand and may differ byte-wise from `git archive`, so
the checksum must come from the published asset.

1. Wait for the GitHub Release to be published.
2. Check the asset's checksum against the one in the release notes:

   ```bash
   curl -fLO https://github.com/asafelobotomy/uq/releases/download/vX.Y.Z/uq-X.Y.Z.tar.gz
   sha256sum uq-X.Y.Z.tar.gz
   ```

3. In `packaging/uq/PKGBUILD`, set `sha256sums=('<that sum>')`, then regenerate `.SRCINFO`:

   ```bash
   cd packaging/uq && makepkg --printsrcinfo > .SRCINFO
   ```

   (The workflow run's summary and its `aur` artifact have both files ready.) Commit this to
   `main` so `packaging/` mirrors the AUR.
4. Copy `PKGBUILD`, `.SRCINFO` and `uq.install` into your clones of the AUR repositories and push:

   ```bash
   git clone ssh://aur@aur.archlinux.org/uq.git aur-uq
   cp packaging/uq/PKGBUILD packaging/uq/.SRCINFO packaging/uq/uq.install aur-uq/
   cd aur-uq && makepkg --verifysource && git add uq.install && git commit -am "Update to X.Y.Z" && git push
   ```

   Do the same for `uq-hyprland-pin` (`packaging/uq-hyprland-pin/`) whenever the pin changed.
5. Check the install: `yay -S uq` (or `paru -S uq`) on a clean system or VM.

AUR pushes are manual for now. Automating them needs an AUR SSH key stored as a repository secret
and an AUR deploy step in `release.yml`.
