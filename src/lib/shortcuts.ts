export interface KeyState {key:string;ctrlKey:boolean;metaKey:boolean;shiftKey:boolean;altKey:boolean}
export function shortcut(e:KeyState,typing=false):string|undefined {
 const key=e.key.toLowerCase(),mod=e.metaKey||e.ctrlKey;
 if(e.altKey)return;
 if(mod){
  if(typing&&['arrowup','arrowdown','pageup','pagedown'].includes(key))return;
  if(key==='tab'||key==='f6')return e.shiftKey?'previous-tab':'next-tab';
  if(key==='o')return 'open';if(key==='s'&&!e.shiftKey)return 'save';if(key==='w')return 'close-tab';
  if(e.metaKey&&e.ctrlKey&&key==='f')return 'fullscreen';if(key==='f'&&!e.shiftKey)return 'find';if(key==='p')return 'print';
  if(key==='g')return e.shiftKey?'previous-result':'next-result';
  if(!typing&&key==='z')return e.shiftKey?'redo':'undo';
  if(!typing&&key==='y')return 'redo';
  if(key==='k')return 'tools';
  if(key==='n'&&e.shiftKey)return 'go-page';
  if(key==='+'||key==='=')return 'zoom-in';if(key==='-')return 'zoom-out';
  if(key==='0')return 'fit-page';if(key==='1')return 'actual-size';if(key==='2')return 'fit';
  if(key==='arrowup'||key==='pageup')return e.shiftKey?'first':'previous';
  if(key==='arrowdown'||key==='pagedown')return e.shiftKey?'last':'next';
  if(key==='l')return e.shiftKey?'sidebar':'fullscreen';
  if(e.metaKey&&e.ctrlKey&&key==='f')return 'fullscreen';
  if(key===',')return 'settings';
 }
 if(!mod&&!typing){
  if(key==='home')return 'first';if(key==='end')return 'last';
  if(key==='arrowleft')return 'previous';if(key==='arrowright')return 'next';
  if(key==='pageup')return 'screen-up';if(key==='pagedown')return 'screen-down';
  if(key==='f3')return e.shiftKey?'previous-result':'next-result';
  if(key===' ')return e.shiftKey?'screen-up':'screen-down';
  if(key==='f11')return 'fullscreen';
  if(key==='f6')return e.shiftKey?'previous-pane':'next-pane';
  if(key==='f4')return 'sidebar';if(key==='f1')return 'shortcuts';
 }
}
