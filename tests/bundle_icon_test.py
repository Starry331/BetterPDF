import plistlib
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from bundle_icon import check_app_icon

class BundleIconTests(unittest.TestCase):
    def test_missing_declaration_asset_corruption_and_external_link(self):
        source=Path(__file__).resolve().parents[1]/'src-tauri/icons/icon.icns'
        with tempfile.TemporaryDirectory() as folder:
            app=Path(folder)/'BPDF.app';resources=app/'Contents/Resources';resources.mkdir(parents=True)
            plist=app/'Contents/Info.plist';plist.write_bytes(plistlib.dumps({}))
            self.assertTrue(check_app_icon(app,source)[1])
            plist.write_bytes(plistlib.dumps({'CFBundleIconFile':'icon.icns'}))
            self.assertTrue(check_app_icon(app,source)[1])
            icon=resources/'icon.icns';shutil.copyfile(source,icon)
            self.assertEqual(check_app_icon(app,source),('icon.icns',[]))
            icon.write_bytes(b'icns\0\0\0\x08')
            self.assertTrue(check_app_icon(app,source)[1])
            icon.unlink();icon.symlink_to(source)
            self.assertTrue(check_app_icon(app,source)[1])

if __name__=='__main__': unittest.main()
