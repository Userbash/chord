#define _GNU_SOURCE
#include <dlfcn.h>
#include <execinfo.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

/* External diagnostic interposer only. It neither changes app sources nor data.
 * Match a resolved Rust pthread_create caller PC before delaying that child's
 * entry point, independently of the application's deadline implementation. */
static int (*real_create)(pthread_t *, const pthread_attr_t *, void *(*)(void *), void *);
static int (*real_sleep)(const struct timespec *, struct timespec *);
static int trace_fd = -1;
static unsigned long match_pc, delay_ms;
static long long millis(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC,&t); return (long long)t.tv_sec*1000+t.tv_nsec/1000000; }
static void event(const char *kind, unsigned long id, const char *extra) {
    char line[2048]; int n=snprintf(line,sizeof(line),"%lld tid=%ld %s id=%lu %s\n",millis(),syscall(SYS_gettid),kind,id,extra);
    if(trace_fd>=0 && n>0 && n<(int)sizeof(line)) (void)write(trace_fd,line,(size_t)n);
}
__attribute__((constructor)) static void init(void) {
    real_create=dlsym(RTLD_NEXT,"pthread_create"); real_sleep=dlsym(RTLD_NEXT,"nanosleep");
    const char *path=getenv("CHORD_THREAD_TRACE"); if(path) trace_fd=open(path,O_WRONLY|O_CREAT|O_APPEND,0600);
    const char *pc=getenv("CHORD_DELAY_CALLSITE"); if(pc) match_pc=strtoul(pc,NULL,16);
    const char *ms=getenv("CHORD_DELAY_MS"); if(ms) delay_ms=strtoul(ms,NULL,10);
    if(!real_create || !real_sleep) _exit(125);
}
struct entry { void *(*start)(void *); void *arg; unsigned long id, delay; };
static void *entry(void *raw) {
    struct entry e=*(struct entry *)raw; free(raw); event("child_enter",e.id,"");
    if(e.delay) { struct timespec t={e.delay/1000,(e.delay%1000)*1000000}; while(real_sleep(&t,&t)<0) {} }
    event("rust_entry",e.id,""); return e.start(e.arg);
}
int pthread_create(pthread_t *thread,const pthread_attr_t *attr,void *(*start)(void *),void *arg) {
    static unsigned long serial; unsigned long id=__atomic_add_fetch(&serial,1,__ATOMIC_SEQ_CST);
    void *frames[24]; int count=backtrace(frames,24), matched=0; char pcs[1200]={0}; size_t used=0;
    for(int i=1;i<count;i++) { Dl_info d; if(dladdr(frames[i],&d) && d.dli_fbase && d.dli_fname && !strstr(d.dli_fname,".so")) {
        unsigned long pc=(uintptr_t)frames[i]-(uintptr_t)d.dli_fbase; if(pc==match_pc) matched=1;
        int n=snprintf(pcs+used,sizeof(pcs)-used,"0x%lx,",pc); if(n>0 && (size_t)n<sizeof(pcs)-used) used+=(size_t)n;
    }}
    event(matched?"create_delayed":"create",id,pcs);
    struct entry *e=malloc(sizeof(*e)); if(!e) return 11;
    *e=(struct entry){start,arg,id,matched?delay_ms:0};
    int rc=real_create(thread,attr,entry,e); if(rc) free(e); return rc;
}
int nanosleep(const struct timespec *request,struct timespec *remaining) {
    int trace=request->tv_sec==0 && (request->tv_nsec==120000000 || request->tv_nsec==160000000);
    if(trace) event("sleep_begin",request->tv_nsec/1000000,"");
    int rc=real_sleep(request,remaining);
    if(trace) event("sleep_end",request->tv_nsec/1000000,"");
    return rc;
}
