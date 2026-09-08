# Local validation — 2026-09-08

Tested on x86-64 Omarchy Quattro 4.0.0.alpha, with its existing
PipeWire/PulseAudio service.

## 0.1.0 — backend, audio, and the packaged folder

- Nine Rust tests passed: FM recovery at 250 kHz, 1.024 MHz, and 2.4 MHz;
  19 kHz pilot rejection; frequency validation; sample/framing validation;
  and mock-server busy, malformed-packet, receive/retune/disconnect sessions.
- `cargo clippy --locked --all-targets -- -D warnings` passed.
- The packaged manifest/folder passed `omarchy plugin validate`.
- QML lint resolves the actual Omarchy imports. The installed Quickshell type
  metadata emits one warning for `QProcess::ExitStatus` on `Process.onExited`;
  the handler works in the running shell. No OmaSDR runtime QML errors were
  observed during the UI checks.
- A real SDR++ server supplied 1.536 MHz IQ at 102.4 MHz. A 12-second backend
  test produced 11.17 seconds of 48 kHz mono PCM after startup, with zero
  dropped audio blocks. WAV RMS was 0.042 at 30% volume.
- The Omathought-styled panel connected to that server. PipeWire reported
  an active `float32le 1ch 48000Hz` OmaSDR stream.
- Typed `102,5` retuned to 102.5 MHz; typed `102,4` returned to 102.4 MHz.
  Keyboard volume reached 0% and returned to an audible level.
- Escape closed the panel; shell summon reopened it while audio continued.
  Disconnect ended the backend. Disabling the widget removed its audio stream,
  and re-enabling restored the panel. The panel also loaded after shell restart.
  Removal unloaded the test copy; copying the package back and enabling it
  restored the widget. The local test installation keeps the provided server
  address as an Omarchy widget setting; the shipped default remains localhost.

The recorded PCM and desktop audio stream establish decoding and playback;
no station identity or subjective listening-quality claim is made.

## 1.0.0 — installation from the public repository

Repeated against <https://github.com/marvreichmann/omasdr>. The first pass ran
under the plugin's original `marv.omasdr` id; the id became
`com.github.marvreichmann.omasdr` before the release was cut, and the install
and panel checks below were rerun under the new one.

- The nine Rust tests, Clippy, `omarchy plugin validate` (repository root and
  packaged folder), and QML lint were rerun at 1.0.0. Lint still emits only the
  `QProcess::ExitStatus` warning above.
- The release workflow reran the tests and Clippy on a clean checkout of the
  tag, matched the tag against `manifest.json`, `Cargo.toml`, and
  `bin/build-info.json`, and re-verified `bin/omasdr` against `bin/SHA256SUMS`
  and every source hash in `bin/build-info.json`.
- The previous install was removed, then
  `omarchy plugin add https://github.com/marvreichmann/omasdr --enable --yes`
  cloned and enabled the plugin as `com.github.marvreichmann.omasdr`. The clone
  is a git checkout, `bin/omasdr` arrived mode 755 and reports `OmaSDR 1.0.0`,
  and its checksums verify against the committed metadata.
- The widget landed in the bar's right section, as `defaultSection` specifies,
  and drew its radio glyph without the playing dot.
  `omarchy-shell shell summon com.github.marvreichmann.omasdr '{}'` returned
  `ok` and drew the panel with the shipped defaults — `127.0.0.1:5259`,
  102.4 MHz, 30% — read through the manifest fallbacks, since this bar entry
  carries no overrides. `omarchy-shell shell hide
  com.github.marvreichmann.omasdr` closed it. The shell journal recorded no
  OmaSDR warning or error, only its plugin-reload lines.
- `summon` returns `ok` whenever the bar holds a live widget, even when the
  panel Loader has produced no item, so it is not on its own evidence that the
  panel drew. The panel was confirmed on screen instead.

This round covers packaging and installation from the published repository. It
did not repeat the on-air checks: no SDR++ server was running, so reception,
playback, retuning, and disable/re-enable at 1.0.0 rest on the 0.1.0 results
above. The code changed between them only in version strings, the plugin id,
manifest metadata, README wording, and the removal of an uncalled QML
function; no receiver, DSP, or audio path was touched. Marketplace submission
has not been done.
