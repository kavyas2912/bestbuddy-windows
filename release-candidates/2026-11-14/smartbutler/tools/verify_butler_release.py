"""Verify the extracted Butler release and optionally its compiled frontend."""
from pathlib import Path
import argparse
import hashlib
import json
import re

ROOT = Path(__file__).resolve().parents[1]

def verify(branding_only=False, dist=False):
    failures=[]
    def check(condition, message):
        if not condition: failures.append(message)
    config=json.loads((ROOT/'apps/vce-desktop/src-tauri/tauri.conf.json').read_text(encoding='utf-8'))
    check(config['productName']=='Smart Butler','Wrong productName')
    check(config['identifier']=='com.vce.smartbutler.cleanr5','Wrong Butler application identity')
    main=(ROOT/'apps/vce-desktop/src/main.ts').read_text(encoding='utf-8')
    # The source HTML owns the build marker. Reject mixed source/dist files
    # without pinning this verifier to a previous release's literal marker.
    source_html=(ROOT/'apps/vce-desktop/index.html').read_text(encoding='utf-8')
    markers=set(re.findall(r'SB-\d{8}-R\d+-[A-Z0-9-]+', source_html))
    check(len(markers)==1,'Source HTML must contain exactly one Butler build marker')
    marker=next(iter(markers)) if len(markers)==1 else ''
    check(bool(marker) and marker in main,'Source HTML and app build markers do not match')
    check('butler.cleanr5.quietUntil' in main,'Fresh frontend storage namespace is missing')
    backend=(ROOT/'apps/vce-desktop/src-tauri/src/lib.rs').read_text(encoding='utf-8')
    check('.join("CleanR5")' in backend,'Fresh Windows profile directory is missing')
    check('macaw-butler' not in main,'Old macaw image reference in UI')
    check('<div class="persona-shelf"' not in main,'Old chat shortcut row found')
    for panel in ['music-panel', 'video-panel']:
        check(panel not in main, f'Removed widget still in UI: {panel}')
    assets=ROOT/'apps/vce-desktop/public/assets'
    check(not list(assets.glob('*macaw*')),'Macaw files found: extract into a NEW empty folder')
    for pose in ['welcome','listening','news','market','resting']:
        path=assets/f'butler-{pose}.png'
        check(path.is_file() and path.stat().st_size>1000,f'Missing orangutan pose: {pose}')
    builder=(ROOT/'tools/BUILD_STORE_MSIX_WINDOWS.ps1').read_text(encoding='utf-8')
    check("IdentityName = 'SmartButler.CleanPreviewR5'" in builder,'Wrong preview package identity')
    check('SmartButler_CLEAN_${msixVersion}_x64.msix' in builder,'Wrong build artifact name')
    if not branding_only:
        manifest=json.loads((ROOT/'SOURCE_SHA256.json').read_text(encoding='utf-8'))
        for name,digest in manifest.items():
            path=(ROOT/name).resolve()
            check(path.is_relative_to(ROOT),f'Invalid manifest path: {name}')
            if not path.is_relative_to(ROOT):continue
            check(path.is_file(),f'Missing packaged file: {name}')
            if path.is_file():check(hashlib.sha256(path.read_bytes()).hexdigest()==digest,f'Packaged file changed: {name}')
    if dist:
        folder=ROOT/'apps/vce-desktop/dist'
        check((folder/'index.html').is_file(),'Build the frontend first')
        if (folder/'index.html').is_file():
            compiled_markers=set(re.findall(r'SB-\d{8}-R\d+-[A-Z0-9-]+',(folder/'index.html').read_text(encoding='utf-8')))
            check(bool(marker) and compiled_markers=={marker},'Compiled HTML does not match the current source build marker; rebuild the frontend')
            scripts=list((folder/'assets').glob('*.js'))
            check(bool(scripts),'Missing compiled JavaScript')
            code='\n'.join(path.read_text(encoding='utf-8') for path in scripts)
            check(bool(marker) and marker in code,'Compiled JavaScript does not match the current source build marker; rebuild the frontend')
            check('/assets/butler-welcome.png' in code,'Compiled frontend lacks orangutan logo')
            check('macaw-butler.png' not in code,'Compiled frontend still references macaw')
            for panel in ['id="widget-music"', 'id="widget-video"', 'class="panel music-panel"', 'class="panel video-panel"']:
                check(panel not in code, f'Removed widget found in compiled frontend: {panel}')
            for pose in ['welcome','listening','news','market','resting']:
                check((folder/'assets'/f'butler-{pose}.png').is_file(),f'Compiled pose missing: {pose}')
    if failures:raise SystemExit('\n'.join(['BUTLER VERIFICATION FAILED']+failures))
    print('VERIFIED: Smart Butler / orangutan edition / '+marker+(' / compiled frontend' if dist else ''))

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--branding-only',action='store_true');parser.add_argument('--dist',action='store_true');args=parser.parse_args()
    verify(args.branding_only,args.dist)
