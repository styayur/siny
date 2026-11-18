# Siny
Scope: Siny and imported TinySTT/TinyTTS only. Preserve both source histories and licences.
Use normal PRs and current required checks. No bypass, retired evidence/R3/R4 gates,
model calls in governance, hidden advisories, or premature legacy repository archival.
Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets --all-features -- -D warnings,
cargo test --workspace --all-features, cargo audit and Python trust-boundary tests.
Native inference and Windows/device acceptance require real execution, not mocks.
Record missing evidence explicitly. Never commit models, audio, secrets or personal data.
