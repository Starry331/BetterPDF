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
  .item(&item("find","在文档中查找…","Find in Document…","CmdOrCtrl+F",has_document)?).build()?;
 let view=SubmenuBuilder::new(app,t("显示","View"))
  .item(&item("zoom-in","放大","Zoom In","CmdOrCtrl+Equal",has_document)?)
  .item(&item("zoom-out","缩小","Zoom Out","CmdOrCtrl+Minus",has_document)?)
  .item(&item("fit","适合宽度","Fit Width","CmdOrCtrl+0",has_document)?)
  .item(&item("sidebar","显示／隐藏目录","Toggle Sidebar","CmdOrCtrl+Shift+L",has_document)?).separator()
  .item(&item("previous","上一页","Previous Page","CmdOrCtrl+ArrowUp",has_document)?)
  .item(&item("next","下一页","Next Page","CmdOrCtrl+ArrowDown",has_document)?).separator().fullscreen().build()?;
 let tools=SubmenuBuilder::new(app,t("工具","Tools"))
  .item(&item("tools","全部工具…","All Tools…","CmdOrCtrl+K",true)?)
  .item(&item("tasks","任务","Tasks","CmdOrCtrl+Shift+J",true)?)
  .item(&item("share","局域网协作…","LAN Collaboration…","CmdOrCtrl+Shift+S",true)?).build()?;
 let window=SubmenuBuilder::new(app,t("窗口","Window")).minimize().build()?;
 let help=SubmenuBuilder::new(app,t("帮助","Help")).text("github",t("GitHub 项目","GitHub Project")).build()?;
 app.set_menu(MenuBuilder::new(app).items(&[&application,&file,&edit,&view,&tools,&window,&help]).build()?)?;
 Ok(())
}
