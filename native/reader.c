/* SPDX-License-Identifier: AGPL-3.0-only
 * Small process-isolated C ABI around MuPDF rendering. No document JS is enabled.
 */
#include <mupdf/fitz.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
int bpdf_render(const char *input,const char *password,int index,float scale,const char *output,char *error,size_t error_len){
 fz_context *ctx=fz_new_context(NULL,NULL,64*1024*1024);
 if(!ctx){snprintf(error,error_len,"Cannot allocate PDF context");return 1;}
 fz_document *doc=NULL;fz_page *page=NULL;fz_pixmap *pix=NULL;int result=0;
 fz_var(doc);fz_var(page);fz_var(pix);fz_var(result);
 fz_try(ctx){
  fz_register_document_handlers(ctx);doc=fz_open_document(ctx,input);
  if(fz_needs_password(ctx,doc)&&!fz_authenticate_password(ctx,doc,password))fz_throw(ctx,FZ_ERROR_ARGUMENT,"PASSWORD_REQUIRED");
  if(index<0||index>=fz_count_pages(ctx,doc)||!isfinite(scale)||scale<.1||scale>4)fz_throw(ctx,FZ_ERROR_ARGUMENT,"Invalid page or scale");
  page=fz_load_page(ctx,doc,index);fz_rect b=fz_bound_page(ctx,page);double pixels=(b.x1-b.x0)*(b.y1-b.y0)*scale*scale;
  if(!isfinite(pixels)||pixels>40000000)fz_throw(ctx,FZ_ERROR_LIMIT,"Page exceeds render memory limit; reduce zoom");
  pix=fz_new_pixmap_from_page(ctx,page,fz_scale(scale,scale),fz_device_rgb(ctx),0);fz_save_pixmap_as_png(ctx,pix,output);
 }
 fz_always(ctx){fz_drop_pixmap(ctx,pix);fz_drop_page(ctx,page);fz_drop_document(ctx,doc);}
 fz_catch(ctx){snprintf(error,error_len,"%s",fz_caught_message(ctx));result=1;}
 fz_drop_context(ctx);return result;
}
int main(int argc,char **argv){
 if(argc!=6){fprintf(stderr,"Expected input, password-file, page, scale, output\n");return 2;}
 char password[4096]={0},error[512]={0};FILE *f=fopen(argv[2],"rb");if(!f)return 2;size_t n=fread(password,1,sizeof(password)-1,f);fclose(f);password[n]=0;
 int code=bpdf_render(argv[1],password,atoi(argv[3]),strtof(argv[4],NULL),argv[5],error,sizeof(error));memset(password,0,sizeof(password));if(code)fprintf(stderr,"%s\n",error);return code;
}
