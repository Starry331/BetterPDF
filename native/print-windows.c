/* SPDX-License-Identifier: AGPL-3.0-only
 * Native Windows print dialog and bounded per-page PDF raster spooler.
 */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <commdlg.h>
#include <mupdf/fitz.h>
#include <stdio.h>
#include <stdlib.h>
#include <math.h>

static char *utf8(const wchar_t *text) {
 int size=WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,text,-1,NULL,0,NULL,NULL);
 if(!size)return NULL;
 char *value=malloc((size_t)size);
 if(value)WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,text,-1,value,size,NULL,NULL);
 return value;
}
int wmain(int argc,wchar_t **argv) {
 if(argc!=3)return 2;
 char *path=utf8(argv[1]),password[4096]={0};
 FILE *file=_wfopen(argv[2],L"rb");
 if(!file||!path){if(file)fclose(file);free(path);return 2;}
 fread(password,1,sizeof(password)-1,file);fclose(file);
 fz_context *ctx=fz_new_context(NULL,NULL,64*1024*1024);
 if(!ctx){free(path);return 2;}
 fz_document *doc=NULL;fz_page *page=NULL;fz_pixmap *pix=NULL;
 unsigned char *dib=NULL;int result=0,started=0;
 PRINTDLGW dialog={0};dialog.lStructSize=sizeof(dialog);
 dialog.Flags=PD_RETURNDC|PD_USEDEVMODECOPIESANDCOLLATE|PD_NOSELECTION;
 dialog.hwndOwner=GetForegroundWindow();dialog.nCopies=1;dialog.nMinPage=1;dialog.nFromPage=1;
 fz_var(doc);fz_var(page);fz_var(pix);fz_var(dib);fz_var(result);fz_var(started);
 fz_try(ctx){
  fz_register_document_handlers(ctx);doc=fz_open_document(ctx,path);
  if(fz_needs_password(ctx,doc)&&!fz_authenticate_password(ctx,doc,password))fz_throw(ctx,FZ_ERROR_ARGUMENT,"PASSWORD_REQUIRED");
  int count=fz_count_pages(ctx,doc);
  if(count<1||count>20000)fz_throw(ctx,FZ_ERROR_LIMIT,"Invalid page count");
  dialog.nMaxPage=dialog.nToPage=(WORD)count;
  if(PrintDlgW(&dialog)){
   DOCINFOW info={0};info.cbSize=sizeof(info);info.lpszDocName=L"Better PDF";
   if(StartDocW(dialog.hDC,&info)<=0)fz_throw(ctx,FZ_ERROR_SYSTEM,"Printer could not start document");
   started=1;
   int first=(dialog.Flags&PD_PAGENUMS)?dialog.nFromPage-1:0;
   int last=(dialog.Flags&PD_PAGENUMS)?dialog.nToPage:count;
   for(int index=first;index<last;index++){
    page=fz_load_page(ctx,doc,index);fz_rect bounds=fz_bound_page(ctx,page);
    float width=bounds.x1-bounds.x0,height=bounds.y1-bounds.y0;
    int printable_w=GetDeviceCaps(dialog.hDC,HORZRES),printable_h=GetDeviceCaps(dialog.hDC,VERTRES);
    int dpi_x=GetDeviceCaps(dialog.hDC,LOGPIXELSX),dpi_y=GetDeviceCaps(dialog.hDC,LOGPIXELSY);
    if(width<=0||height<=0||printable_w<=0||printable_h<=0||dpi_x<=0||dpi_y<=0)fz_throw(ctx,FZ_ERROR_ARGUMENT,"Invalid page or printer dimensions");
    float fit=fminf((float)printable_w/dpi_x*72/width,(float)printable_h/dpi_y*72/height);
    float scale=fminf(300.0f/72,fit*fminf((float)dpi_x,(float)dpi_y)/72);
    double pixels=(double)width*height*scale*scale;
    if(!isfinite(pixels)||pixels<=0)fz_throw(ctx,FZ_ERROR_ARGUMENT,"Invalid print scale");
    if(pixels>40000000)scale*=sqrtf(40000000.0f/(float)pixels);
    pix=fz_new_pixmap_from_page(ctx,page,fz_scale(scale,scale),fz_device_rgb(ctx),0);
    int w=fz_pixmap_width(ctx,pix),h=fz_pixmap_height(ctx,pix),stride=fz_pixmap_stride(ctx,pix);
    size_t row=((size_t)w*3+3)&~(size_t)3;
    dib=calloc((size_t)h,row);if(!dib)fz_throw(ctx,FZ_ERROR_SYSTEM,"Not enough memory to print page");
    unsigned char *samples=fz_pixmap_samples(ctx,pix);
    for(int y=0;y<h;y++)for(int x=0;x<w;x++){
     unsigned char *a=samples+(size_t)y*stride+x*3,*b=dib+(size_t)y*row+x*3;
     b[0]=a[2];b[1]=a[1];b[2]=a[0];
    }
    BITMAPINFO bitmap={0};bitmap.bmiHeader.biSize=sizeof(BITMAPINFOHEADER);
    bitmap.bmiHeader.biWidth=w;bitmap.bmiHeader.biHeight=-h;bitmap.bmiHeader.biPlanes=1;
    bitmap.bmiHeader.biBitCount=24;bitmap.bmiHeader.biCompression=BI_RGB;
    int dest_w=(int)(width*fit*dpi_x/72),dest_h=(int)(height*fit*dpi_y/72);
    if(StartPage(dialog.hDC)<=0)fz_throw(ctx,FZ_ERROR_SYSTEM,"Printer could not start page");
    SetStretchBltMode(dialog.hDC,HALFTONE);SetBrushOrgEx(dialog.hDC,0,0,NULL);
    int written=StretchDIBits(dialog.hDC,(printable_w-dest_w)/2,(printable_h-dest_h)/2,dest_w,dest_h,0,0,w,h,dib,&bitmap,DIB_RGB_COLORS,SRCCOPY);
    if(written==GDI_ERROR||written==0||EndPage(dialog.hDC)<=0)fz_throw(ctx,FZ_ERROR_SYSTEM,"Printer could not write page");
    free(dib);dib=NULL;fz_drop_pixmap(ctx,pix);pix=NULL;fz_drop_page(ctx,page);page=NULL;
   }
   if(EndDoc(dialog.hDC)<=0)fz_throw(ctx,FZ_ERROR_SYSTEM,"Printer could not finish document");
   started=0;
  }else if(CommDlgExtendedError()!=0)fz_throw(ctx,FZ_ERROR_SYSTEM,"Could not open print dialog");
 }
 fz_always(ctx){
  if(started)AbortDoc(dialog.hDC);
  if(dialog.hDC)DeleteDC(dialog.hDC);
  if(dialog.hDevMode)GlobalFree(dialog.hDevMode);if(dialog.hDevNames)GlobalFree(dialog.hDevNames);
  free(dib);fz_drop_pixmap(ctx,pix);fz_drop_page(ctx,page);fz_drop_document(ctx,doc);
 }
 fz_catch(ctx){fprintf(stderr,"%s\n",fz_caught_message(ctx));result=1;}
 memset(password,0,sizeof(password));free(path);fz_drop_context(ctx);return result;
}
