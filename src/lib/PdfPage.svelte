<script lang="ts">
 import {onMount} from 'svelte';
 import {api} from './api';
 import type {DocumentSession,PageData,Rect} from './types';
 let {doc,page,scale=1,mode='read',thumbnail=false,onselect,onvisible,onerror}:{doc:DocumentSession;page:number;scale?:number;mode?:string;thumbnail?:boolean;onselect?:(e:Record<string,unknown>)=>void;onvisible?:(page:number)=>void;onerror?:(s:string)=>void}=$props();
 let element:HTMLDivElement;let visible=$state(false);let image=$state('');let data=$state<PageData|null>(null);let loading=$state(false);let error=$state('');let points=$state<number[][]>([]);let drawing=false;let sequence=0;
 const box=$derived(doc.bounds[page]||[0,0,595,842]);const width=$derived((box[2]-box[0])*scale);const height=$derived((box[3]-box[1])*scale);
 onMount(()=>{const observer=new IntersectionObserver(entries=>{visible=entries[0].isIntersecting;},{rootMargin:thumbnail?'100px':'250px'});observer.observe(element);const current=new IntersectionObserver(entries=>{if(entries[0].isIntersecting&&!thumbnail)onvisible?.(page);},{threshold:.35});if(!thumbnail)current.observe(element);return()=>{observer.disconnect();current.disconnect();sequence++;};});
 $effect(()=>{const id=doc.id,rev=doc.revision,zoom=scale,show=visible;const token=++sequence;const controller=new AbortController();if(!show){image='';loading=false;return;}image='';loading=true;error='';const timer=setTimeout(()=>{api.render(id,page,Math.min(4,zoom*(thumbnail?1:window.devicePixelRatio||1)),controller.signal,thumbnail).then(src=>{if(sequence===token)image=src;}).catch(e=>{if(sequence===token&&e?.name!=='AbortError')error=String(e);}).finally(()=>{if(sequence===token)loading=false;});},70);return()=>{clearTimeout(timer);controller.abort();sequence++;};});
 $effect(()=>{const id=doc.id,rev=doc.revision,show=visible;let stale=false;const controller=new AbortController();if(!show||thumbnail){data=null;return;}data=null;api.page(id,page,rev,controller.signal).then(p=>{if(!stale)data=p;}).catch(e=>{if(!stale&&e?.name!=='AbortError')error=String(e);});return()=>{stale=true;controller.abort();};});
 function relative(e:PointerEvent):number[]{const r=element.getBoundingClientRect();return [(e.clientX-r.left)/scale+box[0],(e.clientY-r.top)/scale+box[1]];}
 function down(e:PointerEvent){if(mode==='ink'){drawing=true;points=[relative(e)];element.setPointerCapture(e.pointerId);e.preventDefault();}else if(mode==='note'){const p=relative(e);onselect?.({kind:'note',page,rect:[p[0],p[1],p[0]+24,p[1]+24],text:''});}}
 function move(e:PointerEvent){if(drawing){points=[...points,relative(e)];}}
 function up(){if(drawing){drawing=false;if(points.length>1)onselect?.({kind:'ink',page,points:$state.snapshot(points)});points=[];}}
 function lineClick(line:PageData['lines'][0]){if(mode==='text')onselect?.({kind:'text',page,rect:line.rect,targetRect:[line.rect[0],line.rect[1],Math.max(line.rect[2],box[2]-40),line.rect[3]+line.size*1.2],size:line.size,text:line.text,font:'__original__',sourceFont:line.font,color:[...line.color]});if(mode==='highlight')onselect?.({kind:'highlight',page,rect:line.rect});}
 function style(r:Rect){return `left:${(r[0]-box[0])*scale}px;top:${(r[1]-box[1])*scale}px;width:${(r[2]-r[0])*scale}px;height:${(r[3]-r[1])*scale}px;`;}
 function addText(e:MouseEvent){if(mode!=='text'||e.target!==element)return;const r=element.getBoundingClientRect(),x=(e.clientX-r.left)/scale+box[0],y=(e.clientY-r.top)/scale+box[1];onselect?.({kind:'text',insert:true,page,rect:[x,y,Math.min(x+250,box[2]),Math.min(y+80,box[3])],size:14,text:'',font:'zh-Hans'});}
</script>
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div bind:this={element} class="pdf-page" class:thumbnail class:inking={mode==='ink'||mode==='note'} style:width="{width}px" style:height="{height}px" data-page={page} onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={up} ondblclick={addText}>
 {#if image}<img class="page-image" src={image} alt="Page {page+1}" draggable="false" />{/if}
 {#if loading&&!image}<div class="page-loading"><span class="spinner"></span></div>{/if}
 {#if error}<div class="page-error">{error}</div>{/if}
 {#if data&&!thumbnail}
  <div class="text-layer" class:selecting={mode==='read'} class:editing={mode==='text'||mode==='highlight'}>
   {#each data.lines as line,i}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <span style="{style(line.rect)}font-size:{line.size*scale}px;" class="text-line" onclick={()=>lineClick(line)}>{line.text}</span>
   {/each}
  </div>
  {#if mode==='image'}{#each data.images as image}<button class="image-target" style={style(image.rect)} title="Edit image" onclick={()=>onselect?.({kind:'image',page,rect:image.rect,targetRect:image.rect})}><span>▧</span></button>{/each}{/if}
  {#if mode==='form'}{#each data.widgets as w}
   {#if w.type==='text'}<input class="pdf-field" style="{style(w.rect)}font-size:{Math.max(10,(w.rect[3]-w.rect[1])*.65*scale)}px" aria-label={w.name} value={w.value} onchange={e=>onselect?.({kind:'widget',page,widget:w.id,value:e.currentTarget.value})}/>
   {:else if w.type==='checkbox'||w.type==='radiobutton'}<input type="checkbox" class="pdf-field" style={style(w.rect)} aria-label={w.name} checked={w.value!=='Off'&&w.value!==''} onchange={e=>onselect?.({kind:'widget',page,widget:w.id,checked:e.currentTarget.checked})}/>
   {:else if w.type==='combobox'||w.type==='listbox'}<select class="pdf-field" style={style(w.rect)} aria-label={w.name} value={w.value} onchange={e=>onselect?.({kind:'widget',page,widget:w.id,value:e.currentTarget.value})}>{#each w.options||[] as o}<option>{o}</option>{/each}</select>{/if}
  {/each}{/if}
 {/if}
 {#if points.length}<svg class="ink-overlay" viewBox="{box[0]} {box[1]} {box[2]-box[0]} {box[3]-box[1]}"><polyline points={points.map(p=>p.join(',')).join(' ')} fill="none" stroke="#2659cc" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>{/if}
</div>
