/** Small bounded scheduler. Lower priority runs first; aborted queued work never starts. */
export class WorkQueue {
 private active=0;
 private queue:{priority:number;run:()=>void;cancel:()=>void}[]=[];
 constructor(private limit=2,private capacity=256){}
 run<T>(work:()=>Promise<T>,priority=0,signal?:AbortSignal):Promise<T>{
  return new Promise((resolve,reject)=>{
   const abortError=()=>new DOMException('Superseded work','AbortError');
   if(signal?.aborted){reject(abortError());return;}
   if(this.queue.length>=this.capacity){reject(new Error('Reader queue is busy. Try again.'));return;}
   let started=false;
   const entry={priority,run:()=>{started=true;signal?.removeEventListener('abort',entry.cancel);this.active++;Promise.resolve().then(work).then(resolve,reject).finally(()=>{this.active--;this.drain();});},cancel:()=>{if(started)return;this.queue=this.queue.filter(item=>item!==entry);signal?.removeEventListener('abort',entry.cancel);reject(abortError());}};
   signal?.addEventListener('abort',entry.cancel,{once:true});this.queue.push(entry);this.queue.sort((a,b)=>a.priority-b.priority);this.drain();
  });
 }
 private drain(){while(this.active<this.limit&&this.queue.length)this.queue.shift()!.run();}
}
export class BoundedCache<T> {
 private entries=new Map<string,{value:T;bytes:number}>();private bytes=0;
 constructor(private maxBytes:number,private maxEntries:number){}
 get(key:string){const entry=this.entries.get(key);if(!entry)return;this.entries.delete(key);this.entries.set(key,entry);return entry.value;}
 put(key:string,value:T,bytes:number){this.remove(key);if(bytes>this.maxBytes)return;this.entries.set(key,{value,bytes});this.bytes+=bytes;while(this.bytes>this.maxBytes||this.entries.size>this.maxEntries)this.remove(this.entries.keys().next().value!);}
 clearPrefix(prefix:string){for(const key of this.entries.keys())if(key.startsWith(prefix))this.remove(key);}
 private remove(key:string){const old=this.entries.get(key);if(old){this.bytes-=old.bytes;this.entries.delete(key);}}
}
