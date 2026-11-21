/* Control messages arrive on a separate socket; stdin stays available to readLine. */
#define K_SESSION_RUNTIME
#include "runtime.h"
#ifdef _WIN32
#include <winsock2.h>
#include <ws2tcpip.h>
#else
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <dlfcn.h>
#endif

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

#ifdef _WIN32
/* Windows has no Unix sockets in std, so the compiler listens on a loopback TCP port and
   passes it as the first argument. The worker proves itself with the token from its
   environment before the compiler sends code. */
static SOCKET control_socket=INVALID_SOCKET;
static char control_buffer[4096];
static size_t control_start,control_end;
static int control_open(const char *port) {
    WSADATA data;
    if(WSAStartup(MAKEWORD(2,2),&data)) return 0;
    const char *token=getenv("KESTREL_SESSION_TOKEN");
    if(!token || !*token) return 0;
    struct addrinfo hints={0},*found=NULL;
    hints.ai_family=AF_INET;hints.ai_socktype=SOCK_STREAM;hints.ai_protocol=IPPROTO_TCP;
    if(getaddrinfo("127.0.0.1",port,&hints,&found) || !found) return 0;
    control_socket=socket(found->ai_family,found->ai_socktype,found->ai_protocol);
    int connected=control_socket!=INVALID_SOCKET && connect(control_socket,found->ai_addr,(int)found->ai_addrlen)==0;
    freeaddrinfo(found);
    if(!connected) return 0;
    size_t length=strlen(token);
    return send(control_socket,token,(int)length,0)==(int)length && send(control_socket,"\n",1,0)==1;
}
/* One line without its newline, or NULL at the end of the stream. */
static char *control_line(char **line,size_t *capacity) {
    size_t used=0;
    for(;;) {
        if(control_start==control_end) {
            int count=recv(control_socket,control_buffer,sizeof control_buffer,0);
            if(count<=0) { if(!used) return NULL; break; }
            control_start=0;control_end=(size_t)count;
        }
        char c=control_buffer[control_start++];
        if(c=='\n') break;
        if(used+2>*capacity) {
            size_t grown=*capacity?*capacity*2:256;
            char *next=realloc(*line,grown);if(!next) k_fatal("Out of memory");
            *line=next;*capacity=grown;
        }
        (*line)[used++]=c;
    }
    if(!*line) { *line=malloc(1);if(!*line) k_fatal("Out of memory"); }
    (*line)[used]=0;
    return *line;
}
static void control_send(const char *text) {
    size_t length=strlen(text);
    while(length) {
        int sent=send(control_socket,text,(int)length,0);
        if(sent<=0) return;
        text+=sent;length-=(size_t)sent;
    }
}
static void control_close(void) { closesocket(control_socket);WSACleanup(); }
#else
static FILE *control_input,*control_output;
static int control_open(const char *path) {
    int socket_fd=socket(AF_UNIX,SOCK_STREAM,0);
    struct sockaddr_un address={0};address.sun_family=AF_UNIX;
    if(socket_fd<0 || strlen(path)>=sizeof address.sun_path) return 0;
    strcpy(address.sun_path,path);
    if(connect(socket_fd,(struct sockaddr *)&address,sizeof address)) return 0;
    control_input=fdopen(dup(socket_fd),"r");control_output=fdopen(socket_fd,"w");
    return control_input && control_output;
}
static char *control_line(char **line,size_t *capacity) {
    if(getline(line,capacity,control_input)<0) return NULL;
    (*line)[strcspn(*line,"\r\n")]=0;
    return *line;
}
static void control_send(const char *text) { fputs(text,control_output);fflush(control_output); }
static void control_close(void) { fclose(control_input);fclose(control_output); }
#endif

int main(int argc,char **argv) {
    if(argc<2) return 2;
    k_platform_start();
    setvbuf(stdin,NULL,_IONBF,0);
    const char *marks=getenv("KESTREL_SESSION_MARKED");marked=marks && *marks && strcmp(marks,"0");
    if(!control_open(argv[1])) return 2;
    char *path=NULL;size_t capacity=0,count=0,libraries=0;void **globals=NULL,**handles=NULL;
    KSessionDrop drop=NULL;
    while(control_line(&path,&capacity)) {
        void *handle=dlopen(path,RTLD_NOW|RTLD_LOCAL);
        if(!handle) {
            char message[1024];const char *error=dlerror();size_t at=0;
            for(;*error && at+2<sizeof message;++error) message[at++]=*error=='\n'||*error=='\r'?' ':*error;
            message[at++]='\n';message[at]=0;
            control_send("Cannot load native entry: ");control_send(message);continue;
        }
        KSessionStep step=(KSessionStep)dlsym(handle,"k_session_step");
        KSessionDrop next_drop=(KSessionDrop)dlsym(handle,"k_session_drop");
        if(!step || !next_drop) {control_send("Native entry has no session interface\n");dlclose(handle);continue;}
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
        control_send(k_session_abort?"abort\n":"ok\n");
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
    free(handles);free(path);control_close();
    if(fflush(stdout)==EOF) k_fatal("Cannot write standard output");
    return 0;
}
