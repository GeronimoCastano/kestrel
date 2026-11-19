//! Hash tables with counted immutable entries and copy-on-write persistent handles.
use super::{c_type, copies, suffix};
use crate::{
    core::{LayoutKind, MapLayout, MapStyle, Program, Ty},
    types::Type,
};
use std::fmt::Write;

pub(super) fn flat(info: MapLayout) -> bool {
    info.style == MapStyle::Map
        && matches!(info.key, Ty::Int | Ty::Nat | Ty::Double | Ty::Bool)
        && matches!(info.value, Ty::Int | Ty::Nat | Ty::Double | Ty::Bool)
}

pub(super) fn declare_data(program: &Program, out: &mut String) {
    for (id, layout) in program.layouts.iter().enumerate() {
        let LayoutKind::Map(info) = layout.kind else {
            continue;
        };
        let key = c_type(info.key);
        let value = c_type(info.value);
        if flat(info) {
            writeln!(out,"typedef union KMapEntry_{id} {{ struct {{ {key} key; {value} value; }}; KRecord_{} item; }} KMapEntry_{id};\nstruct KMapData_{id} {{ atomic_size_t references;size_t length,capacity;KMapEntry_{id} *entries;uint64_t *buckets; }};",info.item).unwrap();
            continue;
        }
        if !copies(program, info.key) && matches!(info.key, Ty::Record(_)) {
            writeln!(out,"typedef struct KMapKey_{id} {{ atomic_size_t references; {key} value; }} KMapKey_{id};").unwrap();
        }
        let key = if copies(program, info.key) || !matches!(info.key, Ty::Record(_)) {
            key
        } else {
            format!("KMapKey_{id} *")
        };
        writeln!(out,"typedef struct KMapEntry_{id} {{ atomic_size_t references; uint64_t hash; {key} key; {value} value; KRecord_{} item; }} KMapEntry_{id};",info.item).unwrap();
        writeln!(out,"struct KMapData_{id} {{ atomic_size_t references; size_t length,capacity; KMapEntry_{id} **buckets, **ordered; uint64_t *hashes; }};").unwrap();
        writeln!(
            out,
            "static void k_map_{id}_entry_drop_task(void *,KDropStack *);"
        )
        .unwrap();
    }
}
pub(super) fn drop_body(id: usize, info: MapLayout, out: &mut String) {
    if flat(info) {
        writeln!(out,"    struct KMapData_{id} *data=value->handle;value->handle=NULL;if(data && atomic_fetch_sub_explicit(&data->references,1,memory_order_acq_rel)==1) {{free(data->entries);free(data->buckets);free(data);}}").unwrap();
        return;
    }
    writeln!(out,"    struct KMapData_{id} *data = value->handle; value->handle = NULL; if (data && atomic_fetch_sub_explicit(&data->references,1,memory_order_acq_rel) == 1) {{ k_drop_push(stack,data,k_drop_free); k_drop_push(stack,data->buckets,k_drop_free);k_drop_push(stack,data->ordered,k_drop_free);k_drop_push(stack,data->hashes,k_drop_free); for (size_t i=0; i<data->capacity; ++i) if (data->buckets[i]) k_drop_run(data->buckets[i],k_map_{id}_entry_drop_task); }}").unwrap();
}
fn key_expr(program: &Program, info: MapLayout, entry: &str) -> String {
    if !copies(program, info.key) && matches!(info.key, Ty::Record(_)) {
        format!("{entry}->key->value")
    } else {
        format!("{entry}->key")
    }
}
fn copy_expr(program: &Program, ty: Ty, value: &str) -> String {
    if copies(program, ty) {
        format!("k_{}_copy({value})", suffix(ty))
    } else {
        value.to_owned()
    }
}
fn needle_type(program: &Program, key: Ty) -> String {
    if !copies(program, key)
        && let Ty::Record(id) = key
    {
        c_type(Ty::Ref(id))
    } else {
        c_type(key)
    }
}
fn needle_expr(program: &Program, key: Ty, value: &str) -> String {
    if !copies(program, key) && matches!(key, Ty::Record(_)) {
        format!("&({value})")
    } else {
        value.to_owned()
    }
}
pub(super) fn emit(program: &Program, out: &mut String) {
    for (id, l) in program.layouts.iter().enumerate() {
        if matches!(l.kind, LayoutKind::Map(_)) {
            writeln!(out, "static KRecord_{id} k_record_{id}_copy(KRecord_{id});").unwrap();
        }
    }
    for (id, layout) in program.layouts.iter().enumerate() {
        let LayoutKind::Map(info) = layout.kind else {
            continue;
        };
        let key = c_type(info.key);
        let value = c_type(info.value);
        let needle = needle_type(program, info.key);
        let old_key = key_expr(program, info, "entry");
        if !copies(program, info.key) && matches!(info.key, Ty::Record(_)) {
            writeln!(out,"static void k_map_{id}_key_free(void *key,KDropStack *stack) {{ (void)stack; k_small_free(key,sizeof(KMapKey_{id})); }}").unwrap();
            writeln!(out,"static void k_map_{id}_key_drop_task(void *payload,KDropStack *stack) {{KMapKey_{id} *key=payload;if(atomic_fetch_sub_explicit(&key->references,1,memory_order_acq_rel)==1) {{k_drop_push(stack,key,k_map_{id}_key_free);k_drop_push(stack,&key->value,k_{}_drop_task);}}}}",suffix(info.key)).unwrap();
        }

        writeln!(out,"static KRecord_{id} k_record_{id}_copy(KRecord_{id} value) {{ if (value.handle && atomic_fetch_add_explicit(&value.handle->references,1,memory_order_relaxed)==SIZE_MAX) k_fatal(\"Map reference count overflow\"); return value; }}").unwrap();
        if flat(info) {
            writeln!(out,"static void k_map_{id}_entry_drop_task(void *payload,KDropStack *stack) {{(void)payload;(void)stack;}}").unwrap();
        } else {
            writeln!(out,"static void k_map_{id}_entry_drop_task(void *payload,KDropStack *stack) {{ KMapEntry_{id} *entry=payload; if (atomic_fetch_sub_explicit(&entry->references,1,memory_order_acq_rel)==1) {{").unwrap();
            if !copies(program, info.key) && matches!(info.key, Ty::Record(_)) {
                writeln!(out, "    k_map_{id}_key_drop_task(entry->key,stack);").unwrap();
            } else if info.key.owns() {
                writeln!(
                    out,
                    "    k_{}_drop_task(&entry->key,stack);",
                    suffix(info.key)
                )
                .unwrap();
            }
            if info.value.owns() {
                writeln!(
                    out,
                    "    k_{}_drop_task(&entry->value,stack);",
                    suffix(info.value)
                )
                .unwrap();
            }
            writeln!(out, "    k_small_free(entry,sizeof *entry); }} }}").unwrap();
        }
        let hash: String = match info.key {
            Ty::Int | Ty::Nat | Ty::Bool => "k_hash_mix((uint64_t)key)".into(),
            Ty::Double if info.style == MapStyle::SortedSet => {
                "k_hash_mix(isnan(key) ? UINT64_C(0x7ff8000000000000) : k_double_bits(key))".into()
            }
            Ty::Double => "k_hash_mix(key==0.0 ? 0 : k_double_bits(key))".into(),
            Ty::String => "k_hash_string(key)".into(),
            Ty::Record(id) if matches!(program.layouts[id].kind, LayoutKind::BigInt) => {
                "k_bigint_hash(&key->value)".into()
            }
            Ty::ArenaRef(_) => "k_hash_mix(key.arena ^ k_hash_mix((uint64_t)key.slot))".into(),
            // Tuple, enum and record keys hash their parts (REVIEW 1.3).
            _ => info
                .hash
                .map(|function| format!("(uint64_t)k_fn_{function}(key)"))
                .unwrap_or("UINT64_C(0)".into()),
        };
        writeln!(
            out,
            "static uint64_t k_map_{id}_hash({needle} key) {{ (void)key; return {hash}; }}"
        )
        .unwrap();
        if info.style == MapStyle::SortedSet {
            let comparison = match info.key {
                Ty::String => format!("k_compare_string({old_key},key)"),
                Ty::Double => format!("k_compare_double({old_key},key)"),
                Ty::Int | Ty::Nat | Ty::Bool => {
                    format!("((int64_t)({old_key}>key)-(int64_t)({old_key}<key))")
                }
                _ => info
                    .equality
                    .map(|function| {
                        format!(
                            "k_fn_{function}({},key)",
                            needle_expr(program, info.key, &old_key)
                        )
                    })
                    .unwrap_or("0".into()),
            };
            writeln!(out,"static int64_t k_map_{id}_compare(KMapEntry_{id} *entry,{needle} key) {{return {comparison};}}").unwrap();
        }
        let equality = if info.style == MapStyle::SortedSet {
            format!("k_map_{id}_compare(entry,key)==0")
        } else {
            match info.key {
                Ty::String => format!("k_string_equal({old_key},key)"),
                Ty::ArenaRef(_) => format!("k_arenaref_equal({old_key},key)"),
                Ty::Int | Ty::Nat | Ty::Double | Ty::Bool => format!("({old_key}==key)"),
                _ => info
                    .equality
                    .map(|function| {
                        format!(
                            "k_fn_{function}({},key)",
                            needle_expr(program, info.key, &old_key)
                        )
                    })
                    .unwrap_or("false".into()),
            }
        };
        writeln!(out,"static bool k_map_{id}_equal(KMapEntry_{id} *entry,{needle} key) {{ return {equality}; }}").unwrap();
        if flat(info) {
            // A bucket holds the entry's slot + 1 in its low half and the top half of
            // the hash in its high half, so most probes skip a key comparison.
            writeln!(out,"static KMapEntry_{id} *k_map_{id}_entry(struct KMapData_{id} *data,size_t index) {{uint64_t bucket=data->buckets[index];return bucket ? &data->entries[K_MAP_SLOT(bucket)-1] : NULL;}}\nstatic size_t k_map_{id}_find(struct KMapData_{id} *data,uint64_t hash,{needle} key) {{size_t mask=data->capacity-1,index=(size_t)hash&mask;uint64_t tag=K_MAP_TAG(hash);for(;;) {{uint64_t bucket=data->buckets[index];if(!bucket || (K_MAP_TAG(bucket)==tag && k_map_{id}_equal(&data->entries[K_MAP_SLOT(bucket)-1],key))) return index;index=(index+1)&mask;}}}}").unwrap();
        } else {
            writeln!(out,"static size_t k_map_{id}_find(struct KMapData_{id} *data,uint64_t hash,{needle} key) {{ size_t index=(size_t)hash & (data->capacity-1); while (data->buckets[index] && (data->hashes[index]!=hash || !k_map_{id}_equal(data->buckets[index],key))) index=(index+1)&(data->capacity-1); return index; }}").unwrap();
        }
        let ordered_init: String = if info.style == MapStyle::SortedSet {
            "data->ordered=malloc(data->capacity*sizeof *data->ordered);if(!data->ordered) k_fatal(\"Out of memory\");".into()
        } else {
            String::new()
        };
        let ordered_copy: String = if info.style == MapStyle::SortedSet {
            "copy->ordered=malloc(copy->capacity*sizeof *copy->ordered);if(!copy->ordered) k_fatal(\"Out of memory\");memcpy(copy->ordered,data->ordered,copy->length*sizeof *copy->ordered);".into()
        } else {
            "copy->ordered=NULL;".into()
        };
        let ordered_grow = if info.style == MapStyle::SortedSet {
            format!(
                "KMapEntry_{id} **ordered=realloc(data->ordered,capacity*sizeof *ordered);if(!ordered) k_fatal(\"Out of memory\");data->ordered=ordered;"
            )
        } else {
            String::new()
        };
        if flat(info) {
            emit_flat_storage(id, out);
        } else {
            writeln!(out,"static void k_map_{id}_unique(KRecord_{id} *map) {{ struct KMapData_{id} *data=map->handle; if (!data) {{ data=calloc(1,sizeof *data); if (!data) k_fatal(\"Out of memory\"); atomic_init(&data->references,1); data->capacity=8; data->buckets=calloc(data->capacity,sizeof *data->buckets); data->hashes=malloc(data->capacity*sizeof *data->hashes); if (!data->buckets || !data->hashes) k_fatal(\"Out of memory\"); {ordered_init}map->handle=data; }} else if (atomic_load_explicit(&data->references,memory_order_acquire)!=1) {{ struct KMapData_{id} *copy=malloc(sizeof *copy); if (!copy) k_fatal(\"Out of memory\"); atomic_init(&copy->references,1); copy->length=data->length; copy->capacity=data->capacity; copy->buckets=malloc(copy->capacity*sizeof *copy->buckets); if (!copy->buckets) k_fatal(\"Out of memory\"); memcpy(copy->buckets,data->buckets,copy->capacity*sizeof *copy->buckets); copy->hashes=malloc(copy->capacity*sizeof *copy->hashes); if (!copy->hashes) k_fatal(\"Out of memory\"); memcpy(copy->hashes,data->hashes,copy->capacity*sizeof *copy->hashes);{ordered_copy} for (size_t i=0;i<copy->capacity;++i) if (copy->buckets[i] && atomic_fetch_add_explicit(&copy->buckets[i]->references,1,memory_order_relaxed)==SIZE_MAX) k_fatal(\"Map entry reference count overflow\"); KRecord_{id} old=*map; map->handle=copy; k_record_{id}_drop(&old); }} }}").unwrap();
            writeln!(out,"static void k_map_{id}_grow(struct KMapData_{id} *data) {{ if (data->capacity>SIZE_MAX/2/sizeof *data->buckets) k_fatal(\"Map is too large\"); size_t capacity=data->capacity*2; KMapEntry_{id} **buckets=calloc(capacity,sizeof *buckets); uint64_t *hashes=malloc(capacity*sizeof *hashes); if (!buckets || !hashes) k_fatal(\"Out of memory\"); for (size_t i=0;i<data->capacity;++i) if (data->buckets[i]) {{ uint64_t hash=data->hashes[i];size_t index=(size_t)hash & (capacity-1);while (buckets[index]) index=(index+1)&(capacity-1);buckets[index]=data->buckets[i];hashes[index]=hash; }} {ordered_grow}free(data->buckets);free(data->hashes);data->buckets=buckets;data->hashes=hashes;data->capacity=capacity; }}").unwrap();
        }
        let duplicate = if info.style != MapStyle::Map {
            let drop = if info.key.owns() {
                format!("k_{}_drop(&key);", suffix(info.key))
            } else {
                String::new()
            };
            format!(
                "if(map->handle) {{size_t found=k_map_{id}_find(map->handle,k_map_{id}_hash({0}),{0});if(map->handle->buckets[found]) {{{drop}return;}}}}",
                needle_expr(program, info.key, "key")
            )
        } else {
            String::new()
        };
        let allocate = if flat(info) {
            format!("KMapEntry_{id} *entry=previous ? previous : &data->entries[data->length];")
        } else {
            format!(
                "KMapEntry_{id} *entry=k_small_alloc(sizeof *entry);atomic_init(&entry->references,1);entry->hash=hash;"
            )
        };
        let lookup = if flat(info) {
            format!("k_map_{id}_entry(data,index)")
        } else {
            "data->buckets[index]".into()
        };
        writeln!(out,"static void k_map_{id}_put_owned(KRecord_{id} *map,{key} key,{value} value) {{{duplicate} k_map_{id}_unique(map);struct KMapData_{id} *data=map->handle;uint64_t hash=k_map_{id}_hash({});size_t index=k_map_{id}_find(data,hash,{});KMapEntry_{id} *previous={lookup};if (!previous && data->length>=data->capacity-(data->capacity/4)) {{ k_map_{id}_grow(data);index=k_map_{id}_find(data,hash,{}); }} {allocate}entry->value=value;",needle_expr(program,info.key,"key"),needle_expr(program,info.key,"key"),needle_expr(program,info.key,"key")).unwrap();
        if !copies(program, info.key) && matches!(info.key, Ty::Record(_)) {
            writeln!(out,"    if (previous) {{ entry->key=previous->key;if (atomic_fetch_add_explicit(&entry->key->references,1,memory_order_relaxed)==SIZE_MAX) k_fatal(\"Map key reference count overflow\"); k_{}_drop(&key); }} else {{ entry->key=k_small_alloc(sizeof *entry->key);atomic_init(&entry->key->references,1);entry->key->value=key; }}",suffix(info.key)).unwrap();
        } else {
            let previous = copy_expr(program, info.key, "previous->key");
            writeln!(out, "    if (previous) {{ entry->key={previous};").unwrap();
            if info.key.owns() {
                writeln!(out, "k_{}_drop(&key);", suffix(info.key)).unwrap();
            }
            writeln!(out, "    }} else entry->key=key;").unwrap();
        }
        if info.style == MapStyle::Map {
            writeln!(
                out,
                "entry->item=(KRecord_{}){{.f0={},.f1=entry->value}};",
                info.item,
                key_expr(program, info, "entry")
            )
            .unwrap();
        }
        if info.style == MapStyle::SortedSet {
            writeln!(out,"size_t low=0,high=data->length;while(low<high) {{size_t mid=low+(high-low)/2;if(k_map_{id}_compare(data->ordered[mid],{})<0) low=mid+1;else high=mid;}}memmove(data->ordered+low+1,data->ordered+low,(data->length-low)*sizeof *data->ordered);data->ordered[low]=entry;",needle_expr(program,info.key,&key_expr(program,info,"entry"))).unwrap();
        }
        if flat(info) {
            writeln!(out,"if(!previous) {{data->buckets[index]=K_MAP_TAG(hash)|(uint64_t)(data->length+1);++data->length;}} }}").unwrap();
        } else {
            writeln!(out,"data->buckets[index]=entry;data->hashes[index]=hash;if(previous) k_drop_run(previous,k_map_{id}_entry_drop_task);else {{if(data->length>=INT64_MAX) k_fatal(\"Map length exceeds Int\");++data->length;}} }}").unwrap();
        }
        writeln!(out,"static KRecord_{id} k_map_{id}_put(KRecord_{id} owned,{key} key,{value} value) {{k_map_{id}_put_owned(&owned,key,value);return owned;}}").unwrap();
        writeln!(out,"static void k_map_{id}_put_mut(KRecord_{id} *map,{key} key,{value} value) {{k_map_{id}_put_owned(map,key,value);}}").unwrap();
        writeln!(out,"static KRecord_{id} k_map_{id}_new(size_t count,{key} *keys,{value} *values) {{KRecord_{id} map={{0}};for(size_t i=0;i<count;++i) k_map_{id}_put_owned(&map,keys[i],values[i]);return map;}}").unwrap();
        writeln!(out,"static bool k_map_{id}_contains(KRecord_{id} map,{needle} key) {{if(!map.handle) return false;size_t index=k_map_{id}_find(map.handle,k_map_{id}_hash(key),key);return map.handle->buckets[index]!=0;}}").unwrap();
        // remove drops one entry and closes the gap with backward-shift deletion, so
        // lookups keep working without tombstones (Q164). A flat map moves its last
        // entry into the freed slot, which changes the iteration order.
        if info.style != MapStyle::SortedSet {
            if flat(info) {
                writeln!(out,"static __attribute__((unused)) bool k_map_{id}_remove(KRecord_{id} *map,{needle} key) {{if(!map->handle) return false;k_map_{id}_unique(map);struct KMapData_{id} *data=map->handle;size_t mask=data->capacity-1,hole=k_map_{id}_find(data,k_map_{id}_hash(key),key);uint64_t bucket=data->buckets[hole];if(!bucket) return false;size_t slot=K_MAP_SLOT(bucket)-1,last=data->length-1;if(slot!=last) {{data->entries[slot]=data->entries[last];size_t at=(size_t)k_map_{id}_hash(data->entries[slot].key)&mask;while(K_MAP_SLOT(data->buckets[at])!=last+1) at=(at+1)&mask;data->buckets[at]=(data->buckets[at]&~UINT64_C(0xffffffff))|(uint64_t)(slot+1);}}data->buckets[hole]=0;for(size_t next=(hole+1)&mask;data->buckets[next];next=(next+1)&mask) {{size_t home=(size_t)k_map_{id}_hash(data->entries[K_MAP_SLOT(data->buckets[next])-1].key)&mask;bool moves=hole<=next ? (home<=hole || home>next) : (home<=hole && home>next);if(moves) {{data->buckets[hole]=data->buckets[next];data->buckets[next]=0;hole=next;}}}}--data->length;return true;}}").unwrap();
            } else {
                writeln!(out,"static __attribute__((unused)) bool k_map_{id}_remove(KRecord_{id} *map,{needle} key) {{if(!map->handle) return false;k_map_{id}_unique(map);struct KMapData_{id} *data=map->handle;size_t mask=data->capacity-1,hole=k_map_{id}_find(data,k_map_{id}_hash(key),key);KMapEntry_{id} *entry=data->buckets[hole];if(!entry) return false;data->buckets[hole]=NULL;for(size_t next=(hole+1)&mask;data->buckets[next];next=(next+1)&mask) {{size_t home=(size_t)data->hashes[next]&mask;bool moves=hole<=next ? (home<=hole || home>next) : (home<=hole && home>next);if(moves) {{data->buckets[hole]=data->buckets[next];data->hashes[hole]=data->hashes[next];data->buckets[next]=NULL;hole=next;}}}}--data->length;k_drop_run(entry,k_map_{id}_entry_drop_task);return true;}}").unwrap();
            }
            writeln!(out,"static __attribute__((unused)) KRecord_{id} k_map_{id}_removed(KRecord_{id} owned,{needle} key) {{k_map_{id}_remove(&owned,key);return owned;}}").unwrap();
        }
        if info.style != MapStyle::Map
            && !copies(program, info.key)
            && matches!(info.key, Ty::Record(_))
        {
            for (list, layout) in program.layouts.iter().enumerate() {
                if !matches!(layout.kind,LayoutKind::List(element) if element==info.key) {
                    continue;
                }
                let entry = if info.style == MapStyle::SortedSet {
                    "source.handle->ordered[index]"
                } else {
                    "source.handle->buckets[index]"
                };
                writeln!(out,"static void k_list_{list}_append_set_{id}(KRecord_{list} *list,KRecord_{id} source,int64_t index) {{KMapEntry_{id} *entry={entry};if(atomic_fetch_add_explicit(&entry->key->references,1,memory_order_relaxed)==SIZE_MAX) k_fatal(\"Set key reference count overflow\");k_list_{list}_append_view(list,entry->key->value,entry->key,k_map_{id}_key_drop_task);}}").unwrap();
            }
        }
        let result_ty = if !copies(program, info.value) {
            match info.value {
                Ty::Record(value) | Ty::Boxed(value) => Ty::Ref(value),
                Ty::DynBox(value) => Ty::Dyn(value),
                other => other,
            }
        } else {
            info.value
        };
        for (option, l) in program.layouts.iter().enumerate() {
            let (LayoutKind::Enum(cases) | LayoutKind::Step(cases)) = &l.kind else {
                continue;
            };
            if !matches!(&l.source,Type::Named(name,_) if name=="Option")
                || cases[0].fields != [result_ty]
            {
                continue;
            }
            let value = if matches!(result_ty, Ty::Ref(_)) && matches!(info.value, Ty::Record(_)) {
                "&entry->value".into()
            } else if info.value == Ty::Python {
                "k_python_copy(entry->value)".into()
            } else {
                copy_expr(program, info.value, "entry->value")
            };
            let lookup = if flat(info) {
                format!("k_map_{id}_entry(map.handle,index)")
            } else {
                "map.handle->buckets[index]".into()
            };
            writeln!(out,"static KRecord_{option} k_map_{id}_get_{option}(KRecord_{id} map,{needle} key) {{if(!map.handle) return k_aggregate_{option}_1();size_t index=k_map_{id}_find(map.handle,k_map_{id}_hash(key),key);KMapEntry_{id} *entry={lookup};if(!entry) return k_aggregate_{option}_1();return k_aggregate_{option}_0({value});}}").unwrap();
        }
    }
}

