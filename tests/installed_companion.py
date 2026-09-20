#!/usr/bin/env python3
"""Exercise registration, framed IPC through the installed wrapper, and removal."""
import json,os,struct,subprocess,sys,tempfile
from pathlib import Path
prefix=Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory() as temp:
    env=dict(os.environ,XDG_CONFIG_HOME=str(Path(temp)/'config'),XDG_STATE_HOME=str(Path(temp)/'state'))
    script=prefix/'share/omapress/install-companion.py'
    subprocess.run([sys.executable,str(script),'--prefix',str(prefix)],env=env,check=True)
    for browser in ['chromium','google-chrome']:
        host=Path(env['XDG_CONFIG_HOME'])/browser/'NativeMessagingHosts/com.pressroom.companion.json'
        manifest=json.loads(host.read_text())
        request=json.dumps({'command':'list'}).encode()
        result=subprocess.run([manifest['path'],manifest['allowed_origins'][0]],input=struct.pack('=I',len(request))+request,env=env,capture_output=True,check=True,timeout=10)
        assert struct.unpack('=I',result.stdout[:4])[0]==len(result.stdout[4:])
        assert json.loads(result.stdout[4:])=={'schema':1,'ok':True,'result':{'items':[]}}
    subprocess.run([sys.executable,str(script),'--prefix',str(prefix),'--remove'],env=env,check=True)
    assert not list(Path(env['XDG_CONFIG_HOME']).glob('*/NativeMessagingHosts/com.pressroom.companion.json'))
print('Installed companion registration, native IPC and removal passed.')
