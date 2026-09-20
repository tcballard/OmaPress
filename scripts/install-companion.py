#!/usr/bin/env python3
"""Register an explicitly installed browser companion for this user. Never install extensions silently."""
import argparse,json,os
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--prefix',type=Path,default=Path.home()/'.local');p.add_argument('--remove',action='store_true');a=p.parse_args()
prefix=a.prefix.resolve();assets=prefix/'share/omapress/companion'
config=Path(os.environ.get('XDG_CONFIG_HOME',str(Path.home()/'.config')))
if not config.is_absolute():raise SystemExit('XDG_CONFIG_HOME must be absolute')
for browser in ['chromium','google-chrome']:
    host=config/browser/'NativeMessagingHosts/com.pressroom.companion.json'
    if a.remove:
        if host.exists():host.unlink()
        continue
    if not (prefix/'bin/omapress').is_file():raise SystemExit('Install the OmaPress engine first')
    wrapper=prefix/'bin/omapress-companion'
    if not wrapper.is_file():raise SystemExit('Install the OmaPress companion wrapper first')
    host.parent.mkdir(parents=True,exist_ok=True)
    host.write_text(json.dumps(dict(name='com.pressroom.companion',description='OmaPress prepared article outbox',path=str(wrapper),type='stdio',allowed_origins=[(assets/'origin.txt').read_text().strip()]),indent=2)+'\n')
print('Removed companion host registration.' if a.remove else f'Host registered. Load the unpacked extension from {assets} in your browser extension manager.')
