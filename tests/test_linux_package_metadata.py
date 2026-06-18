import pathlib
import tempfile
import unittest

import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "scripts"))

from validate_linux_package_metadata import require_installed_icon


class LinuxPackageMetadataTests(unittest.TestCase):
    def test_missing_desktop_icon_records_error(self):
        with tempfile.TemporaryDirectory() as temp:
            errors = []
            paths = require_installed_icon(errors, pathlib.Path(temp), "csv-anonymizer")

        self.assertEqual(paths, [])
        self.assertEqual(
            errors,
            ["desktop Icon does not resolve to an installed hicolor/pixmaps icon: 'csv-anonymizer'"],
        )

    def test_hicolor_icon_satisfies_desktop_icon(self):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            icon = root / "usr/share/icons/hicolor/128x128/apps/csv-anonymizer.png"
            icon.parent.mkdir(parents=True)
            icon.write_bytes(b"png")
            errors = []
            paths = require_installed_icon(errors, root, "csv-anonymizer")

        self.assertEqual(errors, [])
        self.assertEqual(paths, ["/usr/share/icons/hicolor/128x128/apps/csv-anonymizer.png"])


if __name__ == "__main__":
    unittest.main()
