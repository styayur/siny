"""Load the qualified Siny SDK; never fall back to the original ORT 1.28.2 input."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile
import tomllib
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]


def sha256(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def validate_members(members, name):
    if len(members) > 30000 or sum(m.size for m in members) > 8 * 1024**3:
        raise ValueError('Unexpected SDK archive bounds')
    seen = set()
    for member in members:
        path = PurePosixPath(member.name)
        if ('\\' in member.name or ':' in member.name or path.is_absolute()
                or '..' in path.parts or not path.parts or path.parts[0] != name
                or not (member.isfile() or member.isdir()) or member.name in seen):
            raise ValueError('Unsafe SDK archive member')
        seen.add(member.name)


def prepare(archive_dir=None):
    lock = tomllib.loads((ROOT / 'Cargo.lock').read_text(encoding='utf-8'))
    native = next(p for p in lock['package'] if p['name'] == 'sherpa-onnx-sys')
    if native['version'] != '1.13.8+tinytts.zip9.1':
        raise ValueError('Review native archive identity for the new sherpa-onnx version')
    manifest = json.loads((ROOT / 'docs/native-sdk.json').read_text(encoding='utf-8'))
    if manifest['ort_version'] != '1.28.3' or manifest['sherpa_abi'] != '1.13.8':
        raise ValueError('Unreviewed native SDK identity')
    name = manifest['archive'].removesuffix('.tar.bz2')
    if manifest['archive'] != name + '.tar.bz2' or '/' in name or '\\' in name or ':' in name or name in ['', '.', '..']:
        raise ValueError('Unsafe SDK archive name')
    destination = ROOT / '.native'
    destination.mkdir(exist_ok=True)
    archive = destination / manifest['archive']
    if not archive.exists():
        partial = archive.with_suffix('.part')
        if archive_dir:
            shutil.copyfile(Path(archive_dir) / archive.name, partial)
        else:
            url = 'https://github.com/styayur/siny/releases/download/v0.1.0-alpha.1/' + archive.name
            try:
                with urllib.request.urlopen(url, timeout=60) as response, partial.open('wb') as output:
                    count = 0
                    while chunk := response.read(1024 * 1024):
                        count += len(chunk)
                        if count > manifest['archive_bytes']:
                            raise ValueError('SDK exceeds pinned size')
                        output.write(chunk)
            except urllib.error.HTTPError as error:
                if error.code != 404:
                    raise
                # First-release bootstrap only; the same hash pin covers Actions and release.
                run_id = manifest['native_validation_run'].rsplit('/', 1)[1]
                if not run_id.isdecimal():
                    raise ValueError('Invalid SDK qualification run')
                staging = destination / ('input-' + run_id)
                staging.mkdir(exist_ok=True)
                subprocess.run(['gh', 'run', 'download', run_id, '--repo', 'styayur/siny',
                                '--name', 'native-sdk-input', '--dir', str(staging)],
                               check=True, timeout=300)
                shutil.copyfile(staging / archive.name, partial)
        partial.replace(archive)
    if archive.stat().st_size != manifest['archive_bytes'] or sha256(archive) != manifest['archive_sha256']:
        raise ValueError('Native SDK identity mismatch; refusing extraction')
    with tarfile.open(archive) as source:
        members = source.getmembers()
        validate_members(members, name)
        source.extractall(destination, members=members, filter='data')
    lib_dir = (destination / name / 'lib').resolve()
    libraries = manifest['libraries']
    if {p.name for p in lib_dir.glob('*.lib')} != {row['name'] for row in libraries}:
        raise ValueError('SDK library inventory mismatch')
    for row in libraries:
        if Path(row['name']).name != row['name'] or '/' in row['name'] or '\\' in row['name']:
            raise ValueError('Unsafe SDK library name')
        path = lib_dir / row['name']
        if path.stat().st_size != row['bytes'] or sha256(path) != row['sha256']:
            raise ValueError('SDK library identity mismatch: ' + row['name'])
    return lib_dir


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--archive-dir')
    parser.add_argument('--github-env', action='store_true')
    args = parser.parse_args()
    lib_dir = prepare(args.archive_dir)
    print(lib_dir)
    if args.github_env:
        with Path(os.environ['GITHUB_ENV']).open('a', encoding='utf-8') as output:
            output.write('SHERPA_ONNX_LIB_DIR=' + str(lib_dir) + '\n')


if __name__ == '__main__':
    main()
