# Rebuild the native implementation

Ordinary Windows builds use `scripts/prepare_native.py`, which verifies the pinned SDK
and every library against `docs/native-sdk.json`. The original ORT 1.28.2 input is reserved
for constructing the other sherpa libraries and is replaced before acceptance.

The release supplies `siny-application-source.tar.gz` with application source, the lockfile,
Cargo vendor directory and notices, plus `siny-corresponding-source.tar.gz` with the native
source trees and original native dependency archives. Verify SHA256SUMS.txt before use.
The native archive retains upstream source fixtures; no production SenseVoice/Kokoro pack,
user input, captured audio or generated speech is supplied.

Use Windows x64, MSVC C++ Build Tools, CMake 3.28 or newer, Git, Python 3.12 or newer and
current stable Rust. The recipe is `native/ort` and `scripts/native-security-build.ps1`.
It combines the configured public ORT dependency graph and uses the static release CRT.
The C API smoke target requires C++17. The release's native build evidence retains the
actual CMake cache, source SHA, library hashes and numerical inference reports. Different
compiler/runner revisions may produce different bytes; bit-identical rebuilds are not claimed.

The source archive intentionally excludes `.git` directories and checkout credentials.
The recipe's identity check expects a Git checkout. Establish the same immutable upstream
identity through the verified mirror before running it (see `docs/native-provenance`):

```powershell
git clone --no-checkout https://github.com/ossia/onnxruntime .native-build/onnxruntime
git -C .native-build/onnxruntime checkout --detach 0d68ff6b3b72b04aac578decd6c4c45d322bb962
git -C .native-build/onnxruntime submodule update --init --depth=1 --recursive
New-Item -ItemType Directory -Force work | Out-Null
$env:GITHUB_ENV = Join-Path $PWD 'work/rebuild-environment.txt'
./scripts/native-security-build.ps1
$env:SHERPA_ONNX_LIB_DIR = (Resolve-Path '.native-build/secure-lib').Path
```

This is ORT 1.28.3 with sherpa's 1.13.8 ABI, not a sherpa 2.x upgrade. The mirror retains
the signed official commit; it is not an alternative implementation. Original source and
licences remain in the supplied native archive even if a hosting endpoint later disappears.
The native recipe's Git identity setup still requires reachable immutable Git metadata;
a fully offline rebuild from the archive has not been verified. Do not substitute a
different commit or disable the identity check to label a build as the pinned release.

The archive's `native-src/fetched` contains CMake dependency sources used by the build;
`native-src/archives` contains the reviewed original native source archives and inventory.
The source manifests and upstream CMake scripts pin dependency downloads. Installing the
compiler/SDK and fetching identity metadata may require a network connection. Siny's runtime
does not fetch models or software. Rebuilding does not by itself qualify a release: run the
C API smoke, actual model acceptance, audit, licence inventory and Windows/device checks.
