import hashlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

path = Path(__file__).resolve().parents[1] / 'scripts/prepare_ocr_models.py'
spec = importlib.util.spec_from_file_location('ocr_models', path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class Models(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.data = b'verified OCR bytes'
        self.spec = {'language':'eng','url':'https://example.invalid/model','bytes':len(self.data),'sha256':hashlib.sha256(self.data).hexdigest()}

    def test_verified_model_is_reused_without_network(self):
        target = self.root/'eng.traineddata'
        target.write_bytes(self.data)
        with patch.object(module.urllib.request,'urlopen',side_effect=AssertionError('Unexpected network')):
            module.ensure_model(self.root,self.spec)

    def test_corrupt_download_leaves_no_model_or_temporary_file(self):
        with patch.object(module.urllib.request,'urlopen',return_value=io.BytesIO(b'corrupt')):
            with self.assertRaises(ValueError):
                module.ensure_model(self.root,self.spec)
        self.assertEqual(list(self.root.iterdir()),[])

    def test_valid_download_commits_and_existing_corruption_is_rejected(self):
        with patch.object(module.urllib.request,'urlopen',return_value=io.BytesIO(self.data)):
            module.ensure_model(self.root,self.spec)
        target = self.root/'eng.traineddata'
        self.assertEqual(target.read_bytes(),self.data)
        target.write_bytes(b'changed')
        with self.assertRaises(ValueError):
            module.ensure_model(self.root,self.spec)
        self.assertEqual(target.read_bytes(),b'changed')

if __name__ == '__main__':
    unittest.main()
