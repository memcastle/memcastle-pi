"""Provider-only releases never replace the Core host used for conformance."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from latest_core_release import latest_core_tag


class LatestCoreReleaseTests(unittest.TestCase):
    def test_provider_release_does_not_replace_a_stable_core_release(self):
        releases = [
            {"tagName": "plugin/example/v1.0.0", "isDraft": False, "isPrerelease": False},
            {"tagName": "0.4.1", "isDraft": False, "isPrerelease": False},
            {"tagName": "0.4.0", "isDraft": False, "isPrerelease": False},
        ]
        self.assertEqual(latest_core_tag(releases), "0.4.1")
