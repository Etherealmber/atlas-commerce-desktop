"""Keep user-facing names separate from stable installation identities."""
import importlib.util
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

    def test_icon_svg_has_no_background_tile(self):
        icon = (ROOT / "packaging/icons/atlas-commerce-desktop.svg").read_text()
        self.assertNotIn('<rect', icon)
        self.assertIn('stroke="#0A0A0A"', icon)


if __name__ == "__main__":
    unittest.main()
