# Acceptance and migration gate

Status: PARTIAL — implementation and real execution in progress. CI success is not itself
proof of real microphone, output-device, shortcut or GUI acceptance.

Required before replacement/archival: actual SenseVoice sample inference, Kokoro English,
Chinese, speed/voice coverage, unknown-word regression, reverse verification, cancellation,
recovery, microphone capture, playback pause/resume/stop, clipboard/hotkeys, resource and
licence inventory. Run the CI probes against the exact release SHA. Keep model/audio inputs
local; publish numerical reports and hashes only.

The next-alpha source implements tray controls/background logic, dictation copy and guarded
opt-in paste, explicit legacy-config preview/import, WAV CLI, live language/terms/idle settings,
named model voices, optional capture retention/debug export, incremental long-text reading and
atomic streamed WAV export. [Migration contract](parity-matrix.md) defines the actual old-app
capabilities and acceptance owner. Code implementation is not final Windows product acceptance.
Original repositories stay open until exact-main/release and real Windows checks qualify the
new build. Neither repository currently has an open user issue to transfer.

## DSP evaluation

Push-to-talk bounds capture to 60 seconds. Neural VAD would add a model/load/RAM cost and
has not demonstrated a measured benefit; no VAD accuracy or latency improvement is claimed.
Recording stops playback (half-duplex), avoiding speaker feedback and the compute/latency
cost of AEC; simultaneous conversation / AEC remains deferred until a measured echo fixture
exists. Resampling is needed for actual device rates and 24 kHz TTS reverse recognition.
SenseVoice receives the actual synthesis sample rate through sherpa's native resampler.
The shared microphone path uses sherpa's bandlimited low-pass resampler. A deterministic
12 kHz tone at 48 kHz converted to 16 kHz gave RMS 0.0006614895, versus 0.3535534022 for
the inherited linear interpolation: approximately 54.56 dB less alias energy. Processing
took 0.0005967 seconds per second of fixture audio on Windows CI. This does not establish
speech-accuracy gains or a real microphone performance benchmark. TinySTT's separate
streaming linear resampler also has a boundary regression fix; it is not used for Siny capture.
No claim that ASR and TTS model weights can be merged is made. Shared terminology is a text
transform, and reverse recognition is an explicit diagnostic vulnerable to correlated errors.

## Design decisions

Desktop reader/dictator, one editable transcript, Listen → Text → Speak strip, adjacent
record/read actions, visible reverse transcript. Neutral paper/graphite and blue focus use
the inherited egui 0.37 accessible controls, 4 px spacing base and 40 px action height.
No dashboard cards, animations or chat bubbles. Real window inspection is required.

The first integrated native round trip failed at 0.833 because SenseVoice ITN emitted
`1,2, three` for the spoken `One, two, three`. The comparison now canonicalizes standalone
English zero..ten tokens, with explicit equivalence and omission regressions. The input
sample and 0.85 acceptance threshold are retained. This does not provide general numeric,
date or multilingual semantic normalization. The GUI similarity remains diagnostic.
