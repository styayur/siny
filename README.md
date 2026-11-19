# Siny

Lightweight native offline dictation and speech for Windows x64. SenseVoice recognizes;
Kokoro reads. Both models remain separate and load only when requested.

**Integration preview under validation.** This branch is not an accepted replacement
for TinySTT/TinyTTS. Neither old repository is archived. See [acceptance](docs/acceptance.md).
The published [alpha.1](https://github.com/styayur/siny/releases/tag/v0.1.0-alpha.1)
retains its qualified original bytes. Migration work for the next alpha follows the
[product parity matrix](docs/parity-matrix.md).

## Use

Record or hold Ctrl+Alt+D to dictate. Read aloud speaks the editor; Ctrl+Alt+S reads
the clipboard. Read after dictation connects the two. Verify speech → text runs synthesized
audio back through SenseVoice and displays character similarity. This is a diagnostic,
not a guarantee of pronunciation or recognition accuracy. Dictation copies its result;
automatic paste is off by default and only targets the still-focused application
where recording started. Reading and verification do not replace the clipboard.
Pause, Resume and Stop control streamed playback. **Export speech WAV** renders the
editor incrementally to an explicitly selected file, without retaining the whole
document's PCM in memory. Cancellation/error keeps an existing destination intact.

No account, server, telemetry, updater or runtime downloads. Models are **not** included.
Install the reviewed models separately under `models/sensevoice` and `models/kokoro`
next to `siny.exe`; see the imported [SenseVoice](crates/tinystt/docs/model-provenance.md)
and [Kokoro](crates/tinytts/docs/model-provenance.md) provenance. Absolute model paths
can be saved in Settings. Configuration is `config/siny.toml` beside the executable.
Language, dictation shortcut, idle timeout, auto-paste and shared terms are editable
in Settings. Changes apply after **Save & apply**, without restarting; failed saves
keep the previous active settings. **Import TinySTT/TinyTTS config** reads an explicitly
selected legacy TOML file into the settings preview. It preserves that file, resolves
relative model paths against the old application directory and maps reviewed Kokoro
voice names to stable IDs. Review the preview, including shared threads, before saving.
Shared terms are exact, case-sensitive, longest substring replacements applied once;
add `[[terms]]`, `from = "recognised spelling"`, `to = "preferred spelling"` to that file.
Old config files are not overwritten or silently migrated.

The system tray offers Open, Enable/Disable shortcuts, Stop/Cancel and Exit. Closing
the window keeps Siny in the tray by default; disable this in Settings or choose
**Exit Siny** to quit. If the tray cannot be created, closing exits normally. Hidden
windows keep processing shortcuts and tasks without redrawing the editor while idle.

Microphone-free file recognition is available from a terminal:

```powershell
.\siny.exe --file .\sample.wav --model-dir .\models\sensevoice --language en --num-threads 2
```

File mode writes recognition to stdout, opens no GUI/audio devices/clipboard and
returns 1 on execution errors or 2 on invalid arguments. `--config <siny.toml>` chooses
an explicit Siny config; command options override it. WAV files are bounded at 256 MiB
and one hour. `--help` lists options.

The worker accepts two waiting tasks; the GUI accepts one active task. Microphone
input is capped at 60 seconds; long-form reading/export accepts up to 1,000,000
characters and has no two-minute total-output cap. The production audio channel
holds two chunks; the player separately limits its internal sink to two chunks.
Reverse-verification diagnostics retain 10,000 characters / two minutes of audio.
Models unload
following 60 seconds of worker inactivity (configurable) or Unload models. Recording stops
playback: half-duplex prevents self-triggering without claiming acoustic echo cancellation.
Native ASR cannot be interrupted inside a decode; cancelled output is discarded afterward.
Named voices preserve the reviewed Kokoro IDs; **Load saved model voices** queries
the actual model through the same worker. Retain recording is off by default. If
enabled in Settings, a finished capture stays in memory only until the next capture,
cancel/stop, disabling retention or exit. **Save debug recording** is an explicit
file action; cancelled captures and recordings are never automatically written.

## Build and verify

Use stable MSVC Rust and Visual Studio C++ Build Tools on Windows x64. The supplied SDK
requires MSVC 14.51 or newer (CI uses Visual Studio 2026); the older 14.44 C++ runtime
cannot resolve its newer `__std_*` symbols. Running the portable app does not require
the compiler. Windows 10 compatibility remains unverified on a physical machine.

```powershell
python scripts/prepare_native.py
$env:SHERPA_ONNX_LIB_DIR = (Resolve-Path '.native/siny-sherpa-1.13.8-ort-1.28.3-win-x64-static-MT/lib').Path
cargo build --locked --release -p siny
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo audit
python -m unittest discover -s tests
```

For real inference, explicitly download the pinned models using `scripts/prepare_model.py`
and `scripts/prepare_sensevoice.py`, set `TINY_TTS_TEST_MODEL` and `SINY_TEST_ASR` to their
printed directory names under `.test-model`, build both acceptance examples as in CI,
then run `python scripts/check_acceptance.py`. Those scripts are developer tools only.
They verify archive size and SHA-256 before extracting and reject unsafe archive members.
No model or generated audio is published as an artifact.

`cargo test -p siny-core` exercises shared services without native inference.
`cargo build -p siny --no-default-features --features sherpa` enables a headless backend;
`gui` is the default. Legacy executables remain workspace members for compatibility work.

## Source history and licences

TinyTTS main `8619b1d62c74a1a725fb7e87924d69ba02a2d2db` and TinySTT main
`ebfbbf50e85000bedf40f563a36173e7d920f149` are full, unchanged ancestors of this repository.
Imports use subtree merges without squashing; no original repository history was rewritten.
The initial TinySTT local main was older; the subsequent subtree merge imports current main.

New Siny source is AGPL-3.0-only. TinySTT retains AGPL-3.0-only; TinyTTS retains
GPL-3.0-or-later. The GPL portion is not relicensed; the combined executable uses GPLv3 §13 /
AGPLv3 §13 compatibility, including AGPL network-interaction obligations. Keep both licence
texts, original notices and attribution. See [notices](THIRD_PARTY_NOTICES.md).
