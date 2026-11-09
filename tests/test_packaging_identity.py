"""Keep user-facing names separate from stable installation identities."""
import importlib.util
import struct
import zlib
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class PackagingIdentityTests(unittest.TestCase):
    def test_inno_upgrade_identity_and_display_name(self):
        script = (ROOT / "packaging/windows/atlas-commerce-desktop.iss").read_text()
        self.assertIn('AppId={{E74305A7-4960-45B6-93C9-CEA8D83B1517}', script)
        self.assertIn('#define AppName "Atlas Commerce"', script)
        self.assertIn('AppVerName={#AppName}\n', script)
        self.assertIn('DefaultDirName={localappdata}\\Programs\\Atlas Commerce Desktop', script)

    def test_msi_keeps_upgrade_identity_and_directory(self):
        spec = importlib.util.spec_from_file_location("atlas_msi", ROOT / "packaging/windows/build-msi.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        source = module.wxs(Path("payload"), "0.1.8")
        self.assertIn('<Package Name="Atlas Commerce"', source)
        self.assertIn('UpgradeCode="07D6D2FA-2A3F-42BE-8938-B2AFC2A17C7B"', source)
        self.assertIn('<Directory Id="INSTALLFOLDER" Name="Atlas Commerce Desktop"', source)
        self.assertIn('Name="Atlas Commerce"', source)

    def test_readme_wordmarks_have_transparent_background(self):
        for theme in ("light", "dark"):
            with self.subTest(theme=theme):
                png = (ROOT / f"packaging/branding/atlas-wordmark-{theme}.png").read_bytes()
                self.assertEqual(png[:8], b"\x89PNG\r\n\x1a\n")
                width, height, depth, color, *_ = struct.unpack(">IIBBBBB", png[16:29])
                self.assertGreater(width, 1)
                self.assertGreater(height, 1)
                self.assertEqual((depth, color), (8, 6), "expected 8-bit RGBA")
                chunks = []
                offset = 8
                while offset < len(png):
                    size = struct.unpack(">I", png[offset:offset + 4])[0]
                    kind = png[offset + 4:offset + 8]
                    if kind == b"IDAT":
                        chunks.append(png[offset + 8:offset + 8 + size])
                    offset += 12 + size
                first_row = zlib.decompress(b"".join(chunks))
                self.assertIn(first_row[0], range(5))
                # Every PNG filter has zero predictors for the first pixel
                # of the first row, so its alpha byte is directly readable.
                self.assertEqual(first_row[4], 0)

    def test_icon_svg_has_no_background_tile(self):
        icon = (ROOT / "packaging/icons/atlas-commerce-desktop.svg").read_text()
        self.assertNotIn('<rect', icon)
        self.assertIn('stroke="#0A0A0A"', icon)


if __name__ == "__main__":
    unittest.main()
