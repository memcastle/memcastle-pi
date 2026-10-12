"""Generated-project metadata checks, independent of the module implementations."""

from pathlib import Path
from importlib.util import module_from_spec, spec_from_file_location
import tempfile
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[1]


class PluginManifestTests(unittest.TestCase):
    def test_module_ids_are_unique_and_stable(self):
        plugin = tomllib.loads((ROOT / "plugin.toml").read_text())
        names = [(module["type"], module["id"]) for module in plugin.get("modules", [])]
        self.assertEqual(len(names), len(set(names)))
        self.assertEqual(plugin["plugin"]["id"], "pi")
        self.assertEqual(set(names), {("source", "pi"), ("integration", "pi")})

    def test_declared_module_manifests_exist(self):
        plugin = tomllib.loads((ROOT / "plugin.toml").read_text())
        for module in plugin.get("modules", []):
            path = ROOT / module["manifest"]
            self.assertTrue(path.is_file())
            child = tomllib.loads(path.read_text())
            identity = child["source"] if module["type"] == "source" else child["integration"]
            self.assertEqual(identity["name"] if module["type"] == "source" else identity["id"], module["id"])
            self.assertEqual(identity["version"], module["version"])
            self.assertEqual(child["compatibility"]["memcastle"], plugin["memcastle"])
            if module["type"] == "source":
                self.assertEqual(child["compatibility"]["contract"], module["contract"])

    def test_integration_assets_do_not_include_the_sources_conformance_or_legacy_outputs(self):
        spec = spec_from_file_location("provider_package", ROOT / "scripts/package.py")
        packager = module_from_spec(spec)
        spec.loader.exec_module(packager)
        # Packaging must remain independent of whichever source tests ran before it.
        source = ROOT / "modules/pi/dist/source.wasm"
        legacy = ROOT / "modules/pi/dist/unreviewed-legacy.tar.gz"
        self.assertTrue(source.is_file())
        legacy.write_bytes(b"a package produced by a previous local source test")
        try:
            packager.main()
            import tarfile
            with tarfile.open(ROOT / "dist/pi-0.1.0.tar.gz", "r:gz") as archive:
                names = archive.getnames()
        finally:
            legacy.unlink()
        self.assertIn("modules/pi/dist/extension.js", names)
        self.assertIn("modules/pi/dist/package.json", names)
        self.assertNotIn("modules/pi/dist/source.wasm", names)
        self.assertNotIn("modules/pi/dist/unreviewed-legacy.tar.gz", names)

    def test_packaging_never_follows_a_source_component_symlink(self):
        spec = spec_from_file_location("provider_package", ROOT / "scripts/package.py")
        packager = module_from_spec(spec)
        spec.loader.exec_module(packager)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "modules/files-one").mkdir(parents=True)
            outside = root / "outside.txt"
            outside.write_text("untouched")
            (root / "modules/files-one/source.wasm").symlink_to(outside)
            packager.ROOT = root
            with self.assertRaisesRegex(ValueError, "symlink"):
                packager.safe_parent("modules/files-one/source.wasm")
            self.assertEqual(outside.read_text(), "untouched")
            for alias in ("modules/files-one/./source.wasm", "modules//files-one/source.wasm", "modules/../source.wasm"):
                with self.assertRaisesRegex(ValueError, "unsafe"):
                    packager.safe_parent(alias)


if __name__ == "__main__":
    unittest.main()
