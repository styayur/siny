$ErrorActionPreference = 'Stop'
$ortPath = (Resolve-Path '.native-build/helper/onnxruntime').Path.Replace('\', '/')
$ortCommit = (git -C $ortPath rev-parse HEAD).Trim()
if ($ortCommit -ne '0d68ff6b3b72b04aac578decd6c4c45d322bb962') { throw 'ORT source identity mismatch' }
if ((Get-Content (Join-Path $ortPath 'VERSION_NUMBER') -Raw).Trim() -ne '1.28.3') { throw 'ORT version mismatch' }
git -C $ortPath submodule update --init --depth=1 --recursive
if ($LASTEXITCODE) { throw 'ORT submodule checkout failed' }
$outputPath = [IO.Path]::GetFullPath('.native-build/output').Replace('\', '/')
cmake -S .native-build/helper/static_lib -B .native-build/build -A x64 -DCMAKE_BUILD_TYPE=Release -DCMAKE_CONFIGURATION_TYPES=Release -DUSE_STATIC_CRT=ON -Donnxruntime_BUILD_UNIT_TESTS=OFF "-DONNXRUNTIME_SOURCE_DIR=$ortPath" "-DCMAKE_INSTALL_PREFIX=$outputPath"
if ($LASTEXITCODE) { throw 'Native CMake configure failed' }
cmake --build .native-build/build --config Release --parallel 2
if ($LASTEXITCODE) { throw 'Native CMake build failed' }
cmake --install .native-build/build --config Release
if ($LASTEXITCODE) { throw 'Native CMake install failed' }
cmake -S .native-build/helper/static_lib/tests -B .native-build/smoke -A x64 "-DONNXRUNTIME_INCLUDE_DIR=$outputPath/include" "-DONNXRUNTIME_LIB_DIR=$outputPath/lib"
if ($LASTEXITCODE) { throw 'Native smoke configure failed' }
cmake --build .native-build/smoke --config Release
if ($LASTEXITCODE) { throw 'Native smoke build failed' }
ctest --test-dir .native-build/smoke --build-config Release --verbose --no-tests=error
if ($LASTEXITCODE) { throw 'Native smoke test failed' }
python scripts/prepare_native.py
if ($LASTEXITCODE) { throw 'Pinned sherpa preparation failed' }
$originalPath = (Resolve-Path '.native/sherpa-onnx-v1.13.8-win-x64-static-MT-Release-lib/lib').Path
$securePath = [IO.Path]::GetFullPath('.native-build/secure-lib')
New-Item -ItemType Directory -Force $securePath | Out-Null
Copy-Item "$originalPath/*.lib" $securePath
Copy-Item "$outputPath/lib/onnxruntime.lib" "$securePath/onnxruntime.lib" -Force
"SHERPA_ONNX_LIB_DIR=$securePath" | Add-Content $env:GITHUB_ENV
New-Item -ItemType Directory -Force work/native-security | Out-Null
Get-ChildItem "$securePath/*.lib" | ForEach-Object { [pscustomobject]@{ name=$_.Name; sha256=(Get-FileHash $_.FullName -Algorithm SHA256).Hash } } | ConvertTo-Json | Set-Content work/native-security/native-hashes.json