/// Numeric entries have no independent owners. Copy the table on sharing and
/// store its entries inline, preserving the normal bucket access for consumers.
fn emit_flat_storage(id: usize, out: &mut String) {
    // Entries fill a packed array that holds at most K_MAP_ROOM(capacity) items, the
    // growth threshold, so no slot past three quarters of the buckets is allocated.
    writeln!(out, "static struct KMapData_{id} *k_map_{id}_allocate(size_t capacity) {{\n    if(capacity>K_MAP_MAX_CAPACITY) k_fatal(\"Map is too large\");\n    struct KMapData_{id} *data=calloc(1,sizeof *data);if(!data) k_fatal(\"Out of memory\");\n    atomic_init(&data->references,1);data->capacity=capacity;data->entries=malloc(K_MAP_ROOM(capacity)*sizeof *data->entries);data->buckets=calloc(capacity,sizeof *data->buckets);\n    if(!data->entries || !data->buckets) k_fatal(\"Out of memory\");return data;\n}}").unwrap();
    writeln!(out, "static void k_map_{id}_unique(KRecord_{id} *map) {{\n    struct KMapData_{id} *data=map->handle;if(!data) {{map->handle=k_map_{id}_allocate(8);return;}}\n    if(atomic_load_explicit(&data->references,memory_order_acquire)==1) return;\n    struct KMapData_{id} *copy=k_map_{id}_allocate(data->capacity);copy->length=data->length;\n    memcpy(copy->entries,data->entries,data->length*sizeof *copy->entries);memcpy(copy->buckets,data->buckets,data->capacity*sizeof *copy->buckets);\n    KRecord_{id} old=*map;map->handle=copy;k_record_{id}_drop(&old);\n}}").unwrap();
    writeln!(out, "static void k_map_{id}_grow(struct KMapData_{id} *data) {{\n    if(data->capacity>K_MAP_MAX_CAPACITY/2) k_fatal(\"Map is too large\");\n    size_t capacity=data->capacity*2,mask=capacity-1;\n    KMapEntry_{id} *entries=realloc(data->entries,K_MAP_ROOM(capacity)*sizeof *entries);uint64_t *buckets=calloc(capacity,sizeof *buckets);\n    if(!entries || !buckets) k_fatal(\"Out of memory\");\n    for(size_t i=0;i<data->length;++i) {{uint64_t hash=k_map_{id}_hash(entries[i].key);size_t index=(size_t)hash&mask;while(buckets[index]) index=(index+1)&mask;buckets[index]=K_MAP_TAG(hash)|(uint64_t)(i+1);}}\n    free(data->buckets);data->entries=entries;data->buckets=buckets;data->capacity=capacity;\n}}").unwrap();
}
