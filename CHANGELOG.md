# Changelog

All notable changes to this project are documented here, in
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) form. This project
follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.0] - 2026-09-08

### Added

- **Named bookmarks** for both stations and servers, shown as a row of
  shortcuts under the field each one fills in. `＋` saves what is currently in
  the field and asks for a name; right-clicking a bookmark — or pressing F2 with
  it under the pointer — opens it for renaming or removal. The bookmark matching
  the current value is highlighted. While connected, server bookmarks can still
  be renamed and removed; only switching server waits for Disconnect.
- **Alt+1…9** tunes the first nine saved stations without leaving the keyboard.
  Alt is the modifier because a bare digit belongs to the frequency field.
- A **mute button** beside the volume slider. Muting leaves the slider where it
  is and the readout reads `Muted`; moving the slider unmutes.

### Notes

- Bookmarks persist to `$XDG_STATE_HOME/omarchy/omasdr.json` (by default
  `~/.local/state/omarchy/omasdr.json`), next to the shell's own state. They
  cannot live in the widget's Omarchy settings: `settings` reaches the widget
  one-way from the bar's `shell.json` entry, so the panel cannot write to it.
- An unreadable bookmarks file is left alone rather than overwritten, and a
  single malformed entry drops itself instead of the whole list.

## [1.1.0] - 2026-09-08

### Added

- **Stereo reception.** The receiver locks a phase-locked loop to the 19 kHz
  pilot, recovers the L−R difference from its suppressed 38 kHz carrier, and
  matrixes left and right. The panel reports `stereo` or `mono` while playing,
  and re-announces itself if a station changes mode.
- Weak signals fade back to mono in proportion to the recovered pilot level
  rather than switching, so a fading station loses separation before it gains
  hiss. A station with no pilot stays exactly as loud as it was in 1.0.0.

### Changed

- `--wav` now writes a two-channel file, and audio output opens a stereo
  stream. De-emphasis moved after the difference channel is brought down to
  baseband, where it belongs.

## [1.0.0] - 2026-09-08

First release.

### Added

- A radio icon in the bar whose panel connects to an **SDR++ native server**,
  tunes a frequency, and plays mono broadcast FM through the desktop's default
  audio output. The icon carries a dot while a station is playing.
- Tuning across 65–108 MHz. Type either `102.4` or `102,4` and press Enter, or
  step in 100 kHz with the − and + buttons. Retuning restarts the demodulator
  and mutes briefly so the change lands without a burst of noise.
- A volume slider that affects only the radio, never the desktop's own output.
  Arrow keys move it in 5% steps, Home and End reach silence and full.
- `bin/omasdr`, the bundled receiver the panel drives, usable on its own from a
  terminal: `--server`, `--frequency`, `--volume`, plus `--seconds` and `--wav`
  for a bounded recording and `--no-audio` for a headless check.
- Optional `server`, `frequency`, and `volume` settings in the widget's
  `shell.json` bar entry, overriding the shipped defaults of `127.0.0.1:5259`,
  102.4 MHz, and 30%.

### Notes

The receiver is a Rust executable committed to this repository, not a script:
Omarchy clones a plugin and runs no build steps or install hooks, so the
binary has to be present already. `bin/SHA256SUMS` and `bin/build-info.json`
record its checksum alongside the hash of every source file it was built from,
and the release workflow re-verifies both against the tag. The full Rust
source, lockfile, tests, and build script are in the repository.

Audio goes through Omarchy's existing PipeWire/PulseAudio service. No daemon,
driver, or SDR package is installed, nothing runs privileged, and closing the
panel keeps playing — only Disconnect, disabling the plugin, or unloading it
stops reception.

Version 1.0.0 is mono only, with European 50 µs de-emphasis. Source selection,
gain, and sample rate stay with SDR++; the server must already have a working
radio source and a sample rate between 240 kHz and 20 MHz.

[Unreleased]: https://github.com/marvreichmann/omasdr/compare/v1.2.0...HEAD
[1.2.0]: https://github.com/marvreichmann/omasdr/releases/tag/v1.2.0
[1.1.0]: https://github.com/marvreichmann/omasdr/releases/tag/v1.1.0
[1.0.0]: https://github.com/marvreichmann/omasdr/releases/tag/v1.0.0
