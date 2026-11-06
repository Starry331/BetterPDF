<script lang="ts">
 import {motion,surfaceTransition} from './motion';
 import {onMount} from 'svelte';import {invoke} from '@tauri-apps/api/core';
 import {X,Users,LoaderCircle} from 'lucide-svelte';import type {DocumentSession,Language} from './types';
 let {lang,draft,document,room,onclose,onjoined,onleave,onretry}:{lang:Language;draft:{address:string;code:string;initialized:boolean};document?:DocumentSession;room?:{role:string;invitation?:string;error?:string;lastSync?:number;syncing?:boolean};onclose:()=>void;onjoined:(v:{document:DocumentSession;role:string;invitation?:string})=>void;onleave:()=>void;onretry:()=>void}=$props();
 let busy=$state(false),error=$state('');let addressEdited=false;const t=(zh:string,en:string)=>lang==='zh'?zh:en;
 onMount(()=>{if(draft.initialized)return;draft.initialized=true;invoke<string>('lan_address').then(v=>{if(!addressEdited&&!draft.address)draft.address=v;}).catch(()=>{if(!addressEdited&&!draft.address)draft.address='127.0.0.1';});});
 async function connect(host:boolean){busy=true;error='';try{const result=await invoke<{document:DocumentSession;role:string;invitation?:string}>(host?'collaboration_host':'collaboration_join',host?{id:document?.id,address:draft.address}:{invitation:draft.code});onjoined(result);}catch(e){error=String(e);}finally{busy=false;}}
</script>
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-backdrop" use:motion={{kind:'backdrop'}} onpointerdown={e=>{if(e.button===0&&e.target===e.currentTarget&&!busy)onclose();}} onkeydown={e=>{if(e.key==='Escape'&&!busy){e.stopPropagation();onclose();}}}><div class="compact-dialog collaboration-dialog" in:surfaceTransition={{kind:'dialog'}} out:surfaceTransition={{kind:'dialog',exit:true}} role="dialog" aria-modal="true" aria-label={t('局域网协作','LAN collaboration')}><div class="field-heading"><strong><Users size={18}/> {t('局域网协作','LAN collaboration')}</strong><button class="icon-button" onclick={onclose} disabled={busy} aria-label={t('关闭','Close')}><X size={18}/></button></div>
{#if room}
 <p>{t(room.role==='host'?'你正在主持这份共享副本。将邀请码交给同一局域网中的同事。':'已加入共享文档，修改由主持端统一提交。',room.role==='host'?'Hosting a shared copy. Give the invitation to colleagues on your LAN.':'Joined a shared document. The host serializes edits.')}</p>
 {#if room.invitation}<label>{t('邀请码（包含访问凭证）','Invitation (contains access credentials)')}<textarea readonly rows="5" value={room.invitation} onfocus={e=>e.currentTarget.select()}></textarea></label><button class="text-button" onclick={async()=>{try{await navigator.clipboard.writeText(room?.invitation||'');error='';}catch{error=t('请选中邀请码后复制。','Select the invitation and copy it.');}}}>{t('复制邀请码','Copy invitation')}</button>{/if}
 {#if room.lastSync}<p class="hint">{t('最近同步','Last synced')}: {new Date(room.lastSync).toLocaleTimeString()}</p>{/if}<button class="text-button" disabled={room.syncing} onclick={onretry}>{t(room.syncing?'正在同步…':'立即同步',room.syncing?'Syncing…':'Sync now')}</button>
 {#if room.error}<p class="error-message">{room.error}</p>{/if}
 <p class="hint">{t('原文件不变。协作期间禁用历史撤销；可以另存本地副本。主持端退出后协作结束。远程图片移动和删除可同步，图片替换请在主持端操作。','The original is unchanged. History undo is disabled while sharing. Save a local copy anytime. Closing the host ends the session. Guests can move/delete images; replace images on the host.')}</p>
 <button class="secondary" onclick={onleave}>{t(room.role==='host'?'结束共享，保留本地副本':'退出协作，保留本地副本',room.role==='host'?'End sharing, keep local copy':'Leave, keep local copy')}</button>
{:else}
 <p>{t('无需云端。两台电脑使用同一局域网，通过加密邀请连接。','No cloud. Connect computers on the same LAN using an encrypted invitation.')}</p>
 <label>{t('本机局域网 IP','Local LAN IP')}<input bind:value={draft.address} oninput={()=>addressEdited=true} disabled={busy}/></label>
 <button class="primary" disabled={busy||!document||!draft.address.trim()} onclick={()=>connect(true)}>{#if busy}<LoaderCircle size={16} class="spin"/>{:else}<Users size={16}/>{/if}{t('共享当前 PDF 的副本','Share a copy of this PDF')}</button>
 {#if !document}<p class="hint">{t('先打开 PDF 即可主持；也可以直接粘贴邀请码加入。','Open a PDF to host, or paste an invitation below to join.')}</p>{/if}
 <label>{t('加入协作','Join a session')}<textarea rows="4" bind:value={draft.code} placeholder="bpdf-lan:…" disabled={busy}></textarea></label>
 <button class="secondary" disabled={busy||!draft.code.trim()} onclick={()=>connect(false)}>{t('连接并打开共享文档','Connect and open shared PDF')}</button>
 <p class="hint">{t('仅监听指定的本机地址。最多 8 个同时连接，文件上限 2 GB。大文件首次加入或版本更新需要传输文档；未改变时只检查版本。','Listens only on the selected address. Up to 8 concurrent connections; 2 GB file limit. First join and changed versions transfer the document; unchanged versions only exchange metadata.')}</p>
{/if}
{#if error}<p class="error-message">{error}</p>{/if}
</div></div>
