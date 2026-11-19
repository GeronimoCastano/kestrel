/* Control messages arrive on a separate socket; stdin stays available to readLine. */
#define K_SESSION_RUNTIME
#include "runtime.h"
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <dlfcn.h>

typedef void (*KSessionStep)(void ***,size_t *,int,char **);
typedef void (*KSessionDrop)(void ***);
typedef void (*KSessionModulesDrop)(void);
/* A runtime stop ends one entry and keeps the session; a marked notebook session
   reports it as a cell error, a terminal session on standard error. */
static int marked;
static void report_trap(void) {
    KLocation at=k_session_trap_location;
    const char *message=k_session_trap_message?k_session_trap_message:"Runtime error";
    if(marked) printf("\x1e[error]\n%s:%zu:%zu: runtime error: %s\n  %s\n\x1e[end]\n",at.file,at.line,at.column,message,at.expression);
    else fprintf(stderr,"%s:%zu:%zu: runtime error: %s\n  %s\n",at.file,at.line,at.column,message,at.expression);
    fflush(stdout);
}
int main(int argc,char **argv) {
    if(argc<2) return 2;
    setvbuf(stdin,NULL,_IONBF,0);
    const char *marks=getenv("KESTREL_SESSION_MARKED");marked=marks && *marks && strcmp(marks,"0");
    int socket_fd=socket(AF_UNIX,SOCK_STREAM,0);
    struct sockaddr_un address={0};address.sun_family=AF_UNIX;
    if(socket_fd<0 || strlen(argv[1])>=sizeof address.sun_path) return 2;
    strcpy(address.sun_path,argv[1]);
    if(connect(socket_fd,(struct sockaddr *)&address,sizeof address)) return 2;
    FILE *input=fdopen(dup(socket_fd),"r"),*control=fdopen(socket_fd,"w");if(!input || !control) return 2;
    char *path=NULL;size_t capacity=0,count=0,libraries=0;void **globals=NULL,**handles=NULL;
    KSessionDrop drop=NULL;
    while(getline(&path,&capacity,input)>=0) {
        path[strcspn(path,"\r\n")]=0;
        void *handle=dlopen(path,RTLD_NOW|RTLD_LOCAL);
        if(!handle) {fputs("Cannot load native entry: ",control);const char *error=dlerror();for(;*error;++error) fputc(*error=='\n'||*error=='\r'?' ':*error,control);fputc('\n',control);fflush(control);continue;}
        KSessionStep step=(KSessionStep)dlsym(handle,"k_session_step");
        KSessionDrop next_drop=(KSessionDrop)dlsym(handle,"k_session_drop");
        if(!step || !next_drop) {fprintf(control,"Native entry has no session interface\n");fflush(control);dlclose(handle);continue;}
        void **next=realloc(handles,(libraries+1)*sizeof *handles);if(!next) k_fatal("Out of memory");handles=next;handles[libraries++]=handle;
        k_session_abort=0;
        int trapped=setjmp(k_session_jump);
        if(!trapped) {
            /* A runtime stop ends only this entry, in a terminal session as in a notebook. */
            k_session_trap_thread=pthread_self();k_session_trap_armed=1;
            step(&globals,&count,argc-2,argv+2);
            k_session_trap_armed=0;
            drop=next_drop;
        } else {
            /* The entry stopped partway through: keep the globals, forget its names. */
            k_session_trap_armed=0;
            report_trap();
            k_session_abort=1;
        }
        fputs(k_session_abort?"abort\n":"ok\n",control);fflush(control);
    }
    if(libraries) {
        void (*close_callbacks)(void)=(void (*)(void))dlsym(handles[libraries-1],"k_python_close_callbacks");
        if(close_callbacks) close_callbacks();
    }
    if(drop) drop(&globals);
    for(size_t i=libraries;i>0;--i) {
        KSessionModulesDrop release=(KSessionModulesDrop)dlsym(handles[i-1],"k_session_modules_drop");if(release) release();
    }
    /* Keep loaded code until process exit: Python may still own a callback's code address. */
    /* Version tables remain reachable for late Python capsule destructors. */
    free(handles);free(path);fclose(input);fclose(control);
    if(fflush(stdout)==EOF) k_fatal("Cannot write standard output");
    return 0;
}
