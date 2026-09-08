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

## 1.1.0 — stereo decoding, off air

Automated and synthetic only. **No on-air test has been run at 1.1.0**, so every
claim below comes from generated signals, not from a broadcast station.

- Eleven Rust tests passed, the eight from 1.0.0 (the pilot-rejection and FM
  recovery tests reworked for interleaved output) plus two new ones: channel
  separation from a synthetic composite at 250 kHz, 1.024 MHz, and 2.4 MHz, and
  a no-pilot signal staying bit-identical across both channels. Measured
  separation was 41.9, 41.2, and 37.5 dB; the test asserts 25 dB.
- `cargo clippy --locked --all-targets -- -D warnings` passed, on
  rustc 1.98.1 (the 1.0.0 binary was built with 1.88.0).
- `omarchy plugin validate` passed on the packaged folder. QML lint still emits
  only the `QProcess::ExitStatus` warning recorded above.
- End to end through the shipped `bin/omasdr`: a local mock server streamed
  250 kHz signed-16-bit IQ carrying a 1 kHz tone on the left channel only, at
  9 % pilot injection. `--wav` wrote a two-channel 48 kHz file, the backend
  reported `FM 102.4 MHz · stereo`, and the recording measured 42.0 dB of
  separation. This exercises the real protocol decoder, DSP, and WAV writer, but
  not PulseAudio and not a real receiver.

Still outstanding for 1.1.0: reception from a real SDR++ server, stereo playback
through PipeWire (the stream should now report `float32le 2ch 48000Hz`), the
mono fallback on a weak or mono station, and the panel's `Listening · FM stereo`
label on screen.

## 1.2.0 — bookmarks and mute, on screen

- The eleven Rust tests, Clippy, `omarchy plugin validate`, and QML lint were
  rerun. Lint covers `Bookmarks.qml` now and still emits only the
  `QProcess::ExitStatus` warning, once per file that handles `Process.onExited`.
- The packaged folder was copied into `~/.config/omarchy/plugins/` and the panel
  was confirmed **on screen**: both bookmark strips with their `+` buttons, the
  empty-state hints, the mute button beside the volume readout, and the
  `Alt+1...9` footer.
- Seeding `~/.local/state/omarchy/omasdr.json` with two stations and one server
  and restarting the shell drew all three as shortcuts, with the entries
  matching the current frequency and server highlighted. The seeded file was
  removed afterwards.
- Reaching a reloaded plugin needs `omarchy-restart-shell`, not
  `omarchy-shell shell rescanPlugins`: the launcher runs Quickshell with
  `QS_DISABLE_FILE_WATCHER=1`, so a rescan re-reads the registry but keeps
  serving the QML the engine already compiled.

- The full bookmark round trip was driven through the panel with `wtype` and
  checked against both the screen and the file: F2 on a focused bookmark opened
  the editor prefilled and selected; renaming `Local` to `Studio` updated the
  shortcut and the JSON; Remove emptied `servers`; `+` created it again. `Alt+2`
  tuned 104.6 MHz and moved the highlight to the second station.

- After the gesture rework, F2 on a focused server bookmark was re-checked: the
  editor opens with Save and Remove, and Escape leaves the stored file untouched.
- **On air, against a real RTL-SDR v4 behind an SDR++ server at 1.536 MHz.** The
  stereo blend was the fix for audible hiss: measured noise floors were 0.0048
  (102.4 MHz), 0.028 (88.8) and 0.057 (104.6) against a 0.0005 clean reference,
  with textbook pilots of 0.034–0.049, so every station was correctly judged too
  noisy for full-bandwidth stereo. Strong carriers on a poor floor across the
  whole band points at front-end overload, not weak signal; that is upstream of
  this plugin and untested here.
- Fifteen Rust tests now, adding the audio response at both ends, the stereo
  blend narrowing with noise, the denoiser's reconstruction of a tone it should
  keep, its transparency on a clean carrier, the stereo/quiet trade it forces,
  and the WAV header parsed back field by field. The mock-server suite gained a
  round trip of the `noiseReduction` command through the stats line.
- `preview.png` was retaken for this version. It uses placeholder bookmarks and
  a loopback address, not the development machine's own.

Not tested at 1.2.0: the mouse gestures themselves, and F2 driven by hover
rather than focus — no pointer-synthesis tool is installed and this Hyprland's
`dispatch movecursor` syntax rejected every form tried. Nor was a server
bookmark edited while actually connected — no pointer-synthesis tool
is installed, so right-click was exercised only through `beginRename`, the same
function `onRightClicked` calls. Mute during playback and everything on-air that
1.1.0 left outstanding also remain untested.
