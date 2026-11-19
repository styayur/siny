"""Package validated native inputs alongside corresponding source and notices."""
import hashlib, json, os, re, shutil, subprocess, tarfile, zipfile
from pathlib import Path, PurePosixPath
ROOT = Path(__file__).resolve().parents[1]
ORT_COMMIT = '0d68ff6b3b72b04aac578decd6c4c45d322bb962'
NAME = 'siny-sherpa-1.13.8-ort-1.28.3-win-x64-static-MT'
def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()
def run(args, **kwargs):
    return subprocess.run(args, cwd=ROOT, check=True, **kwargs)
def portable_vendor_config(config):
    return re.sub(r'^directory\s*=.*$', 'directory = "vendor"', config, flags=re.MULTILINE)
def source_filter(info):
    return None if any(p in ['.git', '__pycache__'] for p in Path(info.name).parts) else info
def legal_path(root, name):
    parts = PurePosixPath(name)
    if parts.is_absolute() or '..' in parts.parts or '\\' in name or ':' in name:
        raise ValueError('Unsafe source notice path')
    path = root.joinpath(*parts.parts)
    path.parent.mkdir(parents=True, exist_ok=True)
    return path
def main():
    ort = ROOT / '.native-build/onnxruntime'
    commit = subprocess.check_output(['git', '-C', str(ort), 'rev-parse', 'HEAD'], text=True).strip()
    if commit != ORT_COMMIT:
        raise ValueError('Native source identity mismatch')
    report = json.loads((ROOT / 'work/acceptance/report.json').read_text())
    log = (ROOT / 'work/acceptance/acceptance.log').read_text(encoding='utf-8')
    if 'LINKED_ORT_VERSION=1.28.3' not in log or 'OOV word skipped' in log:
        raise ValueError('Actual patched inference evidence missing')
    if not all(report['siny'][k] for k in ['lazy_start', 'unloaded', 'cancelled', 'recovered']):
        raise ValueError('Lifecycle acceptance incomplete')
    run(['python', 'scripts/prepare_release_sources.py'])
    status = json.loads((ROOT / 'work/release-sources/source-status.json').read_text())
    if not status.get('source_inventory_complete'):
        raise ValueError('Native source inventory incomplete')
    run(['python', 'scripts/license_inventory.py'])
    dist = ROOT / 'dist/native'
    dist.mkdir(parents=True, exist_ok=True)
    stage = ROOT / 'work/native-package'
    stage.mkdir(parents=True, exist_ok=True)
    source_tree = stage / 'source'
    source_tree.mkdir(exist_ok=True)
    app_tar = stage / 'application.tar'
    run(['git', 'archive', '--format=tar', '--output', str(app_tar), 'HEAD'])
    with tarfile.open(app_tar) as archive:
        archive.extractall(source_tree, filter='data')
    config = subprocess.check_output(['cargo', 'vendor', '--locked', '--versioned-dirs', str(source_tree / 'vendor')], cwd=ROOT, text=True)
    config = config.replace(str(source_tree / 'vendor').replace('\\', '/'), 'vendor').replace(str(source_tree / 'vendor'), 'vendor')
    config = portable_vendor_config(config)
    (source_tree / '.cargo').mkdir(exist_ok=True)
    with (source_tree / '.cargo/config.toml').open('a', encoding='utf-8') as stream:
        stream.write('\n' + config)
    (source_tree / 'NATIVE-SOURCE.md').write_text('''# Corresponding native source

This archive supplies application source, Cargo.lock, vendored Rust sources, original licences,
and Siny's build driver in native/ort. The sherpa ABI is 1.13.8; the ORT implementation is 1.28.3
at signed commit 0d68ff6b3b72b04aac578decd6c4c45d322bb962. See docs/native-provenance.

native-src/onnxruntime includes its source and pinned submodules. native-src/fetched includes
CMake dependency sources used for this build. native-src/archives includes the reviewed source
archives for other native components with their manifest. No SenseVoice/Kokoro model pack,
user input, captured audio or generated speech is included. Original licences are retained.

Place the supplied ORT source at .native-build/onnxruntime, then use
scripts/native-security-build.ps1. Rehydrate sherpa inputs using scripts/prepare_native.py and
replace only onnxruntime.lib with the source-built library. The native C API smoke and real
model tests are required. Build dependency downloads remain hash pinned in the source scripts.

The licenses directory inventories the Windows Cargo packages; source notices are retained
in vendor. Siny/TinySTT remain AGPL-3.0-only, TinyTTS remains GPL-3.0-or-later, and third-party
components retain their individual grants. There is no warranty.
''', encoding='utf-8')
    source_bundle = dist / 'siny-corresponding-source.tar.gz'
    with tarfile.open(source_bundle, 'w:gz', compresslevel=6) as archive:
        archive.add(source_tree, arcname='siny-source', filter=source_filter)
        archive.add(ort, arcname='siny-source/native-src/onnxruntime', filter=source_filter)
        deps = ROOT / '.native-build/build/_deps'
        if deps.exists():
            for folder in deps.iterdir():
                if folder.name.endswith('-src'):
                    archive.add(folder, arcname='siny-source/native-src/fetched/' + folder.name, filter=source_filter)
        for file in (ROOT / 'work/release-sources').iterdir():
            if file.is_file() and file.suffix != '.part' and not file.name.startswith('onnxruntime-windows-input'):
                archive.add(file, arcname='siny-source/native-src/archives/' + file.name)
        archive.add(ROOT / 'work/licenses', arcname='siny-source/licenses')
    sdk = stage / NAME
    sdk.mkdir(exist_ok=True)
    shutil.copytree(ROOT / '.native-build/secure-lib', sdk / 'lib', dirs_exist_ok=True, copy_function=os.link)
    shutil.copytree(ROOT / 'work/licenses', sdk / 'licenses', dirs_exist_ok=True)
    for name in ['LICENSE', 'LICENSE-GPL-3.0', 'THIRD_PARTY_NOTICES.md']:
        shutil.copyfile(ROOT / name, sdk / name)
    legal = sdk / 'licenses/native'
    legal.mkdir(parents=True, exist_ok=True)
    for name in ['LICENSE', 'ThirdPartyNotices.txt']:
        if not (ort / name).is_file():
            raise ValueError('Missing ORT legal notice: ' + name)
        shutil.copyfile(ort / name, legal / ('onnxruntime-' + name))
    for row in status['components']:
        if row['name'].endswith('windows-input'):
            continue
        for file in (ROOT / 'work/release-sources').glob(row['name'] + '.*'):
            if file.name.endswith('.zip'):
                with zipfile.ZipFile(file) as archive:
                    for name in row['licence_files']:
                        if name in archive.namelist() and not name.endswith('/'):
                            legal_path(legal / row['name'], name).write_bytes(archive.read(name))
            elif '.tar.' in file.name:
                with tarfile.open(file) as archive:
                    for name in row['licence_files']:
                        member = archive.getmember(name)
                        if member.isfile():
                            legal_path(legal / row['name'], name).write_bytes(archive.extractfile(member).read())
    sha = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    manifest = dict(sherpa_abi='1.13.8', ort_version='1.28.3', ort_source_commit=commit,
        application_commit=sha, source_bundle=source_bundle.name, source_sha256=digest(source_bundle),
        native_validation_run='https://github.com/styayur/siny/actions/runs/' + os.environ['GITHUB_RUN_ID'],
        libraries=[dict(name=p.name, sha256=digest(p), bytes=p.stat().st_size) for p in sorted((sdk / 'lib').glob('*.lib'))])
    (sdk / 'verification.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    (sdk / 'SOURCE.txt').write_text('Corresponding source: siny-corresponding-source.tar.gz supplied alongside this archive.\nSHA-256: ' + manifest['source_sha256'] + '\n', encoding='utf-8')
    archive_path = dist / (NAME + '.tar.bz2')
    with tarfile.open(archive_path, 'w:bz2', compresslevel=9) as archive:
        archive.add(sdk, arcname=NAME)
    manifest.update(archive=archive_path.name, archive_sha256=digest(archive_path), archive_bytes=archive_path.stat().st_size)
    (dist / 'native-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    (dist / 'SHA256SUMS.txt').write_text(''.join(digest(p) + '  ' + p.name + '\n' for p in [archive_path, source_bundle, dist / 'native-manifest.json']), encoding='utf-8')
if __name__ == '__main__':
    main()
