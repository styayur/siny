"""Stage the tested Windows portable program and complete application source."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
NAME = 'siny-v0.1.0-alpha.2-windows-x64'


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def command(args, **kwargs):
    return subprocess.run(args, cwd=ROOT, check=True, **kwargs)


def main():
    native = json.loads((ROOT / 'docs/native-sdk.json').read_text())
    report = json.loads((ROOT / 'work/acceptance/report.json').read_text())
    log = (ROOT / 'work/acceptance/acceptance.log').read_text(encoding='utf-8')
    if 'LINKED_ORT_VERSION=1.28.3' not in log or 'OOV word skipped' in log:
        raise ValueError('Actual secure-runtime acceptance missing')
    if not all(report['siny'][key] for key in ['lazy_start', 'unloaded', 'cancelled', 'recovered']):
        raise ValueError('Lifecycle acceptance incomplete')
    if (report['siny']['streamed_seconds'] <= 120 or not report['siny']['first_audio_before_completion']
            or not report['siny']['stream_cancelled_recovered'] or report['siny']['model_voice_count'] != 54
            or report['siny']['streamed_wav_frames'] <= 16000 or report['cli']['returncode'] != 0):
        raise ValueError('Actual migration/streaming/CLI acceptance incomplete')
    command(['python', 'scripts/license_inventory.py'])
    dist = ROOT / 'dist/windows'
    dist.mkdir(parents=True, exist_ok=True)
    stage = ROOT / 'work/windows-package'
    portable = stage / NAME
    portable.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / 'target/release/siny.exe', portable / 'siny.exe')
    for name in ['LICENSE', 'LICENSE-GPL-3.0', 'THIRD_PARTY_NOTICES.md', 'README.md']:
        shutil.copyfile(ROOT / name, portable / name)
    # Native SDK was verified before linking. Carry all original notices into the portable app.
    sdk = ROOT / '.native' / native['archive'].removesuffix('.tar.bz2')
    shutil.copytree(sdk / 'licenses', portable / 'licenses', dirs_exist_ok=True)
    shutil.copytree(ROOT / 'work/licenses', portable / 'licenses/cargo', dirs_exist_ok=True)
    shutil.copyfile(ROOT / 'docs/native-sdk.json', portable / 'native-sdk.json')
    (portable / 'config').mkdir(exist_ok=True)
    (portable / 'config/siny.example.toml').write_text('''# Copy to siny.toml or save paths in Settings. Models are installed separately.
sensevoice_dir = "models/sensevoice"
kokoro_dir = "models/kokoro"
threads = 2
language = "auto"
voice = 3
speed = 1.0
microphone = ""
hotkey = "Ctrl+Alt+D"
auto_paste = false
close_to_tray = true
retain_recording = false
idle_unload_seconds = 60
terms = []
''', encoding='utf-8')
    sha = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    (portable / 'SOURCE.txt').write_text('''Corresponding application source: siny-application-source.tar.gz
Corresponding native source: siny-corresponding-source.tar.gz
Both archives are supplied alongside this portable package in the same public release.
Native source SHA-256: ''' + native['source_sha256'] + '\nApplication commit: ' + sha + '\n', encoding='utf-8')
    metadata = dict(application_commit=sha, target='x86_64-pc-windows-msvc', crt='static /MT',
                    binary_sha256=digest(portable / 'siny.exe'), binary_bytes=(portable / 'siny.exe').stat().st_size,
                    rustc=subprocess.check_output(['rustc', '-Vv'], text=True).strip(),
                    native_sdk_sha256=native['archive_sha256'], ort_version='1.28.3',
                    validation_run='https://github.com/styayur/siny/actions/runs/' + os.environ.get('GITHUB_RUN_ID', 'local'),
                    acceptance=report, limits='Unsigned alpha. Desktop/device acceptance is supplied separately for this exact binary before publication. No human listening/WER or broad hardware-coverage claim.')
    (portable / 'verification.json').write_text(json.dumps(metadata, indent=2) + '\n', encoding='utf-8')
    with zipfile.ZipFile(dist / (NAME + '.zip'), 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(portable.rglob('*')):
            if path.is_file():
                archive.write(path, path.relative_to(stage))
    source = stage / 'source'
    source.mkdir(exist_ok=True)
    app_tar = stage / 'application.tar'
    command(['git', 'archive', '--format=tar', '--output', str(app_tar), 'HEAD'])
    with tarfile.open(app_tar) as archive:
        archive.extractall(source, filter='data')
    config = subprocess.check_output(['cargo', 'vendor', '--locked', '--versioned-dirs', str(source / 'vendor')], cwd=ROOT, text=True)
    from package_native import portable_vendor_config
    (source / '.cargo').mkdir(exist_ok=True)
    with (source / '.cargo/config.toml').open('a', encoding='utf-8') as stream:
        stream.write('\n' + portable_vendor_config(config))
    with tarfile.open(dist / 'siny-application-source.tar.gz', 'w:gz', compresslevel=6) as archive:
        archive.add(source, arcname='siny-source')
        archive.add(ROOT / 'work/licenses', arcname='siny-source/licenses')
    shutil.copyfile(portable / 'verification.json', dist / 'verification.json')
    (dist / 'SHA256SUMS.txt').write_text(''.join(digest(p) + '  ' + p.name + '\n' for p in sorted(dist.iterdir()) if p.is_file() and p.name != 'SHA256SUMS.txt'), encoding='utf-8')


if __name__ == '__main__':
    main()
