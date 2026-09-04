#!/usr/bin/env python3
import hashlib,json,pathlib,shutil,subprocess,tarfile,tomllib,os
root=pathlib.Path(__file__).resolve().parents[1]
version=tomllib.loads((root/'Cargo.toml').read_text())['workspace']['package']['version']
dest=root/'dist';dest.mkdir(exist_ok=True);bundle=dest/f'omapress-{version}-linux-x86_64'
if bundle.exists():shutil.rmtree(bundle)
(bundle/'bin').mkdir(parents=True)
for src,name in [(root/'target/release/omapress','omapress'),(root/'build/desktop/omapress-desktop','omapress-desktop')]:
    shutil.copy2(src,bundle/'bin'/name);subprocess.run(['strip',str(bundle/'bin'/name)],check=True)
size=sum(p.stat().st_size for p in (bundle/'bin').iterdir())
if size>=10_000_000:raise SystemExit(f'Executable size gate failed: {size} bytes')
shutil.copytree(root/'desktop/resources',bundle/'share/omapress')
shutil.copytree(root/'docs',bundle/'docs');shutil.copy2(root/'README.md',bundle/'README.md');shutil.copy2(root/'LICENSE',bundle/'LICENSE')
shutil.copy2(root/'scripts/install.sh',bundle/'install.sh')
(bundle/'build-info.json').write_text(json.dumps({'version':version,'commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'executable_bytes':size,'shared_dependencies':['Qt6 Core/Gui/Qml/Quick/QuickControls2','git','gh','curl']},indent=2)+'\n')
archive=dest/(bundle.name+'.tar.gz')
with tarfile.open(archive,'w:gz')as tf:tf.add(bundle,arcname=bundle.name)
(dest/'SHA256SUMS').write_text(hashlib.sha256(archive.read_bytes()).hexdigest()+'  '+archive.name+'\n')
print(json.dumps({'archive':str(archive),'executable_bytes':size}))
