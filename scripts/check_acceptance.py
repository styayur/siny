"""Run real probes; retain numerical reports, never publish model/audio inputs."""
import json,os,subprocess,time,tomllib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
output=ROOT/'work/acceptance';output.mkdir(parents=True,exist_ok=True)
reports={}
for binary in ['native_model_acceptance','acceptance']:
    exe=ROOT/'target/release/examples'/(binary+('.exe' if os.name=='nt' else ''))
    started=time.perf_counter()
    result=subprocess.run([str(exe)],cwd=ROOT,capture_output=True,text=True,encoding='utf-8',errors='replace',timeout=300)
    (output/(binary+'.log')).write_text(result.stdout+'\n'+result.stderr,encoding='utf-8')
    if result.returncode: raise RuntimeError(f'{binary} failed: see {output}')
    if 'OOV word skipped' in result.stderr:raise AssertionError('Native backend skipped input words')
    reports[binary]={'returncode':result.returncode,'wall_seconds':time.perf_counter()-started}
reports['siny']=tomllib.loads((output/'siny-report.toml').read_text(encoding='utf-8'))
reports['tinytts']=tomllib.loads((ROOT/'work/model-acceptance/native-report.toml').read_text(encoding='utf-8'))
reports['limits']=['Round-trip recognition is diagnostic, not human listening or WER benchmark','Official upstream en.wav uses a partial reference check','Physical microphone/output and desktop acceptance are separate','No model or audio redistribution']
(output/'report.json').write_text(json.dumps(reports,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps(reports['siny'],ensure_ascii=False))
