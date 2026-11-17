import importlib.util,tempfile,unittest
from pathlib import Path
spec=importlib.util.spec_from_file_location('trim',Path(__file__).resolve().parents[1]/'scripts/trim-office-runtime.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
class RuntimeTrimTest(unittest.TestCase):
 def test_preserves_values_comments_entities_and_unicode(self):
  source=b'<interface>\n  <!-- License -->\n  <object id="a">\n  <property name="label">  A &amp; B\n </property>\n <property name="empty"> </property>\n </object>\n</interface>'
  compact=module.compact(source);self.assertIn(b'<!-- License -->',compact);self.assertIn(b'>  A &amp; B\n </property>',compact);self.assertIn(b'> </property>',compact);self.assertLess(len(compact),len(source));self.assertEqual(module.compact(compact),compact)
  source='<interface>\n<object>\n<property>中文</property>\n</object>\n</interface>'.encode();self.assertIn('中文'.encode(),module.compact(source))
 def test_crlf_and_entity_whitespace_keep_valid_xml(self):
  source=b'<interface>\r\n  <object>\r\n <property>  value\r\n </property>\r\n </object>\r\n</interface>'
  result=module.compact(source);self.assertEqual(result,b'<interface><object><property>  value\r\n </property></object></interface>')
  self.assertEqual(module.compact(b'<interface>&#32;<object/></interface>'),b'<interface>&#32;<object/></interface>')
 def test_preserves_other_resources_and_external_symlinks(self):
  with tempfile.TemporaryDirectory() as d:
   root=Path(d)/'runtime';config=root/'office/Contents/Resources/config/soffice.cfg';ui=config/'modules/swriter/ui/dialog.ui';ui.parent.mkdir(parents=True);ui.write_text('<interface>\n <object/>\n</interface>')
   keep=[root/'fonts/font.ttf',root/'licenses/LICENSE',config/'modules/swriter/filter.xml',config/'other.ui']
   for p in keep:p.parent.mkdir(parents=True,exist_ok=True);p.write_text('keep')
   outside=Path(d)/'outside.ui';outside.write_text('outside');(ui.parent/'link.ui').symlink_to(outside)
   module.trim(root,True);self.assertTrue(ui.exists());self.assertTrue(all(p.read_text()=='keep' for p in keep));self.assertEqual(outside.read_text(),'outside');self.assertEqual(module.trim(root,True)['savedBytes'],0)
if __name__=='__main__':unittest.main()
