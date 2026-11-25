"""Source guards for the currently rendered login; not a runtime flow test."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class NativeLoginGuardTests(unittest.TestCase):
    def test_rendered_login_does_not_launch_browser_or_hosted_login(self):
        auth = (ROOT / "crates/ui/src/screens/auth.rs").read_text()
        login = auth.split("pub fn draw_login(", 1)[1].split(
            "// The earlier native login implementation", 1
        )[0]
        for forbidden in ("open::", "webbrowser", "ShellExecute", "xdg-open",
                          "browser_" + "auth::start", "webview", "start_verify_link",
                          "start_login(", "start_otp(", "start_qr_session("):
            self.assertNotIn(forbidden, login)
        self.assertIn('login_reference::draw', login)
        layout = (ROOT / "crates/ui/src/screens/login_reference.rs").read_text()
        for forbidden in ("browser_" + "auth::start", "webview", "start_verify_link", "start_login(", "start_otp(", "start_qr_session("):
            self.assertNotIn(forbidden, layout)
        self.assertIn('"Bon retour"', layout)
        self.assertIn('"Connexion avec un QR"', layout)

    def test_hosted_auth_modules_are_absent(self):
        ui = ROOT / "crates/ui/src"
        self.assertFalse((ui / ("browser_" + "auth.rs")).exists())
        self.assertFalse(list(ui.glob("webview_" + "auth*.rs")))

    def test_legacy_qr_view_does_not_open_browser(self):
        auth = (ROOT / "crates/ui/src/screens/auth.rs").read_text()
        qr = auth.split("fn draw_qr(", 1)[1].split("fn browser_approval_url(", 1)[0]
        for forbidden in ("open::", "webbrowser", "ShellExecute", "xdg-open"):
            self.assertNotIn(forbidden, qr)


if __name__ == "__main__":
    unittest.main()
