#!/usr/bin/env python3
"""Locked dependency inventory + redistributable notice bundle; no invented owners."""
import json, pathlib, re, subprocess, sys
host=next(line.split(': ',1)[1] for line in subprocess.check_output(['rustc','-vV'],text=True).splitlines() if line.startswith('host: '))
metadata=json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1','--filter-platform',host]))
nodes={n['id']:n for n in metadata['resolve']['nodes']}
reachable=set();pending=[metadata['resolve']['root']]
while pending:
    node=pending.pop()
    if node in reachable: continue
    reachable.add(node)
    pending.extend(dep['pkg'] for dep in nodes[node]['deps'])
allowed={'Apache-2.0','MIT','BSD-2-Clause','BSD-3-Clause','ISC','Zlib','Unicode-3.0','Unicode-DFS-2016','Unlicense','BSL-1.0','CC0-1.0','OpenSSL','MPL-2.0','CDLA-Permissive-2.0'}
problems=[]; packages=[]; notices=[]
for package in sorted(metadata['packages'],key=lambda p:(p['name'],p['version'])):
    if package['id'] not in reachable or (package['name']=='nerve' and package['source'] is None): continue
    expression=package.get('license') or ''
    alternatives=expression.split(' OR ')
    accepted=[part for part in alternatives if set(re.findall(r'[A-Za-z0-9.+-]+',part))-{'AND'} <= allowed]
    if not accepted:
        problems.append(f"Review {package['name']} {package['version']}: {expression or 'missing SPDX expression'}")
    root=pathlib.Path(package['manifest_path']).parent
    files=sorted(set(root.glob('LICENSE*'))|set(root.glob('COPYING*'))|set(root.glob('NOTICE*')))
    if package.get('license_file'): files.append(root/package['license_file'])
    # Some crates place notices one directory down.
    if not files: files=sorted(root.glob('licenses/*'))
    contents=[]
    for file in files:
        if file.is_file():
            contents.append(f"--- {file.name} ---\n"+file.read_text(errors='replace'))
    if not contents: problems.append(f"Notice text absent from registry archive: {package['name']} {package['version']}; retrieve before binary redistribution")
    packages.append(dict(name=package['name'],version=package['version'],license=expression,selected_license=accepted[0] if accepted else None,source=package['source']))
    notices.append(f"\n{'='*70}\n{package['name']} {package['version']}\nSPDX: {expression}\n"+'\n'.join(contents))
out=pathlib.Path('target/notices');out.mkdir(parents=True,exist_ok=True)
(out/'inventory.json').write_text(json.dumps(packages,indent=2)+'\n')
(out/'THIRD_PARTY_LICENSES.txt').write_text('Nerve dependency notices, generated from Cargo.lock registry archives.\n'+'\n'.join(notices))
(out/'findings.txt').write_text('\n'.join(problems)+'\n')
print(f'Inventoried {len(packages)} dependencies for {host}. Notices: target/notices/THIRD_PARTY_LICENSES.txt')
for problem in problems: print(problem,file=sys.stderr)
if problems: sys.exit(1)
