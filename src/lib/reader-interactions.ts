/** Native scrolling remains untouched; pinch gestures update at display refresh rate. */
export function pinchZoom(node:HTMLElement, change:(factor:number,x:number,y:number)=>void){
 let frame=0,factor=1,x=0,y=0,gesture=false,last=1;
 function queue(f:number,cx:number,cy:number){factor*=f;x=cx;y=cy;if(!frame)frame=requestAnimationFrame(()=>{frame=0;const value=factor;factor=1;change(value,x,y);});}
 function wheel(e:WheelEvent){if(!e.ctrlKey)return;e.preventDefault();if(gesture)return;queue(Math.exp(-Math.max(-100,Math.min(100,e.deltaY*(e.deltaMode===1?16:1)))*.01),e.clientX,e.clientY);}
 function start(e:Event){e.preventDefault();gesture=true;last=1;}
 function move(e:Event){e.preventDefault();const g=e as Event&{scale:number;clientX:number;clientY:number};if(g.scale>0&&Number.isFinite(g.scale)){queue(g.scale/last,g.clientX,g.clientY);last=g.scale;}}
 function end(e:Event){e.preventDefault();gesture=false;}
 node.addEventListener('wheel',wheel,{passive:false});node.addEventListener('gesturestart',start,{passive:false});node.addEventListener('gesturechange',move,{passive:false});node.addEventListener('gestureend',end,{passive:false});
 return {destroy(){cancelAnimationFrame(frame);node.removeEventListener('wheel',wheel);node.removeEventListener('gesturestart',start);node.removeEventListener('gesturechange',move);node.removeEventListener('gestureend',end);}};
}
export const sidebarWidth=(value:number,available:number)=>Math.max(150,Math.min(Number.isFinite(value)?value:185,420,Math.max(150,available*.45)));
