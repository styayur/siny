# Security

Report privately at https://github.com/styayur/siny/security/advisories/new .
Siny has no runtime HTTP server, telemetry or automatic downloads. Clipboard reads happen
only on a requested reading action; transcript copying and WAV export are explicit.
Models and config are local inputs. Obtain models from the pinned upstream sources; do not
load untrusted ONNX graphs. Build/test tooling verifies archive identities before extraction.
No advisory exceptions or suppression files are used. Native dependency risks require
separate review; cargo audit covers Rust crates only. Only the newest validated release
receives security fixes. Remove private text/audio from reports.
