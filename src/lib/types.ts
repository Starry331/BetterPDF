export type Language = 'zh' | 'en';
export type Rect = [number,number,number,number];
export interface TextLine { chars?:{text:string;quad:number[]}[]; text:string; rect:Rect; size:number; font:string; color:number[]; dir?:number[] }
export interface PageData { bounds:Rect; lines:TextLine[]; widgets:Widget[]; images: {id:number; rect:Rect}[] }
export interface Widget { id:number; name:string; type:string; value:string; rect:Rect; options?:string[] }
export interface Outline { title:string; page:number; children?:Outline[] }
export interface DocumentSession { format:string; id:string; path:string; name:string; pages:number; bounds:Rect[]; outline:Outline[]; revision:number; dirty:boolean; canUndo:boolean; canRedo:boolean }
export interface Task {id:string; tool:string; state:'queued'|'running'|'completed'|'failed'|'cancelled'; progress:number; message:string; outputs:string[]; warnings:string[]}
export interface Tool {id:string; group:string; zh:string; en:string; desc:string; descEn:string; ext:string; multiple?:boolean; input?:string[]; fields?:string[]}
export interface SearchHit {page:number; quads:number[][]}
