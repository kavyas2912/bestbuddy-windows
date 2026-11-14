"""Quick deterministic tests. No Rust/Windows compilation required."""
import importlib.util
import tempfile
from pathlib import Path
import unittest

here = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("audit_store_payload", here / "audit_store_payload.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class TestStoreAudit(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.stage = Path(self.tmp.name)
        for name in module.EXPECTED:
            target = self.stage / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(b"placeholder")
        self.home = r"C:\Users\TEST_ACCOUNT"

    def tearDown(self):
        self.tmp.cleanup()

    def entries(self):
        for p in self.stage.rglob("*"):
            if p.is_file():
                yield p.relative_to(self.stage).as_posix(), p.read_bytes()

    def test_benign(self):
        self.assertEqual(module.audit(self.entries(), self.home, False)["status"], "PASS")

    def test_external_compiler_path_warns_only(self):
        (self.stage / "smart-butler-desktop.exe").write_bytes(
            b"PE binary \x00C:\\Users\\CI_AGENT\\build\\foo.obj")
        result = module.audit(self.entries(), self.home, False)
        self.assertEqual(result["status"], "PASS")
        self.assertTrue(result["warnings"])

    def test_actual_home_remains_blocked(self):
        (self.stage / "smart-butler-desktop.exe").write_bytes(
            b"PE binary \x00C:\\Users\\TEST_ACCOUNT\\Desktop\\repo")
        with self.assertRaisesRegex(ValueError, "Current developer home"):
            module.audit(self.entries(), self.home, False)

    def test_home_utf16_remains_blocked(self):
        (self.stage / "smart-butler-desktop.exe").write_bytes(
            r"C:\Users\TEST_ACCOUNT\Desktop\repo".encode("utf-16-le"))
        with self.assertRaisesRegex(ValueError, "Current developer home"):
            module.audit(self.entries(), self.home, False)

    def test_missing_voice_model_rejected(self):
        (self.stage / "models" / "vad" / "silero_vad.onnx").unlink()
        with self.assertRaisesRegex(ValueError, "Missing essential offline"):
            module.audit(self.entries(), self.home, False)

    def test_private_file_rejected(self):
        (self.stage / "profile.json").write_text("{}")
        with self.assertRaisesRegex(ValueError, "User data"):
            module.audit(self.entries(), self.home, False)

    def test_text_path_rejected(self):
        (self.stage / "models" / "kws" / "keywords.txt").write_text(
            r"C:\Users\CI_AGENT\Desktop")
        with self.assertRaisesRegex(ValueError, "Machine-specific path"):
            module.audit(self.entries(), self.home, False)


if __name__ == "__main__":
    unittest.main()
