<script lang="ts">
 import {motion} from './motion';
 import {rowAt,visibleRange} from './viewport';
 import PdfPage from './PdfPage.svelte';import type {DocumentSession} from './types';
 let {doc,scale,mode,view,page,viewport,onselect,onvisible,onerror}:{doc:DocumentSession;scale:number;mode:string;view:string;page:number;viewport?:HTMLDivElement;onselect:(e:Record<string,unknown>)=>void;onvisible:(p:number)=>void;onerror:(s:string)=>void}=$props();
 let range=$state({start:0,end:3});
 const layout=$derived.by(()=>{const rows:{start:number;end:number;y:number;height:number;width:number}[]=[];let y=0,width=0;const columns=view==='spread'?2:1;for(let i=0;i<doc.pages;i+=columns){const boxes=doc.bounds.slice(i,i+columns);const h=Math.max(...boxes.map(b=>(b[3]-b[1])*scale));const w=boxes.reduce((sum,b)=>sum+(b[2]-b[0])*scale,0)+(boxes.length-1)*8;rows.push({start:i,end:Math.min(doc.pages,i+columns),y,height:h,width:w});y+=h+16;width=Math.max(width,w);}return {rows,height:Math.max(0,y-16),width};});
 const visibleRows=$derived(view==='single'?[]:layout.rows.slice(range.start,range.end));
 $effect(()=>{const el=viewport;const rows=layout.rows;const single=view==='single';if(!el||single)return;let frame=0,lastPage=-1;
  const measure=()=>{frame=0;const top=Math.max(0,el.scrollTop-20),height=el.clientHeight;
   const next=visibleRange(rows,top,height,Math.min(900,height));
   if(next.start!==range.start||next.end!==range.end)range=next;
   const current=rows[rowAt(rows,top+Math.min(height*.3,180))]?.start;
   if(current!==undefined&&current!==lastPage){lastPage=current;onvisible(current);}
  };
  const update=()=>{if(!frame)frame=requestAnimationFrame(measure);};
  el.addEventListener('scroll',update,{passive:true});const observer=new ResizeObserver(update);observer.observe(el);update();
  return()=>{el.removeEventListener('scroll',update);observer.disconnect();cancelAnimationFrame(frame);};
 });
 export function jump(index:number){const row=layout.rows[Math.floor(index/(view==='spread'?2:1))];if(viewport&&row)viewport.scrollTop=view==='single'?0:row.y;}
</script>
{#if view==='single'}
 <div class="reader-page-wrap" data-reader-page={page} use:motion={{key:page,kind:'document'}}><PdfPage {doc} {page} {scale} {mode} {onselect} {onerror}/></div>
{:else}
 <div class="virtual-pages" style:height="{layout.height}px" style:width="{layout.width}px">
 {#each visibleRows as row (row.start)}<div class="virtual-row" style:top="{row.y}px" style:height="{row.height}px">
 {#each Array.from({length:row.end-row.start},(_,i)=>row.start+i) as index (index)}<div data-reader-page={index}><PdfPage {doc} page={index} {scale} {mode} {onselect} {onerror}/></div>{/each}
 </div>{/each}
 </div>
{/if}
<style>.virtual-pages{position:relative;flex-shrink:0}.virtual-row{position:absolute;left:0;right:0;display:flex;justify-content:center;align-items:flex-start;gap:8px}</style>
