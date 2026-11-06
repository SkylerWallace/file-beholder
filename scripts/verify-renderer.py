"""Validate the single shared renderer/test snapshot selected by Cargo."""
from pathlib import Path
import argparse,runpy,tomllib,json,hashlib
ROOT=Path(__file__).resolve().parents[1]
def shared_renderer(check_inventory=False):
    data=tomllib.loads((ROOT/'Cargo.toml').read_text(encoding='utf-8'))
    kit=(ROOT/data['patch']['crates-io']['iced_tiny_skia']['path']).resolve().parent
    api=runpy.run_path(str(kit/'tools/verify.py'))
    if api['consumer_kit'](ROOT/'Cargo.toml')!=kit: raise RuntimeError('Inconsistent shared renderer paths')
    api['verify'](kit)
    if check_inventory:
        inventory=json.loads((ROOT/'docs/licenses/dependencies.json').read_text(encoding='utf-8'))
        if inventory.get('renderer_snapshot_sha256')!=hashlib.sha256((kit/'SHA256SUMS.json').read_bytes()).hexdigest():
            raise RuntimeError('Renderer license inventory is stale; regenerate scripts/generate-licenses.py')
        if inventory.get('lock_sha256')!=hashlib.sha256((ROOT/'Cargo.lock').read_bytes()).hexdigest():
            raise RuntimeError('Dependency license inventory is stale; regenerate scripts/generate-licenses.py')
    return kit
if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--root-only',action='store_true');args=parser.parse_args()
    kit=shared_renderer(check_inventory=True)
    print(kit if args.root_only else 'Verified common renderer and regression snapshot: '+str(kit))
