/** Short, interruptible UI motion. No timers, scroll interception or permanent layers. */
export type MotionKind = 'page' | 'document' | 'panel' | 'dialog' | 'item' | 'feedback' | 'backdrop';
export type MotionOptions = {key?:unknown;kind?:MotionKind};
const presets:Record<MotionKind,{duration:number;x:number;y:number;opacity:number}>={
 page:{duration:160,x:0,y:6,opacity:.6},document:{duration:140,x:4,y:0,opacity:.7},
 panel:{duration:150,x:8,y:0,opacity:.5},dialog:{duration:160,x:0,y:8,opacity:0},
 item:{duration:120,x:0,y:4,opacity:.5},feedback:{duration:140,x:0,y:2,opacity:.4},
 backdrop:{duration:120,x:0,y:0,opacity:0}
};
export function motionSpec(kind:MotionKind='page',reduced=false){return {...presets[kind],duration:reduced?0:presets[kind].duration};}
export function motion(node:HTMLElement,options:MotionOptions={}){
 const preference=matchMedia('(prefers-reduced-motion: reduce)');let animation:Animation|undefined;let current=options;
 const stop=()=>{animation?.cancel();animation=undefined;};
 const play=()=>{stop();if(preference.matches||!node.animate)return;const s=motionSpec(current.kind);animation=node.animate([{opacity:s.opacity,transform:`translate3d(${s.x}px,${s.y}px,0)`},{opacity:1,transform:'translate3d(0,0,0)'}],{duration:s.duration,easing:'cubic-bezier(.2,.7,.2,1)'});};
 const change=()=>{if(preference.matches)stop();};preference.addEventListener('change',change);play();
 return {update(next:MotionOptions){if(next.key===current.key&&next.kind===current.kind)return;current=next;play();},destroy(){stop();preference.removeEventListener('change',change);}};
}
/** Svelte retains a closing surface just long enough for its exit transition. */
export function surfaceTransition(_node:HTMLElement,options:{kind?:MotionKind;exit?:boolean}={}){
 const base=getComputedStyle(_node).transform;
 const transform=base==='none'?'':base+' ';
 const s=motionSpec(options.kind||'dialog',matchMedia('(prefers-reduced-motion: reduce)').matches);
 return {duration:options.exit?Math.min(110,s.duration):s.duration,easing:(t:number)=>1-Math.pow(1-t,3),css:(t:number)=>`opacity:${t};transform:${transform}translate3d(${(1-t)*s.x}px,${(1-t)*s.y}px,0)`};
}
