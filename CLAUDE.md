# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

An Omarchy bar-widget plugin (`com.github.marvreichmann.omasdr`): a QML panel for the Omarchy/Quickshell
shell plus a bundled Rust executable that talks the **SDR++ native server protocol**,
demodulates stereo broadcast FM, and plays it through PulseAudio/PipeWire.

Two languages, one process boundary. Everything interesting is at that boundary.

## Commands

```sh
cargo test --locked                       # unit + integration (integration opens local TCP sockets)
cargo test --locked recovers_fm_tone      # single test by name substring
cargo test --locked --test session        # only the integration suite
cargo clippy --locked --all-targets -- -D warnings
bash scripts/build.sh                     # release build → bin/, dist/, archive; runs `omarchy plugin validate`
bash scripts/lint-qml.sh                  # qmllint with Omarchy's qs.* imports symlinked in
```

Live smoke test against a real server (WAV path must not already exist):

```sh
bin/omasdr --server HOST:5259 --frequency 102.4 --seconds 12 --wav /tmp/fm-test.wav
```

`--no-audio` skips PulseAudio entirely — that is how the integration tests run headless.
`--seconds` bounds a run and makes "no audio received" a hard error, so unattended
tests fail loudly instead of hanging.

## Architecture

**QML → Rust is one-way JSON over stdin; Rust → QML is JSON lines over stdout.**

- `BarWidget.qml` — bar icon, owns the `Receiver` and lazily loads the panel. Reads
  `server`/`frequency`/`volume` from the widget's Omarchy settings.
- `Receiver.qml` — the process wrapper. Spawns `bin/omasdr` via Quickshell `Process`,
  writes `{"frequency":…}` / `{"volume":…}` / `{"stop":true}` lines, parses stdout
  events into `phase` + `status` + `failed`. Panel and bar are pure views over these.
- `Panel.qml` — Omathought-styled UI; holds no state, calls `receiver.tune()` /
  `receiver.setVolume()` / `receiver.toggleMute()`. `BookmarkStrip` is one inline
  component used twice, over the station list and the server list; only its
  accessors differ. Chips are `Ui.Button`, which owns a full-size `MouseArea`:
  child pointer handlers never fire, so `clicked` and `rightClicked` are the only
  gestures available — there is no double-click to bind, and F2 exists because
  right-click has no keyboard equivalent. Closing the editor hands focus back to
  the chip; without that the focus lands nowhere and Tab order restarts. The
  strip's `switchable` gates only activation — setting `enabled: false` on a
  strip to stop mid-session server switching also kills rename and remove.
- `Bookmarks.qml` — named stations and servers in
  `$XDG_STATE_HOME/omarchy/omasdr.json`. Writes are refused until the first load
  settles, so a slow read cannot blank an existing file, and an unparseable file
  is kept rather than overwritten.
- `src/main.rs` — arg parsing, a stdin reader thread (bounded line length), and one
  blocking select-ish loop: read socket → frame → dispatch → demodulate → push audio.
  Emits `event()` JSON lines; `stats` lines once a second (QML ignores them).
- `src/protocol.rs` — `Framer` reassembles length-prefixed SDR++ packets across TCP
  reads; `samples()` converts int8/int16/float32 IQ payloads. Both reject malformed
  input rather than tolerating it.
- `src/dsp.rs` — `Fm`: halfband decimation stages down to ≤600 kHz, 100 kHz channel
  FIR, phase discriminator, a 19 kHz pilot PLL whose doubled carrier brings L−R down
  from 38 kHz, 50 µs de-emphasis on sum and difference, 15 kHz audio FIR evaluated
  through a 64-phase fractional-delay bank at 48 kHz, DC blocker. `process` writes
  **interleaved** left/right pairs. The pilot correlation doubles as the lock
  detector and drives a mono/stereo blend, so an unlocked loop degrades to mono.
- `src/audio.rs` — hand-written `pa_simple_*` FFI (no crate). Playback lives on its
  own thread behind a bounded `sync_channel`; a full channel drops a block rather
  than stalling the receive loop, and `failed` surfaces device loss to the main loop.

Constraints worth knowing before changing things:

