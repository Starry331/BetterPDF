<script lang="ts">
 import PdfPage from './PdfPage.svelte';import type {DocumentSession} from './types';
 let {doc,scale,mode,view,page,viewport,onselect,onvisible,onerror}:{doc:DocumentSession;scale:number;mode:string;view:string;page:number;viewport?:HTMLDivElement;onselect:(e:Record<string,unknown>)=>void;onvisible:(p:number)=>void;onerror:(s:string)=>void}=$props();
 let top=$state(0),height=$state(900);
 const layout=$derived.by(()=>{const rows:{start:number;end:number;y:number;height:number;width:number}[]=[];let y=0,width=0;const columns=view==='spread'?2:1;for(let i=0;i<doc.pages;i+=columns){const boxes=doc.bounds.slice(i,i+columns);const h=Math.max(...boxes.map(b=>(b[3]-b[1])*scale));const w=boxes.reduce((sum,b)=>sum+(b[2]-b[0])*scale,0)+(boxes.length-1)*8;rows.push({start:i,end:Math.min(doc.pages,i+columns),y,height:h,width:w});y+=h+16;width=Math.max(width,w);}return {rows,height:Math.max(0,y-16),width};});
 const visibleRows=$derived.by(()=>{if(view==='single')return [];const rows=layout.rows;let lo=0,hi=rows.length;while(lo<hi){const mid=(lo+hi)>>>1;if(rows[mid].y+rows[mid].height<Math.max(0,top-400))lo=mid+1;else hi=mid;}const result=[];for(let i=lo;i<rows.length&&rows[i].y<top+height+400;i++)result.push(rows[i]);return result;});
 $effect(()=>{const el=viewport;if(!el)return;let frame=0;const update=()=>{if(frame)return;frame=requestAnimationFrame(()=>{frame=0;top=el.scrollTop;height=el.clientHeight;});};el.addEventListener('scroll',update,{passive:true});const observer=new ResizeObserver(update);observer.observe(el);update();return()=>{el.removeEventListener('scroll',update);observer.disconnect();cancelAnimationFrame(frame);};});
 export function jump(index:number){const row=layout.rows[Math.floor(index/(view==='spread'?2:1))];if(viewport&&row)viewport.scrollTop=view==='single'?0:row.y;}
</script>
{#if view==='single'}
 <div class="reader-page-wrap" data-reader-page={page}><PdfPage {doc} {page} {scale} {mode} {onselect} {onvisible} {onerror}/></div>
{:else}
 <div class="virtual-pages" style:height="{layout.height}px" style:width="{layout.width}px">
 {#each visibleRows as row (row.start)}<div class="virtual-row" style:top="{row.y}px" style:height="{row.height}px">
 {#each Array.from({length:row.end-row.start},(_,i)=>row.start+i) as index (index)}<div data-reader-page={index}><PdfPage {doc} page={index} {scale} {mode} {onselect} {onvisible} {onerror}/></div>{/each}
 </div>{/each}
 </div>
{/if}
<style>.virtual-pages{position:relative;flex-shrink:0}.virtual-row{position:absolute;left:0;right:0;display:flex;justify-content:center;align-items:flex-start;gap:8px}</style>
