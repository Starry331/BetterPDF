import {invoke,convertFileSrc} from '@tauri-apps/api/core';
import type { DocumentSession, PageData, SearchHit, Task } from './types';
export const native = '__TAURI_INTERNALS__' in window;
import {WorkQueue,BoundedCache} from './work-queue';
const renderQueue=new WorkQueue(2),textQueue=new WorkQueue(1);
const pageCache=new BoundedCache<PageData>(4*1024*1024,24);
const pageRequests=new Map<string,Promise<PageData>>();
async function renderPage(id:string,page:number,scale:number,signal?:AbortSignal,thumbnail=false){
 return renderQueue.run(()=>invoke<string>('render_page',{id,page,scale}).then(convertFileSrc),thumbnail?1:0,signal);
}
function pageData(id:string,page:number,revision=0,signal?:AbortSignal){
 const key=`${id}:${revision}:${page}`,cached=pageCache.get(key);if(cached)return Promise.resolve(cached);
 // Pending requests are shared only when their lifetime is not tied to a component.
 const pending=pageRequests.get(key);if(pending&&!signal)return pending;
 const request=textQueue.run(()=>invoke<PageData>('page_data',{id,page}),0,signal).then(data=>{pageCache.put(key,data,JSON.stringify(data).length*2);return data;});
 if(!signal){pageRequests.set(key,request);request.then(()=>pageRequests.delete(key),()=>pageRequests.delete(key));}return request;
}
export const api = {
 open:(path:string,password='')=>invoke<DocumentSession>('open_document',{path,password}),
 close:(id:string)=>{pageCache.clearPrefix(id+':');return invoke('close_document',{id});},
 render:renderPage,
 page:pageData,
 search:(id:string,query:string)=>invoke<SearchHit[]>('search_document',{id,query}),
 previewEdit:(id:string,edit:Record<string,unknown>,revision:number)=>invoke<string>('preview_edit',{id,edit,revision}).then(p=>convertFileSrc(p)+'?v='+Date.now()),
 edit:(id:string,edit:Record<string,unknown>,expectedRevision?:number)=>invoke<DocumentSession>('edit_document',{id,edit,expectedRevision}),
 history:(id:string,direction:string)=>invoke<DocumentSession>('document_history',{id,direction}),
 save:(id:string,output:string)=>invoke('save_document',{id,output}),
 submit:(tool:string,inputs:string[],output:string,options:Record<string,unknown>)=>invoke<string>('submit_task',{tool,inputs,output,options}),
 cancel:(id:string)=>invoke('cancel_task',{id}),
 tasks:()=>invoke<Task[]>('list_tasks'),
 capabilities:()=>invoke<Record<string,boolean>>('capabilities'),
 preview:(tool:string,inputs:string[],options:Record<string,unknown>)=>invoke<{pages:number;selected:number;message:string;image?:string;affected?:number;instances?:number}>('preview_task',{tool,inputs,options}).then(v=>({...v,image:v.image?convertFileSrc(v.image):undefined})),
 print:(id:string)=>invoke('print_document',{id})
};
