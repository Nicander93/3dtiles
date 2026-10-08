"""Run actual processor pipelines; retain fixtures, JSONL logs and output for inspection."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import zlib

REPO = Path(__file__).resolve().parents[1]


def fingerprint(path):
    return {str(p.relative_to(path)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(path.rglob('*')) if p.is_file()}


def inspect_geometry(path):
    data = path.read_bytes()
    offset = 28 + sum(struct.unpack_from('<4I', data, 12)) if data[:4] == b'b3dm' else 0
    assert data[offset:offset + 4] == b'glTF', path
    version, length, size = struct.unpack_from('<3I', data, offset + 4)
    assert version == 2 and length <= len(data) - offset
    doc = json.loads(data[offset + 20:offset + 20 + size])
    count = 0
    for mesh in doc.get('meshes', []):
        for primitive in mesh['primitives']:
            accessor = doc['accessors'][primitive['attributes']['POSITION']]
            assert accessor['count'] > 0 and accessor['type'] == 'VEC3'
            assert all(math.isfinite(v) for v in accessor['min'] + accessor['max'])
            assert all(a <= b for a, b in zip(accessor['min'], accessor['max']))
            count += accessor['count']
    assert count > 0, f'empty geometry: {path}'
    images = []
    binary = offset + 28 + size
    for image in doc.get('images', []):
        if 'bufferView' in image:
            view = doc['bufferViews'][image['bufferView']]
            start = binary + view.get('byteOffset', 0)
            encoded = data[start:start + view['byteLength']]
        else:
            encoded = (path.parent / image['uri']).read_bytes()
        assert encoded, f'empty image: {path}'
        images.append(encoded)
    return count, images


def png():
    def chunk(kind, payload):
        return struct.pack('>I', len(payload)) + kind + payload + struct.pack('>I', zlib.crc32(kind + payload))
    scanlines = (b'\0' + bytes([255, 0, 0, 255]) * 4) * 4
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>2I5B', 4, 4, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(scanlines)) + chunk(b'IEND', b'')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--processor', default=str(REPO / 'target/debug' / ('processor.exe' if os.name == 'nt' else 'processor')))
    parser.add_argument('--converter', default=str(REPO / 'dist/runtime/converter' / ('_3dtile.exe' if os.name == 'nt' else '_3dtile')))
    parser.add_argument('--fbx', help='Optional independent FBX fixture; copied before use')
    parser.add_argument('--osgb', help='Complete OSGB dataset; copy one block plus metadata before use')
    args = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix='feature-audit-', dir=REPO / 'target'))
    env = dict(os.environ, GEOFORGE_3DTILE=str(Path(args.converter).resolve()),
               GEOFORGE_TOP_REBUILD=str(REPO / 'target/debug' / ('top_rebuild.exe' if os.name == 'nt' else 'top_rebuild')))
    results = []

    def case(name, action):
        try:
            evidence = action()
            results.append({'name': name, 'status': 'passed', 'evidence': evidence})
        except Exception as error:
            results.append({'name': name, 'status': 'failed', 'error': str(error)})
        print(f'{name}: {results[-1]["status"]}', flush=True)

    def run(name, operation, source, options, expected=0, output=None, error_text=None):
        output = output or root / name
        existing = fingerprint(output) if output.is_dir() else None
        task = root / f'{name}.json'
        task.write_text(json.dumps({'schemaVersion': 1, 'taskId': name, 'operation': operation,
                                  'input': {'path': str(source)}, 'output': {'path': str(output)}, 'options': options}), encoding='utf-8')
        before = fingerprint(source if source.is_dir() else source.parent)
        process = subprocess.run([args.processor, 'run', '--task', str(task)], env=env,
                                 capture_output=True, text=True, encoding='utf-8', timeout=90)
        (root / f'{name}.jsonl').write_text(process.stdout, encoding='utf-8')
        (root / f'{name}.stderr').write_text(process.stderr, encoding='utf-8')
        events = [json.loads(line) for line in process.stdout.splitlines() if line.strip()]
        assert process.returncode == expected, f'exit {process.returncode}, expected {expected}: {events[-3:]}'
        assert fingerprint(source if source.is_dir() else source.parent) == before, 'input changed'
        assert not (root / f'.geoforge-task-{name}').exists(), 'temporary work leaked'
        if expected:
            assert not any(event['type'] == 'result' for event in events), 'failure emitted a result'
            if existing is None:
                assert not output.exists(), 'failure committed output'
            else:
                assert fingerprint(output) == existing, 'existing output changed'
            if error_text:
                assert any(error_text.lower() in e.get('message', '').lower() for e in events if e['type'] == 'error'), 'unexpected failure reason'
            return {'exitCode': process.returncode, 'errors': [e for e in events if e['type'] == 'error']}
        assert (output / 'tileset.json').is_file() and any(e['type'] == 'result' for e in events)
        files = list(output.rglob('*.b3dm')) + list(output.rglob('*.glb'))
        assert files, 'no geometry output'
        geometry = [inspect_geometry(path) for path in files]
        return {'output': str(output), 'geometryFiles': len(files), 'positions': sum(g[0] for g in geometry), 'images': sum(len(g[1]) for g in geometry)}

    model = root / '输入 model'
    model.mkdir()
    obj = model / 'triangle.obj'
    obj.write_text('mtllib material.mtl\nv 0 0 0\nv 2 0 0\nv 0 3 0\nvt 0 0\nvt 1 0\nvt 0 1\nusemtl red\nf 1/1 2/2 3/3\n', encoding='utf-8')
    (model / 'material.mtl').write_text('newmtl red\nKd 1 1 1\nmap_Kd color.png\n', encoding='utf-8')
    texture = root / '贴图 root'
    texture.mkdir()
    (texture / 'color.png').write_bytes(png())
    basic = {'model': {'format': 'obj', 'unit': 'meters', 'axes': 'zUpRightHanded', 'missingTexturePolicy': 'error'}}
    case('obj-missing-texture-error', lambda: run('obj-missing-texture-error', 'convert-model', obj, basic, 1, error_text='texture'))
    warn = json.loads(json.dumps(basic))
    warn['model']['missingTexturePolicy'] = 'warn'
    case('obj-missing-texture-warn', lambda: run('obj-missing-texture-warn', 'convert-model', obj, warn))
    textured = json.loads(json.dumps(basic))
    textured['model']['textureRoots'] = [str(texture)]

    def textured_case():
        result = run('obj-texture-root', 'convert-model', obj, textured)
        assert result['images'] > 0, 'texture option succeeded without images'
        return result
    case('obj-texture-root', textured_case)
    case('obj-existing-output', lambda: run('obj-existing-output', 'convert-model', obj, textured, 1, root / 'obj-texture-root', 'output'))
    case('obj-overlapping-output', lambda: run('obj-overlapping-output', 'convert-model', obj, textured, 1, model, 'output'))
    case('obj-invalid-unit', lambda: run('obj-invalid-unit', 'convert-model', obj, {'model': {'format': 'obj'}}, 1, error_text='unit'))

    if args.fbx:
        fbxdir = root / 'fbx-source'
        fbxdir.mkdir()
        fbx = fbxdir / 'model.fbx'
        shutil.copyfile(args.fbx, fbx)
        case('fbx-metadata', lambda: run('fbx-metadata', 'convert-model', fbx, {'model': {'format': 'fbx'}}))
    else:
        results.append({'name': 'fbx-metadata', 'status': 'not-run', 'reason': 'Supply --fbx fixture'})

    osgb = root / 'osgb-source'
    tile = osgb / 'Data/Tile_+000_+000'
    tile.mkdir(parents=True)
    shutil.copyfile(REPO / 'tests/fixtures/osgb/test.osgb', tile / 'Tile_+000_+000.osgb')
    (osgb / 'metadata.xml').write_text('<ModelMetadata version="1"><SRS>ENU:35,117</SRS><SRSOrigin>0,0,0</SRSOrigin></ModelMetadata>', encoding='utf-8')
    case('osgb-incomplete-dependencies', lambda: run('osgb-incomplete-dependencies', 'convert-osgb', osgb, {'texture': {'mode': 'keep'}}, 1, error_text='converter returned null'))
    case('osgb-invalid-override', lambda: run('osgb-invalid-override', 'convert-osgb', osgb, {'geo': {'crs': 'EPSG:3857'}}, 1, error_text='unsupported OSGB coordinate override'))
    if args.osgb:
        source = Path(args.osgb)
        sample = root / 'osgb-complete'
        sample.mkdir()
        shutil.copyfile(source / 'metadata.xml', sample / 'metadata.xml')
        block = sorted(p for p in (source / 'Data').iterdir() if p.is_dir())[0]
        shutil.copytree(block, sample / 'Data' / block.name)
        case('osgb-native-keep', lambda: run('osgb-native-keep', 'convert-osgb', sample, {'texture': {'mode': 'keep'}}))
        def compressed():
            result = run('osgb-native-ktx2', 'convert-osgb', sample, {'texture': {'mode': 'ktx2-etc1s'}})
            images = [image for path in Path(result['output']).rglob('*.b3dm') for image in inspect_geometry(path)[1]]
            assert images and all(image.startswith(b'\xabKTX 20\xbb\r\n\x1a\n') for image in images), 'KTX2 not present in all images'
            return result
        case('osgb-native-ktx2', compressed)
    else:
        results.append({'name': 'osgb-complete-conversion', 'status': 'not-run', 'reason': 'Supply --osgb complete dataset'})

    grid = root / 'grid-source'
    shutil.copytree(REPO / 'tests/fixtures/top_rebuild/grid_4x4', grid)
    for path in (REPO / 'tests/fixtures/top_rebuild/hlod_4x4/Data').glob('Tile_*/*.b3dm'):
        shutil.copyfile(path, grid / 'Data' / path.parent.name / path.name)
    case('process-rebuild-grid', lambda: run('process-rebuild-grid', 'process-tileset', grid, {'rebuildTop': {'enabled': True, 'levels': 1, 'quality': 'balanced'}, 'texture': {'mode': 'keep'}}))
    case('process-no-operation', lambda: run('process-no-operation', 'process-tileset', grid, {'texture': {'mode': 'keep'}}, 1, error_text='requires'))

    report = {'processor': str(Path(args.processor).resolve()), 'converter': str(Path(args.converter).resolve()), 'results': results}
    path = root / 'report.json'
    path.write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding='utf-8')
    print(f'Report: {path}', flush=True)
    for result in results:
        if result['status'] == 'failed':
            print(json.dumps(result, ensure_ascii=False), flush=True)
    return int(any(r['status'] == 'failed' for r in results))


if __name__ == '__main__':
    raise SystemExit(main())
