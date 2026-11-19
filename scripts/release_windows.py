"""Publish only exact-main successful CI artifacts and matching qualified native sources."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]
REPO = 'styayur/siny'
TAG = 'v0.1.0-alpha.1'


def gh(*args):
    return subprocess.check_output(['gh', *args], cwd=ROOT, text=True).strip()


def sha256(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def checked_run(run_id, commit):
    run = json.loads(gh('api', f'repos/{REPO}/actions/runs/{run_id}'))
    if run['status'] != 'completed' or run['conclusion'] != 'success' or run['head_sha'] != commit:
        raise ValueError('Successful exact-commit qualification is required')
    return run


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--run', required=True)
    parser.add_argument('--tag', default=TAG)
    args = parser.parse_args()
    if args.tag != TAG or not args.run.isdecimal():
        raise ValueError('Review a new release identity explicitly')
    commit = gh('api', f'repos/{REPO}/commits/main', '--jq', '.sha')
    local = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if commit != local:
        raise ValueError('Release checkout must be current main')
    refs = json.loads(gh('api', f'repos/{REPO}/git/matching-refs/tags/{TAG}'))
    if any(ref['ref'] == 'refs/tags/' + TAG for ref in refs):
        raise ValueError('Release tag already exists; never overwrite an existing identity')
    run = checked_run(args.run, commit)
    if run['name'] != 'Siny validation' or run['event'] != 'push' or run['head_branch'] != 'main':
        raise ValueError('Main push validation required')
    native = json.loads((ROOT / 'docs/native-sdk.json').read_text())
    native_run = native['native_validation_run'].rsplit('/', 1)[1]
    checked_run(native_run, native['application_commit'])
    stage = ROOT / 'work/release' / args.run
    stage.mkdir(parents=True, exist_ok=False)
    gh('run', 'download', args.run, '--repo', REPO, '--name', 'windows-portable', '--dir', str(stage))
    sdk_stage = stage / 'native'
    gh('run', 'download', native_run, '--repo', REPO, '--name', 'native-sdk-input', '--dir', str(sdk_stage))
    source_stage = stage / 'native-source'
    gh('run', 'download', native_run, '--repo', REPO, '--name', 'native-corresponding-source', '--dir', str(source_stage))
    evidence = stage / 'native-evidence'
    gh('run', 'download', native_run, '--repo', REPO, '--name', 'native-security-evidence', '--dir', str(evidence))
    sdk = sdk_stage / native['archive']
    source = source_stage / native['source_bundle']
    if sha256(sdk) != native['archive_sha256'] or sha256(source) != native['source_sha256']:
        raise ValueError('Native binary/source identity mismatch')
    if json.loads((sdk_stage / 'native-manifest.json').read_text()) != native:
        raise ValueError('Native qualification manifest mismatch')
    verification = json.loads((stage / 'verification.json').read_text())
    if verification['application_commit'] != commit or verification['ort_version'] != '1.28.3' or verification['native_sdk_sha256'] != native['archive_sha256']:
        raise ValueError('Portable package provenance mismatch')
    portable = stage / 'siny-v0.1.0-alpha.1-windows-x64.zip'
    with zipfile.ZipFile(portable) as archive:
        prefix = 'siny-v0.1.0-alpha.1-windows-x64/'
        binary = archive.read(prefix + 'siny.exe')
        if hashlib.sha256(binary).hexdigest() != verification['binary_sha256']:
            raise ValueError('Portable binary identity mismatch')
        for notice in ['LICENSE', 'LICENSE-GPL-3.0', 'THIRD_PARTY_NOTICES.md', 'SOURCE.txt', 'licenses/inventory.json']:
            if not archive.read(prefix + notice):
                raise ValueError('Missing portable legal notice')
    # Preserve the Windows pipeline's checksum list, then build the complete release list.
    (stage / 'SHA256SUMS.txt').rename(stage / 'windows-SHA256SUMS.txt')
    for path in [sdk, source, sdk_stage / 'native-manifest.json']:
        shutil.copyfile(path, stage / path.name)
    with zipfile.ZipFile(stage / 'siny-native-build-evidence.zip', 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(evidence.rglob('*')):
            if path.is_file():
                archive.write(path, path.relative_to(evidence))
    assets = sorted(path for path in stage.iterdir() if path.is_file())
    sums = stage / 'SHA256SUMS.txt'
    sums.write_text(''.join(sha256(path) + '  ' + path.name + '\n' for path in assets), encoding='utf-8')
    assets.append(sums)
    notes = stage / 'release-notes.md'
    notes.write_text('''First integrated Windows x64 alpha: offline SenseVoice dictation and Kokoro reading, shared terminology, reverse-recognition diagnostic, bounded tasks and lazy/idle model unloading. Production models/voice packs are installed separately. No user input, captured audio or generated speech is included; upstream source archives retain their original test fixtures.

The sherpa 1.13.8 ABI is retained with source-built ONNX Runtime 1.28.3. Both application and native corresponding-source archives, original GPL/AGPL/third-party notices and SHA-256 checksums are supplied. The portable program is unsigned.

Legacy parity is partial: tray, automatic paste, debug recording export, complete named-voice UI, unlimited long-text queue and old-config migration remain deferred. TinySTT/TinyTTS remain active. Reverse similarity is diagnostic, not a human listening or WER guarantee.

Application commit: ''' + commit + '\nValidation: ' + run['html_url'] + '\nNative qualification: ' + native['native_validation_run'] + '\n', encoding='utf-8')
    gh('release', 'create', TAG, '--repo', REPO, '--target', commit, '--draft', '--prerelease', '--title', 'Siny 0.1.0 alpha 1', '--notes-file', str(notes), *[str(path) for path in assets])
    downloaded = stage / 'draft-download'
    gh('release', 'download', TAG, '--repo', REPO, '--dir', str(downloaded))
    for path in assets:
        if sha256(downloaded / path.name) != sha256(path):
            raise ValueError('Uploaded asset verification failed: ' + path.name)
    gh('release', 'edit', TAG, '--repo', REPO, '--draft=false')
    tag = json.loads(gh('api', f'repos/{REPO}/git/ref/tags/{TAG}'))
    if tag['object']['type'] != 'commit' or tag['object']['sha'] != commit:
        raise ValueError('Published tag does not identify the qualified commit')
    # Anonymous public download verifies the published boundary as well as authenticated upload.
    public = stage / 'public-download'
    public.mkdir()
    for path in assets:
        url = f'https://github.com/{REPO}/releases/download/{TAG}/{path.name}'
        subprocess.run(['curl', '--fail', '--location', '--silent', '--show-error', '--retry', '1', '--max-time', '600', '--output', str(public / path.name), url], check=True, timeout=610)
        if sha256(public / path.name) != sha256(path):
            raise ValueError('Public asset verification failed: ' + path.name)
    print(json.dumps(dict(release=f'https://github.com/{REPO}/releases/tag/{TAG}', commit=commit, assets={path.name: sha256(path) for path in assets}), indent=2))


if __name__ == '__main__':
    main()
