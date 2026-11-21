//! Concrete resource layouts and their native operations.
use super::{c_type, suffix};
use crate::core::{Layout, LayoutKind, Program, Resource, Ty};
use crate::types::Type;
use std::fmt::Write;
#[path = "native_arena.rs"]
mod arena;

pub(super) fn mutex_for(program: &Program, payload: Ty) -> usize {
    program.layouts.iter().position(|layout| matches!(layout.kind, LayoutKind::Resource(Resource::Mutex(t)) if t == payload)).expect("guard mutex layout")
}
pub(super) fn declare(program: &Program, id: usize, resource: Resource, out: &mut String) {
    match resource {
        Resource::Reader | Resource::Writer => writeln!(out, "    FILE *file;").unwrap(),
        Resource::Guard(payload) => writeln!(
            out,
            "    struct KResourceData_{} *handle;",
            mutex_for(program, payload)
        )
        .unwrap(),
        _ => writeln!(out, "    struct KResourceData_{id} *handle;").unwrap(),
    }
}
pub(super) fn declare_data(program: &Program, out: &mut String) {
    for (id, layout) in program.layouts.iter().enumerate() {
        match layout.kind {
            LayoutKind::Resource(Resource::Arena(t)) => writeln!(out,
                "struct KResourceData_{id} {{ uint64_t identity; size_t length, capacity; {} *items; }};",
                c_type(t)).unwrap(),
            LayoutKind::Resource(Resource::Mutex(t)) => writeln!(
                out,
                "struct KResourceData_{id} {{ pthread_mutex_t mutex; {} value; }};",
                c_type(t)
            )
            .unwrap(),
            LayoutKind::Resource(Resource::Channel(t)) => {
                // An unbounded queue of fixed-size chunks. The sending end, the published
                // count, the receiving end and the sleep state each sit on their own cache
                // line, so the two ends do not fight over one line on every item.
                let c = c_type(t);
                writeln!(out, "enum {{ K_CHUNK_{id} = sizeof({c}) <= 8 ? 1024 : 256, K_BATCH_{id} = sizeof({c}) >= 128 ? 1 : 128 / sizeof({c}) }};\ntypedef struct KChannelChunk_{id} {{ _Atomic(struct KChannelChunk_{id} *) next; {c} items[K_CHUNK_{id}]; }} KChannelChunk_{id};\nstruct KResourceData_{id} {{\n    _Alignas(128) atomic_bool send_lock; KChannelChunk_{id} *tail; size_t sent;\n    _Alignas(128) atomic_size_t published;\n    _Alignas(128) atomic_bool receive_lock; atomic_bool streaming; KChannelChunk_{id} *head; size_t taken, limit;\n    _Alignas(128) atomic_bool closed; atomic_size_t waiting; pthread_mutex_t mutex; pthread_cond_t ready;\n}};").unwrap();
            }
            LayoutKind::Resource(Resource::Atomic(t)) => writeln!(
                out,
                "struct KResourceData_{id} {{ _Atomic({}) value; }};",
                if t == Ty::Double {
                    "uint64_t"
                } else {
                    "int64_t"
                }
            )
            .unwrap(),
            _ => {}
        }
    }
}
pub(super) fn drop_body(_program: &Program, id: usize, resource: Resource, out: &mut String) {
    match resource {
        Resource::Arena(t) => {
            writeln!(out, "    struct KResourceData_{id} *h = value->handle; value->handle = NULL; if (h) {{ k_drop_push(stack,h,k_drop_free); k_drop_push(stack,h->items,k_drop_free);").unwrap();
            if t.owns() {
                writeln!(out, "    for (size_t i=0; i<h->length; ++i) k_drop_push(stack,&h->items[i],k_{}_drop_task);", suffix(t)).unwrap();
            }
            writeln!(out, "    }}").unwrap();
        }
        Resource::Reader | Resource::Writer => {
            writeln!(out, "    FILE *file = value->file; value->file = NULL; if (file && fclose(file)) k_fatal(\"Cannot close file\");").unwrap();
        }
        Resource::Guard(_) => {
            writeln!(out, "    if (value->handle) {{ if (pthread_mutex_unlock(&value->handle->mutex)) k_fatal(\"Cannot unlock Mutex\"); value->handle = NULL; }}").unwrap();
        }
        Resource::Mutex(t) => {
            writeln!(out, "    if (value->handle) {{ if (pthread_mutex_destroy(&value->handle->mutex)) k_fatal(\"Cannot release an acquired Mutex\"); k_drop_push(stack,value->handle,k_drop_free);").unwrap();
            if t.owns() {
                writeln!(
                    out,
                    "    k_drop_push(stack,&value->handle->value,k_{}_drop_task);",
                    suffix(t)
                )
                .unwrap();
            }
            writeln!(out, "    value->handle = NULL; }}").unwrap();
        }
        Resource::Channel(t) => {
            // Frees go on the stack first, so the queued items drop before their chunks.
            writeln!(out, "    struct KResourceData_{id} *h = value->handle; value->handle = NULL; if (h) {{ pthread_mutex_destroy(&h->mutex); pthread_cond_destroy(&h->ready); k_drop_push(stack,h,k_drop_aligned_free); for (KChannelChunk_{id} *c = h->head; c; c = atomic_load_explicit(&c->next,memory_order_relaxed)) k_drop_push(stack,c,k_drop_free);").unwrap();
            if t.owns() {
                writeln!(out, "    KChannelChunk_{id} *c = h->head; size_t base = h->taken ? ((h->taken - 1) / K_CHUNK_{id}) * K_CHUNK_{id} : 0; for (size_t p = h->taken; p < h->sent; ++p) {{ while (p >= base + K_CHUNK_{id}) {{ c = atomic_load_explicit(&c->next,memory_order_relaxed); base += K_CHUNK_{id}; }} k_drop_push(stack,&c->items[p - base],k_{}_drop_task); }}", suffix(t)).unwrap();
            }
            writeln!(out, "    }}").unwrap();
        }
        Resource::Atomic(_) => {
            writeln!(out, "    free(value->handle); value->handle = NULL;").unwrap()
        }
    }
}
fn union(program: &Program, success: &Type) -> Option<(usize, usize, usize)> {
    program.layouts.iter().enumerate().find_map(|(id, l)| {
        let LayoutKind::Union(parts, _) = &l.kind else {
            return None;
        };
        if parts.len() != 2 {
            return None;
        }
        Some((
            id,
            parts.iter().position(|t| t == success)?,
            parts.iter().position(|t| *t == Type::named("Error"))?,
        ))
    })
}
fn error(out: &mut String, kind: u32, message: &str) {
    write!(out, "k_runtime_error({kind},K_TEXT(\"{message}\"))").unwrap();
}
pub(super) fn emit(program: &Program, out: &mut String) {
    if let Some((result, success, failure)) = union(program, &Type::named("String")) {
        writeln!(out,"static KRecord_{result} k_readLine(void) {{ KReadResult line = k_file_read_line(stdin); if (line.error.type) return k_aggregate_{result}_{failure}(line.error); return k_aggregate_{result}_{success}(line.text); }}").unwrap();
    }
    for (id, layout) in program.layouts.iter().enumerate() {
        let LayoutKind::Resource(resource) = layout.kind else {
            continue;
        };
        match resource {
            Resource::Arena(t) => arena::emit(program, id, t, out),
            Resource::Reader | Resource::Writer => file(program, id, layout, resource, out),
            Resource::Mutex(t) => mutex(program, id, t, out),
            Resource::Channel(t) => channel(program, id, t, out),
            Resource::Atomic(t) => atomic(program, id, t, out),
            Resource::Guard(_) => {}
        }
    }
}
fn file(program: &Program, id: usize, layout: &Layout, resource: Resource, out: &mut String) {
    if let Some((result, success, failure)) = union(program, &layout.source) {
        let mode = if matches!(resource, Resource::Reader) {
            "rb"
        } else {
            "wb"
        };
        let op = if matches!(resource, Resource::Reader) {
            "open"
        } else {
            "create"
        };
        writeln!(out,"static KRecord_{result} k_resource_{id}_{op}(KString path) {{ if (memchr(path.data,0,path.length)) return k_aggregate_{result}_{failure}(k_runtime_error(3,K_TEXT(\"A file path contains NUL\"))); FILE *file = fopen(path.data,\"{mode}\"); if (!file) return k_aggregate_{result}_{failure}(k_io_error(\"Cannot {op} file\")); return k_aggregate_{result}_{success}((KRecord_{id}) {{ file }}); }}").unwrap();
    }
    writeln!(out,"static KError k_resource_{id}_close(KRecord_{id} *value) {{ FILE *file = value->file; value->file = NULL; if (file && fclose(file)) return k_io_error(\"Cannot close file\"); return (KError) {{0}}; }}").unwrap();
    if matches!(resource, Resource::Reader) {
        if let Some((result, success, failure)) = union(program, &Type::named("String")) {
            writeln!(out,"static KRecord_{result} k_resource_{id}_readLine(KRecord_{id} *reader) {{ if (!reader->file) return k_aggregate_{result}_{failure}(k_runtime_error(3,K_TEXT(\"Reader is closed\"))); KReadResult result = k_file_read_line(reader->file); if (result.error.type) return k_aggregate_{result}_{failure}(result.error); return k_aggregate_{result}_{success}(result.text); }}").unwrap();
        }
    } else if let Some((result, success, failure)) = union(program, &Type::named("void")) {
        writeln!(out,"static KRecord_{result} k_resource_{id}_writeLine(KRecord_{id} *writer, KString text) {{ if (!writer->file) return k_aggregate_{result}_{failure}(k_runtime_error(3,K_TEXT(\"Writer is closed\"))); if (fwrite(text.data,1,text.length,writer->file) != text.length || fputc('\\n',writer->file) == EOF) return k_aggregate_{result}_{failure}(k_io_error(\"Cannot write line\")); return k_aggregate_{result}_{success}(); }}").unwrap();
    }
}
fn mutex(program: &Program, id: usize, t: Ty, out: &mut String) {
    writeln!(out,"static KRecord_{id} k_resource_{id}_new({} owned) {{ struct KResourceData_{id} *handle = malloc(sizeof *handle); if (!handle) k_fatal(\"Out of memory\"); pthread_mutexattr_t attributes; if (pthread_mutexattr_init(&attributes)) k_fatal(\"Cannot initialize Mutex attributes\"); if (pthread_mutexattr_settype(&attributes,PTHREAD_MUTEX_ERRORCHECK)) k_fatal(\"Cannot configure Mutex\"); int status = pthread_mutex_init(&handle->mutex,&attributes); pthread_mutexattr_destroy(&attributes); if (status) k_fatal(\"Cannot initialize Mutex\"); handle->value = owned; return (KRecord_{id}) {{ handle }}; }}",c_type(t)).unwrap();
    let guard = Type::Guard(Box::new(match &program.layouts[id].source {
        Type::Named(_, a) => a[0].clone(),
        _ => unreachable!(),
    }));
    if let Some((result, success, failure)) = union(program, &guard) {
        let payload = match &program.layouts[result].kind {
            LayoutKind::Union(_, cases) => match cases[success].fields[0] {
                Ty::Record(id) => id,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };
        write!(out,"static KRecord_{result} k_resource_{id}_lock(KRecord_{id} *mutex) {{ if (!mutex->handle) return k_aggregate_{result}_{failure}(k_runtime_error(1,K_TEXT(\"Mutex was moved\"))); int status = pthread_mutex_lock(&mutex->handle->mutex); if (status) return k_aggregate_{result}_{failure}(status == EDEADLK ? ").unwrap();
        error(out, 1, "Mutex is already locked in this thread");
        writeln!(out," : k_runtime_error(3,K_TEXT(\"Cannot lock Mutex\"))); return k_aggregate_{result}_{success}((KRecord_{payload}) {{mutex->handle}}); }}").unwrap();
    }
}
fn channel(program: &Program, id: usize, t: Ty, out: &mut String) {
    let c = c_type(t);
    writeln!(out,"static KChannelChunk_{id} *k_channel_{id}_chunk(void) {{ KChannelChunk_{id} *c = malloc(sizeof *c); if (!c) k_fatal(\"Out of memory\"); atomic_init(&c->next,NULL); return c; }}").unwrap();
    writeln!(out,"static KRecord_{id} k_resource_{id}_new(void) {{ struct KResourceData_{id} *h = k_aligned_alloc(128,sizeof *h); if (!h) k_fatal(\"Out of memory\"); memset(h,0,sizeof *h); h->head = h->tail = k_channel_{id}_chunk(); if (pthread_mutex_init(&h->mutex,NULL) || pthread_cond_init(&h->ready,NULL)) k_fatal(\"Cannot initialize Channel\"); return (KRecord_{id}) {{h}}; }}").unwrap();
    writeln!(out,"static void k_channel_{id}_wake(struct KResourceData_{id} *h, bool all) {{ k_pthread_check(pthread_mutex_lock(&h->mutex)); if (all) k_pthread_check(pthread_cond_broadcast(&h->ready)); else k_pthread_check(pthread_cond_signal(&h->ready)); k_pthread_check(pthread_mutex_unlock(&h->mutex)); }}").unwrap();
    // Closing under the send lock orders it after every accepted send.
    writeln!(out,"static void k_resource_{id}_close(KRecord_{id} *channel) {{ struct KResourceData_{id} *h = channel->handle; if (!h) k_fatal(\"Channel was moved\"); k_spin_lock(&h->send_lock); atomic_store_explicit(&h->closed,true,memory_order_seq_cst); k_spin_unlock(&h->send_lock); k_channel_{id}_wake(h,true); }}").unwrap();
    if let Some((result, success, failure)) = union(program, &Type::named("void")) {
        let release = if t.owns() {
            format!("k_{}_drop(&owned); ", suffix(t))
        } else {
            String::new()
        };
        // The published store and the waiting load are both sequentially consistent:
        // either the receiver sees the item, or the sender sees the receiver waiting.
        writeln!(out,"static KRecord_{result} k_resource_{id}_send(KRecord_{id} *channel, {c} owned) {{ struct KResourceData_{id} *h = channel->handle; if (!h) k_fatal(\"Channel was moved\"); k_spin_lock(&h->send_lock); if (atomic_load_explicit(&h->closed,memory_order_relaxed)) {{ k_spin_unlock(&h->send_lock); {release}return k_aggregate_{result}_{failure}(k_runtime_error(1,K_TEXT(\"Channel is closed\"))); }} size_t slot = h->sent % K_CHUNK_{id}; if (slot == 0 && h->sent) {{ KChannelChunk_{id} *next = k_channel_{id}_chunk(); atomic_store_explicit(&h->tail->next,next,memory_order_release); h->tail = next; }} h->tail->items[slot] = owned; ++h->sent; atomic_store_explicit(&h->published,h->sent,memory_order_seq_cst); k_spin_unlock(&h->send_lock); if (atomic_load_explicit(&h->waiting,memory_order_seq_cst)) k_channel_{id}_wake(h,false); return k_aggregate_{result}_{success}(); }}").unwrap();
    }
    let Type::Named(_, args) = &program.layouts[id].source else {
        unreachable!()
    };
    if let Some((option, _)) = program
        .layouts
        .iter()
        .enumerate()
        .find(|(_, l)| l.source == Type::applied("Option", args.clone()))
    {
        // Takes one item when at least `batch` are published. Holds receive_lock.
        writeln!(out,"static bool k_channel_{id}_take(struct KResourceData_{id} *h, {c} *value, size_t batch) {{ if (h->taken == h->limit) {{ size_t published = atomic_load_explicit(&h->published,memory_order_seq_cst); if (published - h->taken < batch) return false; if (published - h->taken >= K_BATCH_{id}) atomic_store_explicit(&h->streaming,true,memory_order_relaxed); h->limit = published; }} size_t slot = h->taken % K_CHUNK_{id}; if (slot == 0 && h->taken) {{ KChannelChunk_{id} *next = atomic_load_explicit(&h->head->next,memory_order_acquire); free(h->head); h->head = next; }} *value = h->head->items[slot]; ++h->taken; return true; }}").unwrap();
        // While the sender streams, the receiver briefly waits for a cache line of
        // items instead of taking each one as the sender writes its line. A lone
        // message never sets `streaming`, so it is taken at once. After a bounded
        // spin the receiver sleeps on the condition variable.
        writeln!(out,"static bool k_channel_{id}_poll(struct KResourceData_{id} *h, {c} *value, size_t batch) {{ k_spin_lock(&h->receive_lock); bool found = k_channel_{id}_take(h,value,batch); k_spin_unlock(&h->receive_lock); return found; }}").unwrap();
        writeln!(out,"static KRecord_{option} k_resource_{id}_receive(KRecord_{id} *channel) {{ struct KResourceData_{id} *h = channel->handle; if (!h) k_fatal(\"Channel was moved\"); {c} value; for (unsigned spin = 0; spin < 12; ++spin) {{ if (spin == 10) atomic_store_explicit(&h->streaming,false,memory_order_relaxed); if (k_channel_{id}_poll(h,&value,spin < 10 && atomic_load_explicit(&h->streaming,memory_order_relaxed) ? K_BATCH_{id} : 1)) return k_aggregate_{option}_0(value); if (atomic_load_explicit(&h->closed,memory_order_seq_cst)) return k_channel_{id}_poll(h,&value,1) ? k_aggregate_{option}_0(value) : k_aggregate_{option}_1(); for (unsigned i = 0; i < (1u << (spin < 7 ? spin : 7)); ++i) k_cpu_relax(); }} k_pthread_check(pthread_mutex_lock(&h->mutex)); atomic_fetch_add_explicit(&h->waiting,1,memory_order_seq_cst); bool found; for (;;) {{ found = k_channel_{id}_poll(h,&value,1); if (found) break; if (atomic_load_explicit(&h->closed,memory_order_seq_cst)) {{ found = k_channel_{id}_poll(h,&value,1); break; }} k_pthread_check(pthread_cond_wait(&h->ready,&h->mutex)); }} atomic_fetch_sub_explicit(&h->waiting,1,memory_order_seq_cst); k_pthread_check(pthread_mutex_unlock(&h->mutex)); return found ? k_aggregate_{option}_0(value) : k_aggregate_{option}_1(); }}").unwrap();
    }
}
fn atomic(program: &Program, id: usize, t: Ty, out: &mut String) {
    let c = c_type(t);
    let store = if t == Ty::Double {
        "k_double_bits(value)"
    } else {
        "value"
    };
    let load = if t == Ty::Double {
        "k_bits_double(atomic_load(&atomic->handle->value))"
    } else {
        "atomic_load(&atomic->handle->value)"
    };
    writeln!(out,"static KRecord_{id} k_resource_{id}_new({c} value) {{ struct KResourceData_{id} *h = malloc(sizeof *h); if (!h) k_fatal(\"Out of memory\"); atomic_init(&h->value,{store}); return (KRecord_{id}) {{h}}; }}").unwrap();
    writeln!(
        out,
        "static {c} k_resource_{id}_load(KRecord_{id} *atomic) {{ return {load}; }}"
    )
    .unwrap();
    writeln!(out,"static void k_resource_{id}_store(KRecord_{id} *atomic, {c} value) {{ atomic_store(&atomic->handle->value,{store}); }}").unwrap();
    if t == Ty::Int {
        if let Some((result, success, failure)) = union(program, &Type::named("Int")) {
            writeln!(out,"static KRecord_{result} k_resource_{id}_fetchAdd(KRecord_{id} *atomic, int64_t value) {{ int64_t old = atomic_load(&atomic->handle->value); for (;;) {{ int64_t next; if (__builtin_add_overflow(old,value,&next)) return k_aggregate_{result}_{failure}(k_runtime_error(0,K_TEXT(\"Atomic Int addition overflow\"))); if (atomic_compare_exchange_weak(&atomic->handle->value,&old,next)) return k_aggregate_{result}_{success}(old); }} }}").unwrap();
        }
        writeln!(out,"static bool k_resource_{id}_compareExchange(KRecord_{id} *atomic, int64_t expected, int64_t value) {{ return atomic_compare_exchange_strong(&atomic->handle->value,&expected,value); }}").unwrap();
    } else {
        writeln!(out,"static double k_resource_{id}_fetchAdd(KRecord_{id} *atomic, double value) {{ uint64_t old = atomic_load(&atomic->handle->value); for (;;) {{ uint64_t next = k_double_bits(k_bits_double(old) + value); if (atomic_compare_exchange_weak(&atomic->handle->value,&old,next)) return k_bits_double(old); }} }}").unwrap();
        writeln!(out,"static bool k_resource_{id}_compareExchange(KRecord_{id} *atomic, double expected, double value) {{ uint64_t bits = k_double_bits(expected); return atomic_compare_exchange_strong(&atomic->handle->value,&bits,k_double_bits(value)); }}").unwrap();
    }
}
