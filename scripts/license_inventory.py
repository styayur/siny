"""Inventory public Cargo source licences for the Windows workspace; reject path escapes."""
import argparse,json,shutil,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def collect(destination):
    result=subprocess.run(['cargo','metadata','--locked','--format-version','1','--filter-platform','x86_64-pc-windows-msvc'],cwd=ROOT,capture_output=True,text=True,encoding='utf-8',errors='strict',timeout=300)
    result.check_returncode();data=json.loads(result.stdout)
    destination=Path(destination);destination.mkdir(parents=True,exist_ok=True)
    inventory=[];missing=[]
    for package in data['packages']:
        source=Path(package['manifest_path']).resolve().parent
        files=[]
        for path in source.iterdir():
            if path.is_file() and any(word in path.name.lower() for word in ['license','licence','copying','notice']):files.append(path)
        for directory in source.iterdir():
            if directory.is_dir() and directory.name.lower() in ['fonts','licenses','licences']:
                files.extend(path for path in directory.rglob('*') if path.is_file() and (directory.name.lower()=='fonts' and path.suffix.lower()=='.txt' or any(word in path.name.lower() for word in ['license','licence','copying','notice','ofl','ufl'])))
        if package.get('license_file'):
            path=(source/package['license_file']).resolve()
            if not path.is_relative_to(source):raise ValueError('Licence path escapes package: '+package['name'])
            if path.is_file() and path not in files:files.append(path)
        canonical=None
        declared=package.get('license')
        if not files and package['id'] in data['workspace_members'] and declared=='AGPL-3.0-only':
            files=[ROOT/'LICENSE']
        elif not files:
            chosen={'MIT OR Apache-2.0':'Apache-2.0','MPL-2.0':'MPL-2.0','BSL-1.0':'BSL-1.0','Apache-2.0':'Apache-2.0'}.get(declared)
            if chosen:
                canonical=chosen
                files=[ROOT/'third-party/license-texts'/(chosen+'.txt')]
        if package['name']=='epaint_default_fonts':
            required=['emoji-icon-font-mit-license.txt','OFL.txt','UFL.txt','Hack-Regular.txt','egui-icons.txt']
            if not all(any(path.name==name for path in files) for name in required):
                raise RuntimeError('Incomplete egui font licence set')
            canonical='Apache-2.0'
            files.append(ROOT/'third-party/license-texts/Apache-2.0.txt')
        folder=destination/(package['name']+'-'+package['version'])
        folder.mkdir(exist_ok=True)
        for path in files:shutil.copyfile(path,folder/path.name)
        row=dict(name=package['name'],version=package['version'],declared_license=package.get('license'),repository=package.get('repository'),authors=package.get('authors',[]),files=[str(p.name) for p in files],canonical_text=canonical,notice='Original Cargo manifests and source headers are retained in corresponding source; canonical SPDX texts supplement crates that do not ship a licence file')
        inventory.append(row)
        if not files:missing.append(row['name']+' '+row['version'])
    (destination/'inventory.json').write_text(json.dumps(dict(scope='Windows workspace metadata, includes build and optional components; not all are linked into siny.exe',packages=inventory,missing_texts=missing),indent=2)+'\n',encoding='utf-8')
    if missing:raise RuntimeError('Missing licence texts: '+', '.join(missing))
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--output',default=str(ROOT/'work/licenses'));args=parser.parse_args();collect(args.output)