- The **only** runtime dependency is `serde_json`. Adding a crate means new license
  notices in `licenses/` and a bigger committed binary — do not add one casually.
- `bin/omasdr` is **committed** with its executable bit, plus `bin/SHA256SUMS` and
  `bin/build-info.json`. Omarchy clones the repo and runs no build hooks. Any change
  to `src/` or `Cargo.*` invalidates those files — rerun `scripts/build.sh`.
- `manifest.json` and `Cargo.toml` versions must match; `scripts/release.py` asserts it.
- The packaged plugin must contain no symlinks (also asserted in `release.py`).
- Audio is stereo everywhere downstream of `Fm::process`: the PulseAudio spec, the
  WAV header, the `chunks(1920)` push size, and the `audioSamples` stat (which counts
  frames, not samples). Changing the channel count means changing all four.
- Bookmarks cannot live in the widget's Omarchy settings: `settings` is one-way
  from the `shell.json` bar entry, so the panel cannot write a bookmark back.
- A new QML file must be added in three places: `scripts/release.py`'s packaged
  file list, `scripts/lint-qml.sh`, and whatever loads it.
- The frequency range 65–108 MHz and the volume range 0–1 are validated in three
  places (Rust args, Rust stdin commands, QML `tune`). Keep them in sync.
- Server sample rate must be 240 kHz–20 MHz; anything else is a hard `Fm::new` error.
- `Denoiser` is SDR++'s FM IF noise reduction: a 32-bin sliding FFT keeping only
  the strongest bin. Because one bin survives, the inverse transform at the
  window's centre is `X[idx] * (-1)^idx`, so it costs one forward FFT per input
  sample and no second transform. It cannot carry stereo at any rate we run at:
  passing the 53 kHz composite would need bins ~106 kHz wide, i.e. a 3.4 MHz
  channel rate. Treat it as an alternative to stereo.
- The stereo blend reads the 76 kHz noise floor (4× the pilot, so the PLL's own
  oscillator supplies the carrier), not pilot amplitude: a pilot survives noise
  the difference channel does not. The post-mix lowpass is four cascaded poles
  because one lets the 53 kHz programme edge through at only −21 dB, which
  swamps the measurement. `NOISE_STEREO`/`NOISE_MONO` are calibrated against a
  zero-noise floor of 0.0003–0.0008 across the supported rates.
- Compressed IQ (packet kind 3) is refused by design — compression is disabled at
  handshake, so receiving it means the server misbehaved.
- `Panel.qml` deliberately has no `PanelKeyCatcher`: the form's fields need Tab,
  so the panel gives up Tab-to-adjacent-panel switching. Escape still closes.
- `manifest.json`'s `barWidget.schema` has no consumer in Omarchy 4.0.0.alpha —
  no shell version renders it yet. `settings` reaches the widget only from the
  `shell.json` bar entry, so the `setting()` fallbacks in `BarWidget.qml` are
  load-bearing and must keep matching `barWidget.defaults`.

## Releasing

Versions live in three places that must agree: `manifest.json`, `Cargo.toml`
(hence `Cargo.lock`), and `bin/build-info.json` — plus the `--help` banner in
`src/main.rs`. Bump them, rerun `scripts/build.sh`, then add the section to
`CHANGELOG.md` (Keep a Changelog form, one `## [X.Y.Z] - DATE` heading and a
matching link at the bottom).

Pushing a `vX.Y.Z` tag triggers `.github/workflows/release.yml`, which reruns
the tests and clippy, refuses a tag that disagrees with the manifest, the crate,
or `bin/build-info.json`, re-verifies the committed executable against
`bin/SHA256SUMS` and every source hash in `build-info.json`, and publishes a
GitHub release whose notes are that changelog section. Releases carry no
attached assets — installation is `omarchy plugin add`, matching the other
plugins on this account.

## Docs

`PUBLISHING.md` is the marketplace checklist (validate → runtime-test the copy in
`~/.config/omarchy/plugins/` → commit binary → tag → submit). `VALIDATION.md`
records what was actually tested and on what hardware; update it when
re-validating, and keep its claims to what was measured.
