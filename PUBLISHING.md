# Publishing OmaSDR

Follow the [Omarchy publishing guide](https://plugins.omarchy.org/publish.html)
and [development guide](https://plugins.omarchy.org/develop.html).

1. Keep the permanent non-reserved ID `marv.omasdr` consistent in the root
   manifest and QML. Confirm the author and license before the initial release.
2. Run the Rust tests, Clippy, `scripts/build.sh`, and `scripts/lint-qml.sh` on
   x86-64 Omarchy. Verify `bin/SHA256SUMS` from the repository root.
3. Copy `dist/marv.omasdr/` into `~/.config/omarchy/plugins/` for runtime testing.
   Validate the folder, rescan, and enable it. Test clicks, typing, tuning,
   volume/mute, Escape, outside dismissal, shell summon/hide, disconnect,
   disable/re-enable, shell restart, and removal. Watch shell logs for errors.
   Confirm that disabling/removing the plugin releases the server and audio.
4. Commit the root manifest, QML, Rust source and lockfile, tests, build scripts,
   README, license, third-party notices, and **prebuilt executable**. Preserve
   the executable bit; do not commit `target/`, `dist/`, or symlinks. Build
   metadata and checksums must match the source and executable being released.
5. Bump `manifest.json` and `Cargo.toml` together, rerun `scripts/build.sh` so
   the committed executable and its metadata match, and add the version's
   section to `CHANGELOG.md`. Push to
   <https://github.com/marvreichmann/omasdr>, then tag `vX.Y.Z` and push the
   tag: `.github/workflows/release.yml` reruns the tests, checks the tag
   against the manifest, the crate, and `bin/build-info.json`, re-verifies the
   committed executable's checksums, and publishes the release with that
   changelog section as its notes. Test
   `omarchy plugin add https://github.com/marvreichmann/omasdr --enable`.
6. Submit the repository through the marketplace's linked submission form,
   using a suitable media category and SDR/FM tags. An optimized preview is
   optional. Listing approval remains with the marketplace maintainers.

The installer only clones files and validates the plugin. It runs no build
steps, dependency installers, or hooks. The backend must already be present.
Documented runtime requirements are the existing x86-64 Omarchy shell and
audio libraries; other architectures are not part of this release.
