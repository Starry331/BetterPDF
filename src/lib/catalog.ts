import type { Tool } from './types';
export const groups = [
 {id:'organize',zh:'整理页面',en:'Organize pages',color:'violet'},
 {id:'from',zh:'从 PDF 转换',en:'Convert from PDF',color:'blue'},
 {id:'to',zh:'转换为 PDF',en:'Convert to PDF',color:'orange'},
 {id:'optimize',zh:'优化文档',en:'Optimize',color:'green'},
 {id:'protect',zh:'保护与处理',en:'Protect & finish',color:'pink'},
 {id:'other',zh:'其他工具',en:'Other tools',color:'orange'}
];
const entries: [string,string,string,string,string,string,string,string[]?,boolean?,string[]?][] = [
 ['merge','organize','合并 PDF','Merge PDF','将多个文档按顺序合为一个','Combine documents in your chosen order','pdf',[],true],
 ['split','organize','拆分 PDF','Split PDF','按页或指定页数拆分文档','Split into individual pages or groups','folder',['chunk']],
 ['rotate','organize','旋转页面','Rotate pages','调整选定页面的方向','Rotate selected pages','pdf',['pages','angle']],
 ['delete','organize','删除页面','Delete pages','移除不需要的页面','Remove unwanted pages','pdf',['pages']],
 ['extract','organize','提取页面','Extract pages','将选定页面保存为新文档','Save selected pages as a new document','pdf',['pages']],
 ['reorder','organize','整理页面','Reorder pages','重新排列或重复指定页面','Rearrange or duplicate pages','pdf',['pages']],
 ['resize','organize','调整页面尺寸','Resize pages','等比缩放内容到指定纸张','Fit content to a new page size','pdf',['pages','width','height']],
 ['crop','organize','裁剪 PDF','Crop PDF','调整页面的可见区域','Adjust the visible page area','pdf',['pages','left','top','right','bottom']],
 ['to_docx','from','PDF 转 Word','PDF to Word','重建可编辑的段落与图片','Rebuild editable paragraphs and images','docx'],
 ['to_pptx','from','PDF 转 PPT','PDF to PowerPoint','保留位置，创建可编辑对象','Create editable text and image objects','pptx'],
 ['to_xlsx','from','PDF 转 Excel','PDF to Excel','识别表格并提取真实单元格','Extract tables into editable cells','xlsx'],
 ['to_images','from','PDF 转图片','PDF to images','以指定分辨率导出页面','Export pages at your chosen resolution','folder',['pages','dpi']],
 ['to_pdfa','from','PDF 转 PDF/A','PDF to PDF/A','生成 PDF/A-2b 归档文档','Create PDF/A-2b archival documents','pdf'],
 ['to_txt','from','PDF 转 TXT','PDF to text','按阅读顺序提取文字','Extract text in reading order','txt'],
 ['to_md','from','PDF 转 Markdown','PDF to Markdown','提取标题、段落与图片','Extract headings, paragraphs and images','md'],
 ['to_epub','from','PDF 转 EPUB','PDF to EPUB','生成可重排阅读的电子书','Create a reflowable ebook','epub'],
 ['word_pdf','to','Word 转 PDF','Word to PDF','离线转换 DOC 与 DOCX','Convert DOC and DOCX offline','pdf',[],true,['doc','docx']],
 ['ppt_pdf','to','PPT 转 PDF','PowerPoint to PDF','离线转换 PPT 与 PPTX','Convert PPT and PPTX offline','pdf',[],true,['ppt','pptx']],
 ['excel_pdf','to','Excel 转 PDF','Excel to PDF','保留工作表的打印布局','Preserve spreadsheet print layouts','pdf',[],true,['xls','xlsx']],
 ['images_pdf','to','图片转 PDF','Images to PDF','将图片按顺序组合成文档','Arrange images into a PDF','pdf',[],true,['png','jpg','jpeg','tif','tiff','bmp','webp']],
 ['epub_pdf','to','EPUB 转 PDF','EPUB to PDF','将电子书排版为固定页面','Lay out ebooks as fixed pages','pdf',[],true,['epub']],
 ['caj_pdf','to','CAJ 转 PDF','CAJ to PDF','转换支持的 CAJ 文献结构','Convert supported CAJ document structures','pdf',[],true,['caj','nh']],
 ['file_pdf','to','文件转 PDF','Files to PDF','Office、图片、文本与电子书','Office, images, text and ebooks','pdf',[],true,['pdf','doc','docx','ppt','pptx','xls','xlsx','png','jpg','jpeg','tif','tiff','bmp','webp','txt','md','html','htm','epub','caj']],
 ['compress','optimize','压缩 PDF','Compress PDF','减少文件体积，预览压缩结果','Reduce size with a quality preset','pdf',['quality']],
 ['repair','optimize','修复 PDF','Repair PDF','恢复可修复的文档结构','Recover repairable document structure','pdf'],
 ['ocr','optimize','识别文字 · OCR','Searchable PDF','为扫描件添加可搜索文字','Add searchable text to scanned pages','pdf',['language','dpi','autoRotate']],
 ['grayscale','optimize','PDF 转黑白','Black & white','转换为灰度或纯黑白页面','Convert to grayscale or monochrome','pdf',['colorMode','dpi']],
 ['watermark','protect','添加水印','Add watermark','为页面添加文字水印','Apply a text watermark','pdf',['pages','text','size','opacity']],
 ['remove_watermark','protect','移除水印','Remove watermark','定位并移除独立水印对象','Locate and remove independent watermarks','pdf',['pages','watermarkText']],
 ['protect','protect','保护 PDF','Protect PDF','使用 AES-256 设置打开密码','Set an opening password with AES-256','pdf',['newPassword']],
 ['unlock','protect','解锁 PDF','Unlock PDF','输入密码，保存无密码副本','Save an unlocked copy using your password','pdf'],
 ['rasterize','protect','栅格化 PDF','Rasterize PDF','将页面转换为固定图像','Render every page as a fixed image','pdf',['dpi']],
 ['flatten','protect','压平 PDF','Flatten PDF','将表单与批注外观写入页面','Bake forms and annotations into pages','pdf'],
 ['file_images','other','文件转图片','Files to images','将支持的文件逐页导出为图片','Export supported documents as page images','folder',['dpi'],true,['pdf','doc','docx','ppt','pptx','xls','xlsx','png','jpg','jpeg','tif','tiff','bmp','webp','txt','md','html','htm','epub','caj']]
];
export const tools:Tool[] = entries.map(([id,group,zh,en,desc,descEn,ext,fields,multiple,input])=>({id,group,zh,en,desc,descEn,ext,fields:fields||[],multiple,input:input||['pdf']}));
export const toolById=(id:string)=>tools.find(t=>t.id===id)!;
