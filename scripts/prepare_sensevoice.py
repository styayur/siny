"""Explicit CI/local acceptance input preparation; never called by the application."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import tarfile
import urllib.request

ROOT=Path(__file__).resolve().parents[1]
NAME='sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17'
SIZE=163002883
SHA256='7d1efa2138a65b0b488df37f8b89e3d91a60676e416f515b952358d83dfd347e'


def prepare(archive_dir=None):
    destination=ROOT/'.test-model';destination.mkdir(exist_ok=True)
    archive=destination/(NAME+'.tar.bz2')
    if not archive.exists():
        partial=archive.with_suffix('.part')
        if archive_dir:
            shutil.copyfile(Path(archive_dir)/archive.name,partial)
        else:
            url='https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/'+archive.name
            with urllib.request.urlopen(url,timeout=60) as source,partial.open('wb') as output:
                count=0
                while chunk:=source.read(1024*1024):
                    count+=len(chunk)
                    if count>SIZE:raise ValueError('Model archive exceeds pinned size')
                    output.write(chunk)
        partial.replace(archive)
    with archive.open('rb') as source: digest=hashlib.file_digest(source,'sha256').hexdigest()
    if archive.stat().st_size!=SIZE or digest!=SHA256:
        raise ValueError('Model archive identity mismatch; refusing extraction')
    with tarfile.open(archive) as source:
        members=source.getmembers()
        if len(members)>10000 or sum(m.size for m in members)>3*1024**3:raise ValueError('Unexpected model bounds')
        names=set()
        for member in members:
            path=PurePosixPath(member.name)
            if '\\' in member.name or ':' in member.name or path.is_absolute() or '..' in path.parts or not path.parts or path.parts[0]!=NAME or not(member.isfile() or member.isdir()) or member.name in names:
                raise ValueError('Unsafe model archive member')
            names.add(member.name)
        source.extractall(destination,members=members,filter='data')
    model=destination/NAME
    if not all((model/name).is_file() for name in ['model.int8.onnx','tokens.txt','test_wavs/en.wav']):raise ValueError('Missing SenseVoice files')
    (destination/'verification.json').write_text(json.dumps(dict(archive_sha256=digest,size=SIZE,redistributed=False),indent=2)+'\n',encoding='utf-8')
    return model.resolve()


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--archive-dir');parser.add_argument('--github-env',action='store_true')
    args=parser.parse_args();model=prepare(args.archive_dir)
    if args.github_env:
        with Path(os.environ['GITHUB_ENV']).open('a',encoding='utf-8') as stream:stream.write('SINY_TEST_ASR='+str(model)+'\n')


if __name__=='__main__':main()
