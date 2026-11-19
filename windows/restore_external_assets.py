from pathlib import Path
import argparse,hashlib,json,os,shutil
ROOT=Path(__file__).resolve().parent
def sha(path):
 h=hashlib.sha256()
 with path.open('rb') as stream:
  for data in iter(lambda:stream.read(1024*1024),b''):h.update(data)
 return h.hexdigest()
parser=argparse.ArgumentParser(description='Restore checksum-verified external build assets; preserve existing files.')
parser.add_argument('--asset-root',type=Path)
args=parser.parse_args()
for item in json.loads((ROOT/'EXTERNAL_ASSETS.json').read_text(encoding='utf-8')):
 target=(ROOT/item['path']).resolve()
 if not target.is_relative_to(ROOT):raise RuntimeError('External asset destination escapes source')
 if target.exists():
  if sha(target)!=item['sha256']:raise RuntimeError('Existing asset differs; preserved: '+str(target))
  print('Verified '+item['path']);continue
 candidates=[Path(item['retained_local_file'])]
 if args.asset_root:candidates += [args.asset_root/item['path'],args.asset_root/Path(item['path']).name]
 canonical=next((p.resolve() for p in candidates if p.is_file() and sha(p)==item['sha256']),None)
 if canonical is None:raise RuntimeError('Missing verified asset '+item['path']+'; supply --asset-root pointing to the retained assets folder.')
 target.parent.mkdir(parents=True,exist_ok=True)
 try:os.link(canonical,target)
 except OSError:shutil.copy2(canonical,target)
 if sha(target)!=item['sha256']:raise RuntimeError('Restored asset checksum mismatch')
 print('Restored '+item['path'])
