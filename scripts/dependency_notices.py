#!/usr/bin/env python3
"""Collect pinned Linux dependency notices from the local Cargo cache."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
EGUI = {'ecolor', 'eframe', 'egui', 'egui-winit', 'egui_extras', 'egui_glow',
        'emath', 'epaint', 'epaint_default_fonts'}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def notice_file(path):
    return (re.match(r'^(licen[cs]e|copying|notice|copyright)([._-]|$)', path.name, re.I)
            or any(part.lower() in {'licenses', 'licences', 'legal'} for part in path.parts)
            or path.as_posix() in {'fonts/Hack-Regular.txt', 'fonts/OFL.txt',
                                      'fonts/UFL.txt', 'fonts/emoji-icon-font-mit-license.txt'})


def collect(cargo, rustc, target):
    def run(*args):
        return subprocess.check_output(args, cwd=ROOT, text=True)
    metadata = json.loads(run(cargo, 'metadata', '--locked', '--offline',
                              '--format-version', '1', '--filter-platform', target))
    tree = run(cargo, 'tree', '--locked', '--offline', '--target', target,
               '--edges', 'normal,build', '--prefix', 'none', '--format', '{p}')
    active = set(re.findall(r'^(\S+) v([^ \n]+)', tree, re.M))
    provenance = json.loads((ROOT / 'licenses/upstream/SOURCES.json').read_text())
    for record in provenance:
        if digest((ROOT / 'licenses' / record['path']).read_bytes()) != record['sha256']:
            raise ValueError(f"Upstream notice checksum mismatch: {record['path']}")
    output, packages = {}, []
    for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
        name, version = package['name'], package['version']
        if not package['source'] or (name, version) not in active:
            continue
        base = Path(package['manifest_path']).parent
        prefix = f'{name}-{version}'
        files, copyrights = {}, []
        for path in sorted(base.rglob('*')):
            if not path.is_file():
                continue
            relative = path.relative_to(base)
            if notice_file(relative):
                files[relative.as_posix()] = path.read_bytes()
            elif path.suffix in {'.rs', '.c', '.h', '.cpp', '.xml', '.toml'}:
                # Preserve complete comment blocks containing source attributions.
                content = path.read_text(errors='replace')
                blocks = re.findall(r'/\*[\s\S]*?\*/|(?:(?:^[ \t]*(?://|#)[^\n]*\n)+)|<!--[\s\S]*?-->', content, re.M)
                notices = [b.strip() for b in blocks if re.search(r'copyright|SPDX-FileCopyrightText', b, re.I)]
                if notices:
                    copyrights.append(str(relative) + '\n' + '\n\n'.join(notices))
        upstream = f'egui-{version}' if name in EGUI else prefix
        extra = [r for r in provenance if Path(r['path']).parent.name == upstream]
        for record in extra:
            files['upstream/' + Path(record['path']).name] = (ROOT / 'licenses' / record['path']).read_bytes()
        if not files or not package['license']:
            raise ValueError(f'Missing license information for {prefix}')
        if copyrights:
            files['SOURCE-COPYRIGHTS.txt'] = ('\n\n'.join(copyrights) + '\n').encode()
        packages.append({'name': name, 'version': version, 'license': package['license'],
                         'authors': package['authors'], 'repository': package['repository'],
                         'source': f'https://crates.io/crates/{name}/{version}',
                         'upstream_notices': extra,
                         'files': {k: digest(v) for k, v in sorted(files.items())}})
        output.update({prefix + '/' + k: v for k, v in files.items()})
    sysroot = Path(run(rustc, '--print', 'sysroot').strip())
    library = sysroot / 'share/doc/rust/COPYRIGHT-library.html'
    output['rust/COPYRIGHT-library.html'] = library.read_bytes()
    index = {'target': target, 'cargo_lock_sha256': digest((ROOT / 'Cargo.lock').read_bytes()),
             'rustc': run(rustc, '--version', '--verbose').strip(),
             'rust_library_notice_sha256': digest(output['rust/COPYRIGHT-library.html']),
             'packages': packages}
    output['INDEX.json'] = (json.dumps(index, indent=2, ensure_ascii=False) + '\n').encode()
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cargo', default=shutil.which('cargo') or str(Path.home() / '.cargo/bin/cargo'))
    parser.add_argument('--rustc', default=shutil.which('rustc') or str(Path.home() / '.cargo/bin/rustc'))
    parser.add_argument('--target', default='x86_64-unknown-linux-gnu')
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    destination = ROOT / 'licenses/dependencies'
    try:
        expected = collect(args.cargo, args.rustc, args.target)
        actual = {p.relative_to(destination).as_posix(): p.read_bytes()
                  for p in destination.rglob('*') if p.is_file()}
        if args.check:
            changed = sorted(k for k in expected.keys() | actual.keys() if expected.get(k) != actual.get(k))
            if changed:
                parser.exit(1, f'Notices are stale ({len(changed)} files); run scripts/dependency_notices.py\n')
        else:
            for name in actual.keys() - expected.keys():
                (destination / name).unlink()
            for name, data in expected.items():
                path = destination / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
        print(f'{len(expected)} notice files verified' if args.check else f'Collected {len(expected)} notice files')
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'Cannot collect dependency notices: {error}\n')


if __name__ == '__main__':
    main()
