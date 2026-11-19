"""Fail-closed native source/licence inventory. Never substitutes a different native version."""
import hashlib,json,subprocess,tarfile,zipfile,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
manifest=json.loads((ROOT/'docs/native-sources.json').read_text())
output=ROOT/'work/release-sources';output.mkdir(parents=True,exist_ok=True)
report={'components':[],'source_inventory_complete':False,'release_eligible':False,'limits':['Source inventory alone does not validate native security, real inference, devices or release assets']}
try:
    for item in sorted(manifest['components'],key=lambda c:not c.get('required_for_release',False)):
        suffix='.zip' if item['url'].endswith('.zip') else '.tar.gz' if '.tar.gz' in item['url'] else '.tar.bz2'
        path=output/(item['name']+suffix)
        if not path.exists():
            result=subprocess.run(['curl','--fail','--location','--silent','--show-error','--max-time','180','--output',str(path)+'.part',item['url']],capture_output=True,text=True,timeout=190)
            if result.returncode:raise RuntimeError(item['name']+': '+result.stderr.strip())
            Path(str(path)+'.part').replace(path)
        digest=hashlib.file_digest(path.open('rb'),'sha256').hexdigest()
        if item.get('sha256') and item['sha256']!=digest:raise RuntimeError(item['name']+': source/archive SHA mismatch')
        names=[]
        if suffix=='.zip':
            with zipfile.ZipFile(path) as archive:
                names=archive.namelist()
        else:
            with tarfile.open(path) as archive:names=archive.getnames()
        legal=[name for name in names if any(token in Path(name).name.lower() for token in ['license','copying','notice'])]
        if not legal:raise RuntimeError(item['name']+': no licence texts in archive')
        report['components'].append(dict(name=item['name'],url=item['url'],sha256=digest,licence_files=legal))
    report['source_inventory_complete']=True
finally:
    (output/'source-status.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
