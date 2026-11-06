use tauri::{AppHandle, Runtime};
#[tauri::command]
pub fn update_native_menu(app:AppHandle,language:String,has_document:bool)->Result<(),String>{
 #[cfg(target_os="macos")]
 install(&app,&language,has_document).map_err(|e|e.to_string())?;
 #[cfg(not(target_os="macos"))]
 let _=(app,language,has_document);
 Ok(())
}
#[cfg(target_os="macos")]
fn install<R:Runtime>(app:&AppHandle<R>,language:&str,has_document:bool)->tauri::Result<()>{
 use tauri::menu::{MenuBuilder,SubmenuBuilder,MenuItem};
 let zh=language=="zh";let t=|a:&str,b:&str|if zh{a.to_string()}else{b.to_string()};
 let item=|id:&str,a:&str,b:&str,key:&str,enabled:bool|MenuItem::with_id(app,id,t(a,b),enabled,Some(key));
 let application=SubmenuBuilder::new(app,"Better PDF").about(None).separator()
  .item(&item("settings","设置…","Settings…","CmdOrCtrl+,",true)?).separator().services().separator().hide().hide_others().show_all().separator()
  .item(&item("quit","退出 Better PDF","Quit Better PDF","CmdOrCtrl+Q",true)?).build()?;
 let file=SubmenuBuilder::new(app,t("文件","File"))
  .item(&item("open","打开…","Open…","CmdOrCtrl+O",true)?)
  .item(&item("save","另存副本…","Save a Copy…","CmdOrCtrl+S",has_document)?)
  .item(&item("print","打印…","Print…","CmdOrCtrl+P",has_document)?).separator()
  .item(&item("close-tab","关闭文档","Close Document","CmdOrCtrl+W",has_document)?).build()?;
 let edit=SubmenuBuilder::new(app,t("编辑","Edit"))
  .item(&item("undo","撤销","Undo","CmdOrCtrl+Z",true)?)
  .item(&item("redo","重做","Redo","CmdOrCtrl+Shift+Z",true)?)
  .separator().cut().copy().paste().select_all().separator()
  .item(&item("find","在文档中查找…","Find in Document…","CmdOrCtrl+F",has_document)?)
  .item(&item("next-result","下一个查找结果","Next Search Result","CmdOrCtrl+G",has_document)?)
  .item(&item("previous-result","上一个查找结果","Previous Search Result","CmdOrCtrl+Shift+G",has_document)?).build()?;
 let view=SubmenuBuilder::new(app,t("显示","View"))
  .item(&item("zoom-in","放大","Zoom In","CmdOrCtrl+Equal",has_document)?)
  .item(&item("zoom-out","缩小","Zoom Out","CmdOrCtrl+Minus",has_document)?)
  .item(&item("fit","适合宽度","Fit Width","CmdOrCtrl+2",has_document)?)
  .item(&item("fit-page","适合页面","Fit Page","CmdOrCtrl+0",has_document)?)
  .item(&item("actual-size","实际大小","Actual Size","CmdOrCtrl+1",has_document)?)
  .text("view-continuous",t("连续阅读","Continuous" )).text("view-single",t("单页阅读","Single Page")).text("view-spread",t("双页阅读","Two Pages")).separator()
  .item(&item("sidebar","显示／隐藏目录","Toggle Sidebar","CmdOrCtrl+Shift+L",has_document)?).separator()
  .item(&item("previous","上一页","Previous Page","CmdOrCtrl+ArrowUp",has_document)?)
  .item(&item("next","下一页","Next Page","CmdOrCtrl+ArrowDown",has_document)?).item(&item("first","第一页","First Page","CmdOrCtrl+Shift+ArrowUp",has_document)?)
  .item(&item("last","最后一页","Last Page","CmdOrCtrl+Shift+ArrowDown",has_document)?)
  .item(&item("go-page","跳转页码…","Go to Page…","CmdOrCtrl+Shift+N",has_document)?).separator()
  .item(&item("fullscreen","全屏","Full Screen","CmdOrCtrl+L",true)?).build()?;
 let tools=SubmenuBuilder::new(app,t("工具","Tools"))
  .text("mode-text",t("编辑 PDF 文字","Edit PDF Text")).text("mode-image",t("编辑 PDF 图片","Edit PDF Image")).text("mode-highlight",t("高亮","Highlight")).text("mode-note",t("批注","Note")).text("mode-form",t("填写表单","Fill Forms")).separator()
  .item(&item("tools","全部工具…","All Tools…","CmdOrCtrl+K",true)?)
  .item(&item("tasks","任务","Tasks","CmdOrCtrl+Shift+J",true)?)
  .item(&item("share","局域网协作…","LAN Collaboration…","CmdOrCtrl+Shift+S",true)?).build()?;
 let window=SubmenuBuilder::new(app,t("窗口","Window")).minimize().separator()
  .item(&item("next-tab","下一个文档","Next Document","CmdOrCtrl+F6",has_document)?)
  .item(&item("previous-tab","上一个文档","Previous Document","CmdOrCtrl+Shift+F6",has_document)?).build()?;
 let help=SubmenuBuilder::new(app,t("帮助","Help")).text("shortcuts",t("快捷键","Keyboard Shortcuts")).separator().text("github",t("GitHub 项目","GitHub Project")).build()?;
 app.set_menu(MenuBuilder::new(app).items(&[&application,&file,&edit,&view,&tools,&window,&help]).build()?)?;
 Ok(())
}
