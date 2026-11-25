#!/usr/bin/env python3
"""Build an honest parity inventory using reference manifests and fixture PNGs.

Requires numpy and Pillow. Set PYTHONPATH if installed into a separate target.
Each manifest is the newline-separated result of the GitHub contents API.
CIE76 is calculated in D65 Lab; no element measurement is inferred from code.
"""
import argparse
from pathlib import Path
import re
import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]


def lab(path):
    rgb = np.asarray(Image.open(path).convert("RGB"), dtype=np.float32) / 255
    rgb = np.where(rgb <= 0.04045, rgb / 12.92, ((rgb + 0.055) / 1.055) ** 2.4)
    xyz = rgb @ np.array([[.4124564, .2126729, .0193339], [.3575761, .7151522, .1191920], [.1804375, .0721750, .9503041]], dtype=np.float32)
    xyz /= np.array([.95047, 1., 1.08883], dtype=np.float32)
    f = np.where(xyz > .0088564517, np.cbrt(xyz), 7.787037 * xyz + 16 / 116)
    return np.stack((116*f[..., 1]-16, 500*(f[..., 0]-f[..., 1]), 200*(f[..., 1]-f[..., 2])), axis=-1)


def diff(ref, actual):
    a, b = lab(ref), lab(actual)
    if a.shape != b.shape:
        return "Dimensions différentes", "—"
    bad = np.sqrt(np.sum((a - b) ** 2, axis=-1)) > 4
    h, w = bad.shape
    regions = [(int(bad[y:y+100, x:x+100].sum()), x, y) for y in range(0, h, 100) for x in range(0, w, 100)]
    _, x, y = max(regions)
    return f"{100 * bad.mean():.2f}%", f"({x},{y}) 100×100"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--pages-manifest', required=True)
    parser.add_argument('--shell-manifest', required=True)
    args = parser.parse_args()
    rows = []
    for state in ('default', 'error', 'otp', 'claimed', 'focus', 'checked'):
        for theme in ('light', 'dark'):
            name = f'{state}-{theme}-1280x800.png'
            ref = ROOT / 'tests/visual/login-reference' / name
            actual = ROOT / 'docs/parity/shots' / ('login-reference-' + name)
            pct, worst = diff(ref, actual)
            rows.append((f'Login {state}', theme, 'desktop-login-reference/' + name,
                         'docs/parity/shots/' + actual.name, pct, worst))
    for manifest, folder in ((args.pages_manifest, 'desktop-pages-reference'), (args.shell_manifest, 'desktop-shell-reference')):
        for filename in sorted(Path(manifest).read_text().splitlines()):
            match = re.fullmatch(r'(.+)-(light|dark)\.png', filename)
            if not match:
                continue
            rows.append((match.group(1), match.group(2), folder + '/' + filename, 'Non capturé au même état', '—', '—'))
    out = ['# Parité web — état des vérifications', '', 'Audit fonctionnel précédent : [PARITE-WEB-HISTORIQUE.md](PARITE-WEB-HISTORIQUE.md).', '',
           'Références web : branche main. Captures de connexion fictives : `f2cb86d`.',
           'Différence = pourcentage de pixels dont ΔE CIE76 > 4 (Lab D65); région = carré de 100×100 avec le plus de pixels différents.',
           'Les valeurs et éléments non mesurés restent non vérifiés ; aucune ligne ne passe sans relevé des éléments.', '',
           '| # | Écran | Thème | PNG référence | Capture desktop | Diff % | Pire région | Décalage max | Manquants | Extras | Texte | Statut |',
           '|---|---|---|---|---|---:|---|---|---|---|---|---|']
    for n, (name, theme, ref, shot, pct, worst) in enumerate(rows, 1):
        reason = 'éléments non mesurés' if pct != '—' else 'capture manquante'
        out.append(f'| {n} | {name} | {theme} | `{ref}` | `{shot}` | {pct} | {worst} | Non mesuré | Non vérifié | Non vérifié | Non vérifié | ❌ {reason} |')
    (ROOT / 'docs/PARITE-WEB.md').write_text('\n'.join(out) + '\n')
    print(f'{len(rows)} rows; 0 passing (element measurements missing)')


if __name__ == '__main__':
    main()
