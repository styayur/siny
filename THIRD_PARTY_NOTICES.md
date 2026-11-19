# Siny source and third-party notices

Copyright (c) Siny contributors. New `crates/siny*` source and integration scripts are
AGPL-3.0-only unless marked otherwise. Full text: LICENSE. No warranty.

Imported TinySTT: https://github.com/styayur/TinySTT at ebfbbf50e85000bedf40f563a36173e7d920f149,
AGPL-3.0-only. Imported TinyTTS: https://github.com/styayur/TinyTTS at
8619b1d62c74a1a725fb7e87924d69ba02a2d2db, GPL-3.0-or-later (LICENSE-GPL-3.0).
The imports preserve authorship in Git history, all original notices, and original licence
texts within each member. Siny does not claim ownership of third-party code or relicense it.
Changes to the imported members are recorded in the integration commits.

The combination relies on the express compatibility provisions in §13 of both licences:
https://www.gnu.org/licenses/gpl-3.0.html#section13 and
https://www.gnu.org/licenses/agpl-3.0.html#section13.
GPL-covered portions retain their GPL permissions and notices.

See crates/tinytts/THIRD_PARTY_NOTICES.md and crates/tinystt/THIRD_PARTY_NOTICES.md
for model and native dependencies. Models, voices and espeak data are NOT distributed here.
The local Apache-2.0 sherpa-onnx-sys fork is in third-party/sherpa-onnx-sys, with original
licence and per-file provenance. ZIP 9.1.3 is a build-time backport; native ABI stays 1.13.8.

The Windows executable statically links upstream sherpa-onnx and its native dependencies.
Binary redistribution requires their licence texts and corresponding source for copyleft
components, in addition to the Cargo dependency licence inventory. Until this inventory and
source delivery are verified, public binary release is gated. A source repository alone is
not proof of fulfilment of every binary distribution obligation.
