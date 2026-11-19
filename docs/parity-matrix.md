# Siny migration parity

Owner: the Siny source/release agent. This is executable work for alpha.2, not an
ownerless deferral list. Alpha.1 and its published bytes/tags remain unchanged.

The original Siny mission requires the two applications' necessary source histories,
one Rust workspace/backend/device/config/shortcut/clipboard/task/model lifecycle,
dictation, reading, dictation then reading, shared terms, reverse recognition,
real native/audio/cancel/recovery/performance/security acceptance and a Windows
release. Legacy issue migration, README replacement and archival follow **product
parity and acceptance**, not merely CI or the existence of an alpha.

## Migration contract

| Capability | Actual legacy source | Alpha.1 | Implementation and acceptance owner |
| --- | --- | --- | --- |
| Push-to-talk with configurable shortcut; microphone/language/threads | TinySTT settings/hotkey/app | Fixed shortcut; some file-only settings | Siny: editable validated settings, apply without restart, registration errors visible |
| Automatic transcript copy; opt-in paste | TinySTT app/clipboard/paste | Explicit copy only | Siny: dictation-only automatic copy; paste defaults off; cancelled jobs never deliver |
| Tray open, hotkey toggle, exit; close-to-tray | TinySTT tray/app | Missing | Siny: keep UI worker responsive while hidden; visible fallback if tray fails; explicit Exit |
| Existing model/config reuse | Both settings/paths | No import | Siny: explicit per-app import, preserve source config, resolve old relative model paths, preview before save |
| Microphone-free WAV recognition CLI | TinySTT main/audio/wav | Missing | Siny: --file/model-dir/num-threads/language, stdout and exit codes; no GUI/devices/clipboard in CLI |
| Long-text segmented playback, pause/resume/stop | TinyTTS pipeline/queue/player | Whole output capped at 120 seconds | Siny: streamed chunks, bounded queued audio, cancel under backpressure, first audio before completion |
| Named voices and speed | TinyTTS sherpa_kokoro/settings/app | Numeric ID only | Siny: preserve 54-model speaker IDs and names, selected-model metadata; validated names on import |
| Optional debug microphone WAV export | TinySTT app/audio/wav | Only synthesized WAV | Siny: explicit opt-in capture retention and explicit file save; cancelled capture not retained |
| Synthesized WAV export | TinyTTS app/audio/wav | Present, finite synthesis only | Siny: retain explicit export; use streamed file output for long text rather than accumulating unlimited RAM |
| Shared terms, read-after-dictation, reverse verification, lazy/idle unload | Required Siny additions | Implemented | Siny: retain actual native/cancel/recovery regressions and add live settings controls |
| Source histories, copyrights, native ABI, security/licences | Imported Git histories and notices | Qualified | Siny: preserve four legacy ancestors, sherpa 1.13.8 / ORT 1.28.3 and existing gates |

Bounded inputs/queues and explicit export are required to preserve the lightweight
offline product. "Long text" means incremental generation/playback without a
two-minute total-audio cap; it does not require unbounded memory or an infinite
editor. No VAD/AEC, model fusion, speech accuracy guarantee, updater, signing-key
creation or unrelated platform expansion is added to the parity contract.

## Delivery sequence

1. Settings/import, WAV CLI and dictation clipboard/paste delivery, including
   deterministic trust-boundary tests and actual native CLI acceptance.
2. Tray lifecycle and configurable hotkeys, with real Windows tray/device checks.
3. Streaming long text, model voice names, optional debug capture and streamed
   WAV export; cancellation/backpressure tests and actual speech/audio acceptance.
4. Qualify exact main, publish a new alpha.2 identity, verify anonymous downloads,
   then decide legacy migration/README/archival from observed acceptance.

Each stage uses normal PRs and current CI/security/licence protections. This matrix
records the migration contract; per-PR and exact-release evidence is recorded in
the acceptance document and release verification manifest. No unfinished stage is
accepted by this document.
