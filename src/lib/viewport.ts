/** Binary search keeps scroll work independent of the document page count. */
export function rowAt(rows:readonly {y:number;height:number}[],offset:number){
 let lo=0,hi=rows.length;while(lo<hi){const mid=(lo+hi)>>>1;if(rows[mid].y+rows[mid].height<offset)lo=mid+1;else hi=mid;}return Math.min(lo,Math.max(0,rows.length-1));
}
export function visibleRange(rows:readonly {y:number;height:number}[],top:number,height:number,overscan:number){
 if(!rows.length)return {start:0,end:0};const start=rowAt(rows,Math.max(0,top-overscan));let end=start;while(end<rows.length&&rows[end].y<=top+height+overscan)end++;return {start,end};
}
