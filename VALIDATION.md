# Local validation — 2026-09-08

Tested on x86-64 Omarchy Quattro, with its existing PipeWire/PulseAudio service.

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
no station identity or subjective listening-quality claim is made. Marketplace
submission and installation from a public Git repository have not been done.
