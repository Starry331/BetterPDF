<script lang="ts">
 import PdfPage from './PdfPage.svelte';import {visibleRange} from './viewport';import type {DocumentSession} from './types';
 let {doc,page,onselect}:{doc:DocumentSession;page:number;onselect:(page:number)=>void}=$props();
 let spacer:HTMLDivElement;let range=$state({start:0,end:8});
 const layout=$derived.by(()=>{let y=0;const rows=doc.bounds.map((b,index)=>{const scale=Math.min(130/(b[2]-b[0]),180/(b[3]-b[1]));const height=(b[3]-b[1])*scale+44;const row={index,scale,y,height};y+=height;return row;});return {rows,height:y};});
 const visible=$derived(layout.rows.slice(range.start,range.end));
 $effect(()=>{const rows=layout.rows;const el=spacer?.parentElement;if(!el)return;let frame=0;
  const update=()=>{if(!frame)frame=requestAnimationFrame(()=>{frame=0;const next=visibleRange(rows,Math.max(0,el.scrollTop-17),el.clientHeight,240);if(next.start!==range.start||next.end!==range.end)range=next;});};
  el.addEventListener('scroll',update,{passive:true});const observer=new ResizeObserver(update);observer.observe(el);update();
  return()=>{cancelAnimationFrame(frame);el.removeEventListener('scroll',update);observer.disconnect();};
 });
</script>
<div bind:this={spacer} class="thumbnail-spacer" style:height="{layout.height}px">
 {#each visible as row (row.index)}<div class="thumbnail-row" style:top="{row.y}px"><button class="thumbnail-button" class:selected={page===row.index} aria-current={page===row.index?'page':undefined} onclick={()=>onselect(row.index)}><PdfPage {doc} page={row.index} scale={row.scale} thumbnail/><span>{row.index+1}</span></button></div>{/each}
</div>
<style>.thumbnail-spacer{position:relative}.thumbnail-row{position:absolute;left:0;right:0;contain:layout style}.thumbnail-row :global(.thumbnail-button){content-visibility:visible;margin-bottom:0}</style>
