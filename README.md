# Siny

Lightweight native offline dictation and speech for Windows x64. SenseVoice recognizes;
Kokoro reads. Both models remain separate and load only when requested.

**Integration preview under validation.** This branch is not an accepted replacement
for TinySTT/TinyTTS. Neither old repository is archived. See [acceptance](docs/acceptance.md).

## Use

Record or hold Ctrl+Alt+D to dictate. Read aloud speaks the editor; Ctrl+Alt+S reads
the clipboard. Read after dictation connects the two. Verify speech → text runs synthesized
audio back through SenseVoice and displays character similarity. This is a diagnostic,
not a guarantee of pronunciation or recognition accuracy. Copy text is explicit; no auto-paste.
Pause, Resume, Stop and WAV export are available after synthesis.

No account, server, telemetry, updater or runtime downloads. Models are **not** included.
Install the reviewed models separately under `models/sensevoice` and `models/kokoro`
next to `siny.exe`; see the imported [SenseVoice](crates/tinystt/docs/model-provenance.md)
and [Kokoro](crates/tinytts/docs/model-provenance.md) provenance. Absolute model paths
can be saved in Settings. Configuration is `config/siny.toml` beside the executable.
Shared terms are exact, case-sensitive, longest substring replacements applied once;
add `[[terms]]`, `from = "recognised spelling"`, `to = "preferred spelling"` to that file.
Old config files are not overwritten or silently migrated.

The worker accepts two waiting tasks; the GUI accepts one active task. Inputs are capped
at 60 seconds / 10,000 characters; synthesized output at two minutes. Models unload
following 60 seconds of worker inactivity (configurable) or Unload models. Recording stops
playback: half-duplex prevents self-triggering without claiming acoustic echo cancellation.
Native ASR cannot be interrupted inside a decode; cancelled output is discarded afterward.

## Build and verify

Use stable MSVC Rust and Visual Studio C++ Build Tools on Windows 10/11 x64.

```powershell
python scripts/prepare_native.py
$env:SHERPA_ONNX_LIB_DIR = (Resolve-Path '.native/sherpa-onnx-v1.13.8-win-x64-static-MT-Release-lib/lib').Path
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
