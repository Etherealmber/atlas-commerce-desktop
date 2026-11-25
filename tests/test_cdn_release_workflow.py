"""Offline contract check for the release upload step; never sends a request."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[1]


class DesktopUploadWorkflowTests(unittest.TestCase):
    def test_uploads_all_artifacts_without_token_in_arguments(self):
        workflow = (ROOT / '.github/workflows/release.yml').read_text()
        step = workflow.split('      - name: Publish Desktop artifacts to CDN1\n', 1)[1]
        script = textwrap.dedent(step.split('        run: |\n', 1)[1].split('\n      - name: Job summary', 1)[0])
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            (work / 'dist').mkdir()
            names = [
                'atlas-commerce-desktop-v1.2.3-setup.exe',
                'atlas-commerce-desktop-v1.2.3-macos-universal.dmg',
                'atlas-commerce-desktop-1.2.3-x86_64.AppImage',
                'atlas-commerce-desktop-1.2.3-x86_64.AppImage.zsync',
                'atlas-commerce-desktop-1.2.3-aarch64.AppImage',
                'atlas-commerce-desktop-1.2.3-aarch64.AppImage.zsync',
            ]
            for name in names:
                (work / 'dist' / name).write_bytes(b'fixture bytes')
            (work / 'IN_APP_NOTE.txt').write_text('Notes fictives', encoding='utf-8')
            (work / 'bin').mkdir()
            curl = work / 'bin' / 'curl'
            curl.write_text('''#!/usr/bin/env python3
import hashlib, json, os, sys
args = sys.argv[1:]
assert os.environ['DESKTOP_RELEASE_TOKEN'] not in ' '.join(args)
config = args[args.index('--config') + 1]
assert os.environ['DESKTOP_RELEASE_TOKEN'] in open(config).read()
with open(os.environ['CALL_LOG'], 'a') as f: f.write(json.dumps(args) + '\\n')
assets = []
with open(os.environ['CALL_LOG']) as f:
    for line in f:
        call = json.loads(line)
        path = call[call.index('--data-binary') + 1][1:]
        data = open(path, 'rb').read()
        assets.append({'filename': os.path.basename(path), 'sizeBytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
with open(args[args.index('--output') + 1], 'w') as f:
    json.dump({'success': True, 'manifest': {'version': os.environ['RELEASE_VERSION'], 'assets': assets}}, f)
''')
            curl.chmod(0o755)
            env = dict(os.environ, PATH=f"{work / 'bin'}:{os.environ['PATH']}",
                       DESKTOP_RELEASE_TOKEN='fixture-token', RELEASE_TAG='v1.2.3',
                       RELEASE_VERSION='1.2.3', RELEASE_REPO='example/desktop',
                       RELEASE_MANDATORY='false', CALL_LOG=str(work / 'calls.jsonl'))
            subprocess.run(['bash', '-c', script], cwd=work, env=env, check=True)
            calls = [json.loads(line) for line in (work / 'calls.jsonl').read_text().splitlines()]
            self.assertEqual(len(calls), 6)
            for args, filename in zip(calls, names):
                self.assertIn('@dist/' + filename, args)
                self.assertIn('https://api.afmarbre.com/api/admin/desktop-release', args)
                self.assertIn('--retry-all-errors', args)
                self.assertEqual(args[args.index('--retry') + 1], '3')
                self.assertTrue(any(item.startswith('X-Release-Sha256: ') and len(item.split(': ', 1)[1]) == 64 for item in args))
            self.assertIn('X-Release-Platform: windows', calls[0])
            self.assertIn('X-Release-Platform: macos', calls[1])
            self.assertIn('X-Release-Arch: arm64', calls[-1])
            self.assertIn('X-Release-Kind: zsync', calls[-1])


if __name__ == '__main__':
    unittest.main()
