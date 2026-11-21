//! C11 emission and native compilation; generated values retain their concrete types.
use crate::{
    core::{
        Atom, Cleanup, Function, LayoutKind, Location, Program, Rhs, Slot, Statement, Test, Ty,
    },
    types::{Passing, Type},
};
#[path = "native_collections.rs"]
mod collections;
#[path = "native_maps.rs"]
mod maps;
#[path = "native_moves.rs"]
mod moves;
#[path = "native_python.rs"]
mod python;
#[path = "native_resources.rs"]
mod resources;
#[path = "native_session.rs"]
pub mod session;
use std::{
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

/// The C compiler Kestrel drives: `KESTREL_CC` when set, else the Clang shipped beside
/// the compiler (the Windows install bundles one), else `clang` on `PATH`.
pub fn c_compiler() -> &'static Path {
    static COMPILER: OnceLock<PathBuf> = OnceLock::new();
    COMPILER.get_or_init(|| {
        std::env::var_os("KESTREL_CC")
            .filter(|name| !name.is_empty())
            .map(PathBuf::from)
            .or_else(|| crate::paths::bundled("clang"))
            .unwrap_or_else(|| PathBuf::from("clang"))
    })
}

/// A command that runs the C compiler.
pub fn clang_command() -> Command {
    Command::new(c_compiler())
}

/// Whether Clang targets the MinGW ABI, as the bundled Windows toolchain does, rather
/// than MSVC. The two take different linker flags and C runtimes.
pub fn mingw() -> bool {
    static MINGW: OnceLock<bool> = OnceLock::new();
    *MINGW.get_or_init(|| {
        cfg!(windows)
            && clang_command()
                .arg("-dumpmachine")
                .output()
                .is_ok_and(|output| {
                    let target = String::from_utf8_lossy(&output.stdout);
                    target.contains("mingw") || target.contains("windows-gnu")
                })
    })
}

fn bigint_constant(text: &str) -> String {
    let spelling = text
        .chars()
        .filter(|c| *c != '_' && !c.is_ascii_whitespace())
        .collect::<String>();
    let negative = spelling.starts_with('-');
    let digits = spelling
        .strip_prefix('-')
        .or_else(|| spelling.strip_prefix('+'))
        .unwrap_or(&spelling);
    let (base, digits) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
        .map_or((10, digits), |digits| (16, digits));
    let mut words = vec![];
    for digit in digits.chars() {
        let mut carry = u64::from(digit.to_digit(base).expect("checked integer literal"));
        for word in &mut words {
            let product = u64::from(*word) * u64::from(base) + carry;
            *word = product as u32;
            carry = product >> 32;
        }
        if carry != 0 {
            words.push(carry as u32);
        }
    }
    let length = words.len();
    let sign = if length == 0 {
        0
    } else if negative {
        -1
    } else {
        1
    };
    let words = if words.is_empty() {
        "0".into()
    } else {
        words
            .iter()
            .map(|word| format!("UINT32_C({word})"))
            .collect::<Vec<_>>()
            .join(",")
    };
    format!("&(KBigInt){{(uint32_t[]){{{words}}},{length},{sign}}}")
}

// A two-case enum whose first case is empty and whose second starts with a
// Box can store its tag in that pointer: an empty case is a null pointer and
// a payload case is non-null. This is the Rust niche layout for `Leaf | Node`.
fn enum_niche(layout: &crate::core::Layout) -> Option<usize> {
    let LayoutKind::Enum(cases) = &layout.kind else {
        return None;
    };
    if cases.len() != 2 || !cases[0].fields.is_empty() || cases[1].fields.is_empty() {
        return None;
    }
    if !matches!(cases[1].fields[0], Ty::Boxed(_)) {
        return None;
    }
    Some(1)
}

fn c_type(ty: Ty) -> String {
    match ty {
        Ty::Int => "int64_t".into(),
        Ty::Nat => "uint64_t".into(),
        Ty::Double => "double".into(),
        Ty::Bool => "bool".into(),
        Ty::String => "KString".into(),
        Ty::Unit => "void".into(),
        Ty::Error => "KError".into(),
        Ty::Tasks => "KTaskScope".into(),
        Ty::Record(id) => format!("KRecord_{id}"),
        Ty::Ref(id) | Ty::Boxed(id) => format!("KRecord_{id} *"),
        Ty::Dyn(id) | Ty::DynBox(id) => format!("KDyn_{id}"),
        Ty::C(index) => [
            "int8_t", "int16_t", "int32_t", "int64_t", "uint8_t", "uint16_t", "uint32_t",
            "uint64_t", "float", "char",
        ][index]
            .into(),
        Ty::Python => "KPython".into(),
        Ty::Owner => "KOwner *".into(),
        Ty::Pointer(constant) => if constant { "const void *" } else { "void *" }.into(),
        Ty::ArenaRef(_) => "KArenaRef".into(),
        Ty::Variable => "KVariable".into(),
    }
}
fn suffix(ty: Ty) -> String {
    match ty {
        Ty::Int => "int".into(),
        Ty::Nat => "nat".into(),
        Ty::Double => "double".into(),
        Ty::Bool => "bool".into(),
        Ty::String => "string".into(),
        Ty::Unit => "unit".into(),
        Ty::Error => "error".into(),
        Ty::Tasks => "tasks".into(),
        Ty::Record(id) => format!("record_{id}"),
        Ty::Ref(id) => format!("ref_{id}"),
        Ty::Boxed(id) => format!("box_{id}"),
        Ty::Dyn(id) => format!("dyn_{id}"),
        Ty::DynBox(id) => format!("dynbox_{id}"),
        Ty::C(index) => format!("c{index}"),
        Ty::Python => "python".into(),
        Ty::Owner => "owner".into(),
        Ty::Pointer(constant) => if constant {
            "cconstpointer"
        } else {
            "cpointer"
        }
        .into(),
        Ty::ArenaRef(_) => "arenaref".into(),
        Ty::Variable => "variable".into(),
    }
}
/// C text for a parameter declared with this layout and passing mode.
fn parameter_type(ty: Ty, mode: Passing) -> String {
    if mode == Passing::Mut && !matches!(ty, Ty::Ref(_)) {
        format!("{} *", c_type(ty))
    } else {
        c_type(ty)
    }
}
fn copies(program: &Program, ty: Ty) -> bool {
    matches!(ty, Ty::Record(id) | Ty::Boxed(id) if program.copyable_layouts.contains(&id))
        || matches!(ty, Ty::Record(id) if matches!(program.layouts[id].kind, LayoutKind::Closure { .. }))
        || matches!(ty, Ty::String | Ty::Error | Ty::Owner | Ty::Variable)
        || matches!(ty,Ty::Record(id) if matches!(program.layouts[id].kind,LayoutKind::List(_) | LayoutKind::Map(crate::core::MapLayout {mutable:false,..})))
}
fn initial(ty: Ty) -> &'static str {
    match ty {
        Ty::String => "{ \"\", 0, NULL }",
        Ty::Variable => "{0}",
        Ty::Record(_)
        | Ty::Error
        | Ty::Tasks
        | Ty::Dyn(_)
        | Ty::DynBox(_)
        | Ty::Python
        | Ty::ArenaRef(_) => "{0}",
        _ => "0",
    }
}
fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for byte in text.bytes() {
        match byte {
            b'\"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            // `??)` and its kin are C trigraphs, so a question mark is always escaped.
            b'?' => out.push_str("\\?"),
            32..=126 => out.push(byte as char),
            _ => {
                write!(out, "\\{byte:03o}").unwrap();
            }
        }
    }
    out.push('"');
    out
}
/// Whether a String method exists with this many arguments, counting the receiver (Q136).
fn string_method_name(name: &str, arity: usize) -> bool {
    match name {
        "isalpha" | "isdecimal" | "isdigit" | "isnumeric" | "isalnum" | "isspace" | "isupper"
        | "islower" | "istitle" | "isascii" | "isprintable" | "isidentifier" | "capitalize"
        | "title" | "swapcase" | "casefold" | "lower" | "upper" | "splitlines" => arity == 1,
        "zfill" | "removeprefix" | "removesuffix" | "find" | "rfind" | "count" | "index"
        | "rindex" | "partition" | "rpartition" => arity == 2,
        "replace" | "substring" | "slice" => arity == 3,
        "reversed" | "ord" | "byteLength" | "lines" | "words" => arity == 1,
        "byteAt" => arity == 2,
        "byteSlice" => arity == 3,
        "center" | "ljust" | "rjust" | "padStart" | "padEnd" => arity == 2 || arity == 3,
        "strip" | "lstrip" | "rstrip" | "expandtabs" | "split" => arity == 1 || arity == 2,
        _ => false,
    }
}

/// The C call for a String method. Methods with two forms are told apart by their argument count (Q136).
fn string_method(name: &str, args: &[String], ty: Ty, at: &str) -> String {
    const EMPTY: &str = "(KString) { \"\", 0, NULL }";
    const SPACE: &str = "(KString) { \" \", 1, NULL }";
    let arg = |index: usize| args[index].as_str();
    match (name, args.len()) {
        ("isalpha", 1) => format!("k_string_isalpha({})", arg(0)),
        ("isdecimal", 1) => format!("k_string_isdecimal({})", arg(0)),
        ("isdigit", 1) => format!("k_string_isdigit({})", arg(0)),
        ("isnumeric", 1) => format!("k_string_isnumeric({})", arg(0)),
        ("isalnum", 1) => format!("k_string_isalnum({})", arg(0)),
        ("isspace", 1) => format!("k_string_isspace({})", arg(0)),
        ("isupper", 1) => format!("k_string_isupper({})", arg(0)),
        ("islower", 1) => format!("k_string_islower({})", arg(0)),
        ("istitle", 1) => format!("k_string_istitle({})", arg(0)),
        ("isascii", 1) => format!("k_string_isascii({})", arg(0)),
        ("isprintable", 1) => format!("k_string_isprintable({})", arg(0)),
        ("isidentifier", 1) => format!("k_string_isidentifier({})", arg(0)),
        ("capitalize", 1) => format!("k_string_capitalize({})", arg(0)),
        ("title", 1) => format!("k_string_title({})", arg(0)),
        ("swapcase", 1) => format!("k_string_swapcase({})", arg(0)),
        ("casefold", 1) => format!("k_string_casefold({})", arg(0)),
        ("lower", 1) => format!("k_string_case({}, false)", arg(0)),
        ("upper", 1) => format!("k_string_case({}, true)", arg(0)),
        ("center", 2) => format!("k_string_pad({}, {}, {SPACE}, 2, {at})", arg(0), arg(1)),
        ("center", 3) => format!("k_string_pad({}, {}, {}, 2, {at})", arg(0), arg(1), arg(2)),
        ("ljust", 2) => format!("k_string_pad({}, {}, {SPACE}, 0, {at})", arg(0), arg(1)),
        ("ljust", 3) => format!("k_string_pad({}, {}, {}, 0, {at})", arg(0), arg(1), arg(2)),
        ("rjust", 2) => format!("k_string_pad({}, {}, {SPACE}, 1, {at})", arg(0), arg(1)),
        ("rjust", 3) => format!("k_string_pad({}, {}, {}, 1, {at})", arg(0), arg(1), arg(2)),
        ("zfill", 2) => format!("k_string_zfill({}, {})", arg(0), arg(1)),
        ("substring" | "slice", 3) => {
            format!("k_string_substring({}, {}, {})", arg(0), arg(1), arg(2))
        }
        ("reversed", 1) => format!("k_string_reversed({})", arg(0)),
        ("ord", 1) => format!("k_string_ord({}, {at})", arg(0)),
        ("byteLength", 1) => format!("((int64_t){}.length)", arg(0)),
        ("byteAt", 2) => format!("k_string_byte_at({}, {}, {at})", arg(0), arg(1)),
        ("byteSlice", 3) => format!(
            "k_string_byte_slice({}, {}, {}, {at})",
            arg(0),
            arg(1),
            arg(2)
        ),
        ("lines", 1) => string_method("splitlines", args, ty, at),
        ("words", 1) => string_method("split", args, ty, at),
        ("padStart", 2) => string_method("rjust", args, ty, at),
        ("padStart", 3) => string_method("rjust", args, ty, at),
        ("padEnd", 2) => string_method("ljust", args, ty, at),
        ("padEnd", 3) => string_method("ljust", args, ty, at),
        ("strip", 1) => format!("k_string_strip({}, {EMPTY}, false, true, true)", arg(0)),
        ("strip", 2) => format!("k_string_strip({}, {}, true, true, true)", arg(0), arg(1)),
        ("lstrip", 1) => format!("k_string_strip({}, {EMPTY}, false, true, false)", arg(0)),
        ("lstrip", 2) => format!("k_string_strip({}, {}, true, true, false)", arg(0), arg(1)),
        ("rstrip", 1) => format!("k_string_strip({}, {EMPTY}, false, false, true)", arg(0)),
        ("rstrip", 2) => format!("k_string_strip({}, {}, true, false, true)", arg(0), arg(1)),
        ("expandtabs", 1) => format!("k_string_expandtabs({}, 8)", arg(0)),
        ("expandtabs", 2) => format!("k_string_expandtabs({}, {})", arg(0), arg(1)),
        ("removeprefix", 2) => format!("k_string_removeprefix({}, {})", arg(0), arg(1)),
        ("removesuffix", 2) => format!("k_string_removesuffix({}, {})", arg(0), arg(1)),
        ("find", 2) => format!("k_string_find({}, {}, false)", arg(0), arg(1)),
        ("rfind", 2) => format!("k_string_find({}, {}, true)", arg(0), arg(1)),
        ("count", 2) => format!("k_string_count({}, {})", arg(0), arg(1)),
        ("index", 2) | ("rindex", 2) => {
            let Ty::Record(option) = ty else {
                unreachable!()
            };
            let reverse = name == "rindex";
            format!(
                "(({{ int64_t found_ = k_string_find({}, {}, {reverse}); found_ < 0 ? k_aggregate_{option}_1() : k_aggregate_{option}_0(found_); }}))",
                arg(0),
                arg(1)
            )
        }
        ("replace", 3) => format!("k_string_replace({}, {}, {})", arg(0), arg(1), arg(2)),
        ("partition", 2) | ("rpartition", 2) => {
            let Ty::Record(tuple) = ty else {
                unreachable!()
            };
            let reverse = name == "rpartition";
            format!(
                "(({{ KString part0_, part1_, part2_; k_string_partition({}, {}, {reverse}, &part0_, &part1_, &part2_); k_aggregate_{tuple}_tuple(part0_, part1_, part2_); }}))",
                arg(0),
                arg(1)
            )
        }
        ("splitlines", 1) => {
            let Ty::Record(id) = ty else { unreachable!() };
            format!("k_list_{id}_split_lines({})", arg(0))
        }
        ("split", 1) => {
            let Ty::Record(id) = ty else { unreachable!() };
            format!("k_list_{id}_split_space({})", arg(0))
        }
        ("split", 2) => {
            let Ty::Record(id) = ty else { unreachable!() };
            format!("k_list_{id}_split({}, {})", arg(0), arg(1))
        }
        _ => unreachable!("string method {name} with {} arguments", args.len()),
    }
}

fn location(loc: &Location) -> String {
    format!(
        "(KLocation) {{ {}, {}, {}, {} }}",
        quoted(&loc.file),
        loc.line,
        loc.column,
        quoted(&loc.expression)
    )
}
struct Emitter<'a> {
    program: &'a Program,
    function: &'a Function,
    /// The current function's index, to find its tail-call group.
    index: usize,
    /// Temporaries a new value takes over instead of copying.
    consumable: std::collections::BTreeSet<Slot>,
    output: String,
}
impl Emitter<'_> {
    fn ty(&self, slot: Slot) -> Ty {
        if slot.global {
            self.program.globals[slot.id]
        } else {
            self.function.locals[slot.id].ty
        }
    }
    fn slot(&self, slot: Slot) -> String {
        if slot.global {
            return format!("g{}", slot.id);
        }
        if self
            .function
            .parameters
            .iter()
            .any(|(p, _, mode)| *p == slot && *mode == Passing::Mut)
        {
            format!("(*v{})", slot.id)
        } else {
            format!("v{}", slot.id)
        }
    }
    fn atom_type(&self, atom: &Atom) -> Ty {
        match atom {
            Atom::Nat(_) | Atom::NatPred(_) => Ty::Nat,
            Atom::Slot(slot) => self.ty(*slot),
            Atom::Field(_, _, ty)
            | Atom::Payload(_, _, _, ty)
            | Atom::ResourceValue(_, ty)
            | Atom::ListItem(_, _, ty)
            | Atom::ArrayItem(_, _, ty)
            | Atom::SetItem(_, _, ty, _)
            | Atom::EnvField(_, _, ty, _) => *ty,
            Atom::Tag(_)
            | Atom::ErrorType(_)
            | Atom::CollectionLength(_)
            | Atom::LengthLess(..)
            | Atom::ArrayLength(_)
            | Atom::MapCapacity(_) => Ty::Int,
            Atom::ErrorPayload(_, id) => Ty::Ref(*id),
            Atom::Address(_, id) => Ty::Ref(*id),
            Atom::Int(_) => Ty::Int,
            Atom::Double(_) => Ty::Double,
            Atom::Bool(_) => Ty::Bool,
            Atom::Text(_) => Ty::String,
            Atom::MapOccupied(..) => Ty::Bool,
            Atom::MapItem(_, _, id) => Ty::Ref(*id),
            Atom::StepOwner(_) => Ty::Owner,
            Atom::OwnerValue(_, _, view) => *view,
            Atom::Unit => Ty::Unit,
        }
    }
    fn take(&self, atom: &Atom, ty: Ty) -> String {
        if ty.owns() {
            format!("k_{}_take(&{})", suffix(ty), self.atom(atom))
        } else {
            self.atom(atom)
        }
    }
    /// A record's member access prefix: `(v).` for a value, `(v)->` through a pointer.
    fn through(&self, owner: &Atom) -> String {
        let arrow = matches!(self.atom_type(owner), Ty::Ref(_) | Ty::Boxed(_));
        format!("({}){}", self.atom(owner), if arrow { "->" } else { "." })
    }
    fn atom(&self, atom: &Atom) -> String {
        match atom {
            Atom::Nat(n) => format!("UINT64_C({n})"),
            Atom::NatPred(inner) => format!("({} - 1)", self.atom(inner)),
            Atom::StepOwner(step) => format!("{}owner", self.through(step)),
            Atom::OwnerValue(owner, stored, view) => {
                let pointer = format!("(({} *)({})->bytes)", c_type(*stored), self.atom(owner));
                if matches!((stored, view), (Ty::Record(a), Ty::Ref(b)) if a == b) {
                    pointer
                } else {
                    format!("(*{pointer})")
                }
            }
            Atom::EnvField(env, index, _, by_ref) => {
                let field = format!("({})->f{index}", self.atom(env));
                if *by_ref {
                    format!("(*{field})")
                } else {
                    field
                }
            }
            Atom::SetItem(owner, index, ty, sorted) => {
                // A MutSet is reached through a pointer (REVIEW2 section 2).
                let entry = format!(
                    "{}handle->{}[{}]",
                    self.through(owner),
                    if *sorted { "ordered" } else { "buckets" },
                    self.atom(index)
                );
                if !copies(self.program, *ty) && matches!(ty, Ty::Record(_)) {
                    format!("({entry})->key->value")
                } else {
                    format!("({entry})->key")
                }
            }
            Atom::MapCapacity(owner) => {
                let field = if self.flat_map(owner) {
                    "length"
                } else {
                    "capacity"
                };
                let prefix = self.through(owner);
                format!("({prefix}handle ? (int64_t){prefix}handle->{field} : 0)")
            }
            Atom::MapOccupied(owner, index) if self.flat_map(owner) => format!(
                "((size_t){} < {}handle->length)",
                self.atom(index),
                self.through(owner)
            ),
            Atom::MapOccupied(owner, index) => format!(
                "({}handle->buckets[{}] != NULL)",
                self.through(owner),
                self.atom(index)
            ),
            Atom::MapItem(owner, index, _) if self.flat_map(owner) => format!(
                "(&{}handle->entries[{}].item)",
                self.through(owner),
                self.atom(index)
            ),
            Atom::MapItem(owner, index, _) => format!(
                "(&{}handle->buckets[{}]->item)",
                self.through(owner),
                self.atom(index)
            ),
            Atom::CollectionLength(owner) => format!(
                "({}handle ? (int64_t){}handle->length : 0)",
                self.through(owner),
                self.through(owner)
            ),
            Atom::LengthLess(owner, less) => format!(
                "({}handle ? (int64_t){}handle->length - {less} : 0)",
                self.through(owner),
                self.through(owner)
            ),
            Atom::ListItem(owner, index, _) => {
                let scalar_list = match self.atom_type(owner) {
                    Ty::Record(id) | Ty::Ref(id) | Ty::Boxed(id) => matches!(
                        &self.program.layouts[id].kind,
                        LayoutKind::List(element)
                            if matches!(element, Ty::Int | Ty::Nat | Ty::Double | Ty::Bool)
                    ),
                    _ => false,
                };
                if scalar_list {
                    format!(
                        "({}).handle->storage[{}].value",
                        self.atom(owner),
                        self.atom(index)
                    )
                } else {
                    format!(
                        "({}).handle->items[{}]->value",
                        self.atom(owner),
                        self.atom(index)
                    )
                }
            }
            Atom::ResourceValue(owner, _) => format!("({})->handle->value", self.atom(owner)),
            Atom::ArrayItem(owner, index, _) => {
                format!("{}items[{}]", self.through(owner), self.atom(index))
            }
            Atom::ArrayLength(owner) => format!("{}length", self.through(owner)),
            Atom::Slot(slot) => self.slot(*slot),
            Atom::Int(value) if *value == i64::MIN => "INT64_MIN".into(),
            Atom::Int(value) if *value < 0 => format!("(-INT64_C({}))", -*value),
            Atom::Int(value) => format!("INT64_C({value})"),
            Atom::Double(value) if value.is_nan() => "NAN".into(),
            Atom::Double(value) if *value == f64::INFINITY => "INFINITY".into(),
            Atom::Double(value) if *value == f64::NEG_INFINITY => "(-INFINITY)".into(),
            Atom::Double(value) => format!("{value:?}"),
            Atom::Bool(value) => value.to_string(),
            Atom::Text(value) => {
                format!("(KString) {{ {}, {}, NULL }}", quoted(value), value.len())
            }
            Atom::Unit => "((void)0)".into(),
            Atom::Address(value, _) => {
                if matches!(self.atom_type(value), Ty::Ref(_) | Ty::Boxed(_)) {
                    self.atom(value)
                } else {
                    format!("(&{})", self.atom(value))
                }
            }
            Atom::ErrorType(error) => format!("({}).type", self.atom(error)),
            Atom::ErrorPayload(error, id) => {
                format!("((KRecord_{id} *)({}).payload)", self.atom(error))
            }
            Atom::Tag(owner) => {
                let arrow = if matches!(self.atom_type(owner), Ty::Ref(_) | Ty::Boxed(_)) {
                    "->"
                } else {
                    "."
                };
                if let Ty::Record(id) | Ty::Ref(id) | Ty::Boxed(id) = self.atom_type(owner)
                    && enum_niche(&self.program.layouts[id]).is_some()
                {
                    format!("(({}){}f0 ? 2u : 1u)", self.atom(owner), arrow)
                } else {
                    format!("({}){}tag", self.atom(owner), arrow)
                }
            }
            Atom::Payload(owner, case, index, _) => {
                let arrow = if matches!(self.atom_type(owner), Ty::Ref(_) | Ty::Boxed(_)) {
                    "->"
                } else {
                    "."
                };
                if let Ty::Record(id) | Ty::Ref(id) | Ty::Boxed(id) = self.atom_type(owner)
                    && enum_niche(&self.program.layouts[id]) == Some(*case)
                {
                    format!("({}){}f{index}", self.atom(owner), arrow)
                } else {
                    format!("({}){}data.c{case}.f{index}", self.atom(owner), arrow)
                }
            }
            Atom::Field(owner, index, _) if matches!(self.atom_type(owner),Ty::Record(id) | Ty::Ref(id) if matches!(self.program.layouts[id].kind,LayoutKind::ClassView(_))) =>
            {
                let (Ty::Record(id) | Ty::Ref(id)) = self.atom_type(owner) else {
                    unreachable!()
                };
                let LayoutKind::ClassView(cases) = &self.program.layouts[id].kind else {
                    unreachable!()
                };
                let mut pointers = vec![];
                let field = &self.program.layouts[id].fields[*index].name;
                for (case, info) in cases.iter().enumerate() {
                    let Ty::Ref(actual) = info.fields[0] else {
                        unreachable!()
                    };
                    let actual_index = self.program.layouts[actual]
                        .fields
                        .iter()
                        .position(|f| f.name == *field)
                        .unwrap();
                    pointers.push(format!(
                        "&(({}){}data.c{case}.f0->f{actual_index})",
                        self.atom(owner),
                        if matches!(self.atom_type(owner), Ty::Ref(_)) {
                            "->"
                        } else {
                            "."
                        }
                    ));
                }
                let mut pointer = pointers.pop().unwrap();
                for (case, field) in pointers.into_iter().enumerate().rev() {
                    pointer = format!(
                        "(({}){}tag == {} ? {field} : {pointer})",
                        self.atom(owner),
                        if matches!(self.atom_type(owner), Ty::Ref(_)) {
                            "->"
                        } else {
                            "."
                        },
                        case + 1
                    );
                }
                format!("*({pointer})")
            }
            Atom::Field(owner, index, _) => format!(
                "({}){}f{index}",
                self.atom(owner),
                if matches!(self.atom_type(owner), Ty::Ref(_) | Ty::Boxed(_)) {
                    "->"
                } else {
                    "."
                }
            ),
        }
    }
    fn flat_map(&self, owner: &Atom) -> bool {
        match self.atom_type(owner) {
            Ty::Record(id) | Ty::Ref(id) => {
                matches!(self.program.layouts[id].kind,LayoutKind::Map(info) if maps::flat(info))
            }
            _ => false,
        }
    }
    fn bigint_pointer(&self, atom: &Atom) -> String {
        let value = self.atom(atom);
        if matches!(self.atom_type(atom), Ty::Ref(_) | Ty::Boxed(_)) {
            format!("&(({value})->value)")
        } else {
            format!("&(({value}).value)")
        }
    }
    fn test(&self, test: &Test) -> String {
        match test {
            Test::Yes => "true".into(),
            Test::Equal(ty, a, b) => self.equal(*ty, a, b),
            Test::Less(_, a, b) => format!("({} < {})", self.atom(a), self.atom(b)),
            Test::BigIntLiteral(value, text) => format!(
                "(k_bigint_compare({}, {}) == 0)",
                self.bigint_pointer(value),
                bigint_constant(text)
            ),
            Test::ListLength(value, count, exact) => format!(
                "({} {} {count})",
                self.atom(&Atom::CollectionLength(Box::new(value.clone()))),
                if *exact { "==" } else { ">=" },
            ),
            Test::And(a, b) => format!("({} && {})", self.test(a), self.test(b)),
            Test::Or(a, b) => format!("({} || {})", self.test(a), self.test(b)),
            Test::Not(a) => format!("(!{})", self.test(a)),
        }
    }
    fn equal(&self, ty: Ty, left: &Atom, right: &Atom) -> String {
        let (a, b) = (self.atom(left), self.atom(right));
        if ty == Ty::String {
            format!("k_string_equal({a}, {b})")
        } else if matches!(ty, Ty::ArenaRef(_)) {
            format!("k_arenaref_equal({a}, {b})")
        } else {
            format!("({a} == {b})")
        }
    }
    /// The success and Error case positions of a `T or Error` result, and the success case's field.
    fn fallible_cases(&self, ty: Ty) -> Result<(usize, usize, usize, Option<Ty>), String> {
        let Ty::Record(id) = ty else {
            return Err(format!(
                "A fallible system call needs a union result, found {ty:?}"
            ));
        };
        let LayoutKind::Union(parts, cases) = &self.program.layouts[id].kind else {
            return Err("A fallible system call needs a union result".into());
        };
        let failure = parts
            .iter()
            .position(|part| *part == Type::named("Error"))
            .ok_or("A fallible system call needs an Error case")?;
        let success = 1 - failure;
        Ok((id, success, failure, cases[success].fields.first().copied()))
    }

    /// Calls into system_runtime.c for std.fs, std.io, std.time and std.process (REVIEW2 4).
    fn system_rhs(
        &self,
        system: &str,
        args: &[String],
        ty: Ty,
        at: &str,
    ) -> Result<String, String> {
        let arg = |index: usize| args[index].as_str();
        // A call that fills its out-parameter on success, as the result's success case.
        let fallible = |call: String, value: &str, declare: &str| -> Result<String, String> {
            let (id, success, failure, _) = self.fallible_cases(ty)?;
            Ok(format!(
                "(({{ {declare} KError error_; {call} ? k_aggregate_{id}_{success}({value}) : k_aggregate_{id}_{failure}(error_); }}))"
            ))
        };
        Ok(match system {
            "fs.readBytes" => fallible(
                format!("k_fs_read_bytes({}, &text_, &error_)", arg(0)),
                "text_",
                "KString text_;",
            )?,
            "fs.readFile" => fallible(
                format!("k_fs_read_file({}, &text_, &error_)", arg(0)),
                "text_",
                "KString text_;",
            )?,
            "fs.writeFile" | "fs.appendFile" | "fs.writeBytes" | "fs.appendBytes" => fallible(
                format!(
                    "k_fs_write_file({}, {}, {}, &error_)",
                    arg(0),
                    arg(1),
                    matches!(system, "fs.appendFile" | "fs.appendBytes")
                ),
                "",
                "",
            )?,
            "fs.exists" => format!("k_fs_exists({})", arg(0)),
            "fs.isDir" => format!("k_fs_is_dir({})", arg(0)),
            "fs.createDirs" => fallible(format!("k_fs_create_dirs({}, &error_)", arg(0)), "", "")?,
            "fs.remove" => fallible(format!("k_fs_remove({}, &error_)", arg(0)), "", "")?,
            "fs.rename" => fallible(
                format!("k_fs_rename({}, {}, &error_)", arg(0), arg(1)),
                "",
                "",
            )?,
            "fs.copy" => fallible(
                format!("k_fs_copy({}, {}, &error_)", arg(0), arg(1)),
                "",
                "",
            )?,
            "fs.tempDir" => fallible(
                "k_fs_temp_dir(&text_, &error_)".into(),
                "text_",
                "KString text_;",
            )?,
            "fs.listDir" => {
                let (id, success, failure, field) = self.fallible_cases(ty)?;
                let Some(Ty::Record(list)) = field else {
                    return Err("listDir needs a List result".into());
                };
                format!(
                    "(({{ KString *names_; size_t count_; KError error_; k_fs_list_dir({}, &names_, &count_, &error_) ? ({{ KRecord_{list} list_ = {{0}}; for (size_t i_ = 0; i_ < count_; ++i_) k_list_{list}_append(&list_, names_[i_]); free(names_); k_aggregate_{id}_{success}(list_); }}) : k_aggregate_{id}_{failure}(error_); }}))",
                    arg(0)
                )
            }
            "error.context" => format!("k_error_context({}, {})", arg(0), arg(1)),
            "error.cause" => {
                let Ty::Record(id) = ty else {
                    return Err("cause needs Option[Error]".into());
                };
                let cases = self.program.layouts[id]
                    .kind
                    .cases()
                    .ok_or("cause needs Option")?;
                let some = cases
                    .iter()
                    .position(|case| case.name == "Some")
                    .ok_or("Some")?;
                let none = cases
                    .iter()
                    .position(|case| case.name == "None")
                    .ok_or("None")?;
                format!(
                    "({{ KError e_ = {}; e_.owner && e_.owner->cause.type ? k_aggregate_{id}_{some}(k_error_copy(e_.owner->cause)) : k_aggregate_{id}_{none}(); }})",
                    arg(0)
                )
            }
            "bytes.fromUtf8" => format!("k_string_copy({})", arg(0)),
            "bytes.length" => format!("((int64_t)({}).length)", arg(0)),
            "bytes.isEmpty" => format!("(({}).length == 0)", arg(0)),
            "bytes.at" => format!(
                "({{ KString b_ = {}; int64_t i_ = {}; if (i_ < 0 || (uint64_t)i_ >= b_.length) k_trap({at}, \"Byte index out of bounds\"); (int64_t)(unsigned char)b_.data[i_]; }})",
                arg(0),
                arg(1)
            ),
            "bytes.get" => {
                let Ty::Record(id) = ty else {
                    return Err("get needs Option".into());
                };
                let cases = self.program.layouts[id]
                    .kind
                    .cases()
                    .ok_or("get needs Option")?;
                let some = cases
                    .iter()
                    .position(|case| case.name == "Some")
                    .ok_or("Some")?;
                let none = cases
                    .iter()
                    .position(|case| case.name == "None")
                    .ok_or("None")?;
                format!(
                    "({{ KString b_ = {}; int64_t i_ = {}; i_ >= 0 && (uint64_t)i_ < b_.length ? k_aggregate_{id}_{some}((int64_t)(unsigned char)b_.data[i_]) : k_aggregate_{id}_{none}(); }})",
                    arg(0),
                    arg(1)
                )
            }
            "bytes.slice" => format!("k_bytes_slice({}, {}, {})", arg(0), arg(1), arg(2)),
            "bytes.concat" => format!("k_string_join(2, (KString[]){{{}, {}}})", arg(0), arg(1)),
            "bytes.toHex" => format!("k_bytes_hex({})", arg(0)),
            "bytes.decodeUtf8" => {
                let (id, success, failure, _) = self.fallible_cases(ty)?;
                format!(
                    "({{ KString b_ = {}; k_valid_utf8(b_) ? k_aggregate_{id}_{success}(k_string_copy(b_)) : k_aggregate_{id}_{failure}(k_runtime_error(0, K_TEXT(\"Bytes are not valid UTF-8\"))); }})",
                    arg(0)
                )
            }
            "bytes.fromList" => {
                let (id, success, failure, _) = self.fallible_cases(ty)?;
                // The input List[Int] layout is the first matching layout in this program.
                let list = self
                    .program
                    .layouts
                    .iter()
                    .position(|layout| matches!(layout.kind, LayoutKind::List(Ty::Int)))
                    .ok_or("fromList needs List[Int]")?;
                format!(
                    "({{ KRecord_{list} source_ = {}; bool valid_ = true; KTextBuilder data_ = {{0}}; for (size_t i_ = 0; source_.handle && i_ < source_.handle->length; ++i_) {{ int64_t item_ = k_list_{list}_at(source_, (int64_t)i_, {at}); if (item_ < 0 || item_ > 255) {{ valid_ = false; break; }} char byte_ = (char)item_; k_text_put_bytes(&data_, (KString){{&byte_, 1, NULL}}, 0, 1); }} KString bytes_ = k_text_finish(&data_); valid_ ? k_aggregate_{id}_{success}(bytes_) : (k_string_drop(&bytes_), k_aggregate_{id}_{failure}(k_runtime_error(0, K_TEXT(\"A byte must be between 0 and 255\")))); }})",
                    arg(0)
                )
            }
            "bytes.toList" => {
                let Ty::Record(list) = ty else {
                    return Err("toList needs List[Int]".into());
                };
                format!(
                    "({{ KString bytes_ = {}; KRecord_{list} result_ = {{0}}; for (size_t i_ = 0; i_ < bytes_.length; ++i_) k_list_{list}_append(&result_, (int64_t)(unsigned char)bytes_.data[i_]); result_; }})",
                    arg(0)
                )
            }
            "bytes.nextCharBoundary" => {
                format!("k_text_next_boundary({}, {}, false, {at})", arg(0), arg(1))
            }
            "bytes.nextGraphemeBoundary" => {
                format!("k_text_next_boundary({}, {}, true, {at})", arg(0), arg(1))
            }
            "rx.check" => format!("k_rx_check({})", arg(0)),
            "rx.find" => {
                let Ty::Record(list) = ty else {
                    return Err("find needs a List result".into());
                };
                format!(
                    "(({{ KString text_ = {1}; int64_t start_ = {2}; KRxProgram *program_ = k_rx_valid({0}, {at}); if (start_ < 0 || (uint64_t)start_ > text_.length || ((size_t)start_ < text_.length && ((unsigned char)text_.data[start_] & 0xc0) == 0x80)) k_trap({at}, \"regex search start is not a character position\"); size_t slots_ = (size_t)program_->groups * 2; int64_t *caps_ = malloc(slots_ * sizeof *caps_); if (!caps_) k_fatal(\"Out of memory\"); KRecord_{list} found_ = {{0}}; if (k_rx_search(program_, text_, (size_t)start_, caps_)) for (size_t i_ = 0; i_ < slots_; ++i_) k_list_{list}_append(&found_, caps_[i_]); free(caps_); found_; }}))",
                    arg(0),
                    arg(1),
                    arg(2)
                )
            }
            "io.write" => format!("k_os_write({}, false)", arg(0)),
            "io.eprint" => format!(
                "({{ k_os_write({}, true); k_os_write(K_TEXT(\"\\n\"), true); }})",
                arg(0)
            ),
            "io.flush" => "((void)fflush(stdout))".into(),
            "io.input" => format!(
                "({{ k_os_write({}, false); fflush(stdout); k_readLine(); }})",
                arg(0)
            ),
            "time.now" => "k_time_now()".into(),
            "time.monotonic" => "k_time_monotonic()".into(),
            "time.sleep" => format!("k_time_sleep({})", arg(0)),
            "process.exit" => format!("k_os_exit({})", arg(0)),
            "process.cwd" => "k_os_cwd()".into(),
            "process.env" => {
                let Ty::Record(option) = ty else {
                    return Err("env needs an Option result".into());
                };
                let cases = self.program.layouts[option]
                    .kind
                    .cases()
                    .ok_or("env needs an Option result")?;
                let some = cases
                    .iter()
                    .position(|case| case.name == "Some")
                    .ok_or("Some")?;
                let none = cases
                    .iter()
                    .position(|case| case.name == "None")
                    .ok_or("None")?;
                format!(
                    "(({{ KString value_; k_os_env({}, &value_) ? k_aggregate_{option}_{some}(value_) : k_aggregate_{option}_{none}(); }}))",
                    arg(0)
                )
            }
            "process.run" => {
                let (id, success, failure, field) = self.fallible_cases(ty)?;
                let Some(Ty::Record(tuple)) = field else {
                    return Err("run needs a tuple result".into());
                };
                format!(
                    "(({{ int64_t status_; KString output_; KError error_; k_os_run({}, &status_, &output_, &error_) ? k_aggregate_{id}_{success}(k_aggregate_{tuple}_tuple(status_, output_)) : k_aggregate_{id}_{failure}(error_); }}))",
                    arg(0)
                )
            }
            _ => return Err(format!("Unsupported system call {system}")),
        })
    }

    fn rhs(&self, rhs: &Rhs, ty: Ty, loc: &Location) -> Result<String, String> {
        let at = location(loc);
        Ok(match rhs {
            Rhs::OwnerNew(value, native) => {
                let owned = if copies(self.program, *native) {
                    format!("k_{}_copy({})", suffix(*native), self.atom(value))
                } else {
                    self.take(value, *native)
                };
                format!("k_owner_{}_new({owned})", suffix(*native))
            }
            // The value usually points into the owner the step takes. C leaves argument
            // order open, and Windows Clang evaluates right to left, so read it first.
            Rhs::StepSome(id, value, _, owner) => format!(
                "({{ __typeof__({value}) step_value_ = {value}; k_step_{id}(step_value_, {}); }})",
                self.take(owner, Ty::Owner),
                value = self.atom(value),
            ),
            Rhs::ListAppendOwner(id, output, value, native, owner) => {
                let LayoutKind::List(element) = self.program.layouts[*id].kind else {
                    unreachable!()
                };
                let value = if matches!((element, native), (Ty::Record(a), Ty::Ref(b)) if a == *b) {
                    format!("(*{})", self.atom(value))
                } else {
                    self.atom(value)
                };
                format!(
                    "k_list_{id}_append_view(&{}, {value}, k_owner_copy(k_owner_require({})), k_owner_release)",
                    self.atom(output),
                    self.atom(owner)
                )
            }
            Rhs::MapNew(id, values) => {
                let LayoutKind::Map(info) = self.program.layouts[*id].kind else {
                    unreachable!()
                };
                if values.is_empty() {
                    format!("(KRecord_{id}) {{0}}")
                } else {
                    let keys = values
                        .iter()
                        .step_by(2)
                        .map(|(a, t)| self.take(a, *t))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let vals = values
                        .iter()
                        .skip(1)
                        .step_by(2)
                        .map(|(a, t)| self.take(a, *t))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(
                        "k_map_{id}_new({},({}[]){{{keys}}},({}[]){{{vals}}})",
                        values.len() / 2,
                        c_type(info.key),
                        c_type(info.value)
                    )
                }
            }
            Rhs::MapPut(id, map, key, key_ty, value, value_ty) => {
                let LayoutKind::Map(info) = self.program.layouts[*id].kind else {
                    unreachable!()
                };
                format!(
                    "k_map_{id}_put{}({}, {}, {})",
                    if info.mutable { "_mut" } else { "" },
                    if info.mutable {
                        self.atom(map)
                    } else {
                        self.take(map, Ty::Record(*id))
                    },
                    self.take(key, *key_ty),
                    self.take(value, *value_ty)
                )
            }
            Rhs::MapGet(id, map, key, contains) => {
                let owner = if matches!(self.atom_type(map), Ty::Ref(_)) {
                    format!("(*{})", self.atom(map))
                } else {
                    self.atom(map)
                };
                if *contains {
                    format!("k_map_{id}_contains({owner},{})", self.atom(key))
                } else {
                    let Ty::Record(result) = ty else {
                        unreachable!()
                    };
                    format!("k_map_{id}_get_{result}({owner},{})", self.atom(key))
                }
            }
            Rhs::ListNew(id, values) => {
                let LayoutKind::List(element) = self.program.layouts[*id].kind else {
                    unreachable!()
                };
                if values.is_empty() {
                    format!("(KRecord_{id}) {{0}}")
                } else {
                    format!(
                        "k_list_{id}_new({},({}[]) {{ {} }})",
                        values.len(),
                        c_type(element),
                        values
                            .iter()
                            .map(|(atom, ty)| self.take(atom, *ty))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            }
            Rhs::ListConcat(id, left, right) => {
                format!(
                    "k_list_{id}_concat({}, {})",
                    self.atom(left),
                    self.atom(right)
                )
            }
            Rhs::ListSlice(id, source, skip, keep) => {
                format!("k_list_{id}_slice({}, {skip}, {keep})", self.atom(source))
            }
            Rhs::ListAppendSet(list, set, output, source, index) => format!(
                "k_list_{list}_append_set_{set}(&{}, {}, {})",
                self.atom(output),
                self.atom(source),
                self.atom(index)
            ),
            Rhs::ListAppendFrom(id, list, source, index) => format!(
                "k_list_{id}_append_from(&{}, {}, {})",
                self.atom(list),
                self.atom(source),
                self.atom(index)
            ),
            Rhs::ListAppend(id, list, value, ty) => format!(
                "k_list_{id}_append(&{}, {})",
                self.atom(list),
                self.take(value, *ty)
            ),
            Rhs::JoinTasks(slot) => format!("k_tasks_join(&{})", self.slot(*slot)),
            Rhs::FileClose(owner) => {
                let Ty::Ref(id) = self.atom_type(owner) else {
                    return Err("File cleanup needs a resource view".into());
                };
                format!("k_resource_{id}_close({})", self.atom(owner))
            }
            Rhs::StaticRead(id) => format!("k_static_read_{id}()"),
            Rhs::StaticWrite(id, atom) => {
                let field = &self.program.statics[*id];
                let value = static_copy(self.program, field.ty, &self.atom(atom));
                format!("k_static_write_{id}({value})")
            }
            Rhs::Copy(atom) => {
                // Copying a copyable record out of a view reads through the pointer.
                if let Ty::Record(id) = ty
                    && copies(self.program, ty)
                    && matches!(self.atom_type(atom), Ty::Ref(other) | Ty::Boxed(other) if other == id)
                {
                    return Ok(format!("k_record_{id}_copy(*({}))", self.atom(atom)));
                }
                if copies(self.program, ty) || ty == Ty::Python {
                    format!("k_{}_copy({})", suffix(ty), self.atom(atom))
                } else if matches!(ty,Ty::Record(id) if matches!(self.program.layouts[id].kind,LayoutKind::ClassView(_)))
                {
                    self.atom(atom)
                } else if matches!(ty, Ty::Record(_)) {
                    return Err("Internal native lowering attempted to copy an owned class.".into());
                } else {
                    self.atom(atom)
                }
            }
            Rhs::Take(atom) => self.take(atom, ty),
            Rhs::Unary(op, atom) => {
                let value = self.atom(atom);
                match (op.as_str(), ty) {
                    ("-", Ty::Int) => format!("k_neg({value}, {at})"),
                    ("-" | "+", Ty::Double)
                    | ("+", Ty::Int)
                    | ("!", Ty::Bool)
                    | ("~", Ty::Int)
                    | ("-", Ty::C(_)) => {
                        format!("({op}{value})")
                    }
                    _ => return Err(format!("Unsupported native unary operator {op} on {ty:?}")),
                }
            }
            Rhs::Binary(op, operand, a, b) => {
                let (left, right) = (self.atom(a), self.atom(b));
                match (op.as_str(), operand) {
                    ("==", _) => self.equal(*operand, a, b),
                    ("!=", _) => format!("(!{})", self.equal(*operand, a, b)),
                    ("+", Ty::String) => format!("k_string_concat({left}, {right})"),
                    ("*", Ty::String) => format!("k_string_repeat({left}, {right})"),
                    ("+" | "-" | "*" | "/" | "%", Ty::Int) => {
                        let name = match op.as_str() {
                            "+" => "add",
                            "-" => "sub",
                            "*" => "mul",
                            "/" => "div",
                            _ => "rem",
                        };
                        format!("k_{name}({left}, {right}, {at})")
                    }
                    ("%", Ty::Double) => format!("fmod({left}, {right})"),
                    ("+", Ty::Nat) => format!("k_nat_add({left},{right},{at})"),
                    ("*", Ty::Nat) => format!("k_nat_mul({left},{right},{at})"),
                    ("-", Ty::Nat) => format!("k_nat_sub({left},{right},{at})"),
                    ("/", Ty::Nat) => format!("k_nat_div({left},{right},{at})"),
                    ("%", Ty::Nat) => format!("k_nat_rem({left},{right},{at})"),
                    ("**", Ty::Int) => format!("k_pow({left}, {right}, {at})"),
                    ("**", Ty::Double) => format!("pow({left}, {right})"),
                    ("<<", Ty::Int) => format!("k_shl({left}, {right}, {at})"),
                    (">>", Ty::Int) => format!("k_shr({left}, {right}, {at})"),
                    ("<" | "<=" | ">" | ">=", Ty::Nat)
                    | ("+" | "-" | "*" | "/" | "<" | "<=" | ">" | ">=", Ty::Int | Ty::Double)
                    | ("&" | "|" | "^", Ty::Int) => format!("({left} {op} {right})"),
                    _ => {
                        return Err(format!(
                            "Unsupported native binary operator {op} on {operand:?}"
                        ));
                    }
                }
            }
            Rhs::Call(index, args) => {
                let parameters = &self.program.functions[*index].parameters;
                let args = args
                    .iter()
                    .enumerate()
                    .map(|(i, (atom, mode))| {
                        let ty = self.atom_type(atom);
                        // A parameter that views its argument takes the pointer as it is. A
                        // counted box passed this way gains no reference, since the callee
                        // would never release one.
                        let viewed = matches!(parameters.get(i), Some((_, Ty::Ref(_), _)))
                            && matches!(ty, Ty::Boxed(_));
                        if *mode == Passing::Mut {
                            format!("&{}", self.atom(atom))
                        } else if *mode == Passing::Move {
                            self.take(atom, ty)
                        } else if viewed {
                            self.atom(atom)
                        } else if copies(self.program, ty) || ty == Ty::Python {
                            format!("k_{}_copy({})", suffix(ty), self.atom(atom))
                        } else {
                            self.atom(atom)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("k_fn_{index}({args})")
            }
            Rhs::Builtin(name, input, atoms) => {
                let args = atoms.iter().map(|a| self.atom(a)).collect::<Vec<_>>();
                if name == "superView" {
                    let Ty::Record(id) = input else {
                        return Err("Invalid parent receiver".into());
                    };
                    return Ok(format!("((KRecord_{id} *)({}))", args[0]));
                }
                if let Some(id) = name.strip_prefix("mapRemove.") {
                    let map = if matches!(self.atom_type(&atoms[0]), Ty::Ref(_)) {
                        self.atom(&atoms[0])
                    } else {
                        format!("&{}", self.atom(&atoms[0]))
                    };
                    return Ok(format!("k_map_{id}_remove({map}, {})", args[1]));
                }
                if let Some(id) = name.strip_prefix("mapRemoved.") {
                    let id = id.parse::<usize>().map_err(|e| e.to_string())?;
                    return Ok(format!(
                        "k_map_{id}_removed({}, {})",
                        self.take(&atoms[0], Ty::Record(id)),
                        args[1]
                    ));
                }
                if let Some(system) = name.strip_prefix("system.") {
                    return self.system_rhs(system, &args, ty, &at);
                }
                if let Some(shape) = name.strip_prefix("listShape.") {
                    let (operation, list) = shape.split_once('.').unwrap();
                    return Ok(match operation {
                        "slice" => {
                            format!("k_list_{list}_range({}, {}, {})", args[0], args[1], args[2])
                        }
                        "pick" => format!(
                            "(({{ KRecord_{list} source_ = {}, picked_ = {{0}}; __auto_type positions_ = {}; int64_t length_ = source_.handle ? (int64_t)source_.handle->length : 0; if (positions_.handle) for (size_t i_ = 0; i_ < positions_.handle->length; ++i_) {{ int64_t p_ = positions_.handle->storage[i_].value; if (p_ < 0 || p_ >= length_) k_trap({at}, \"List index out of range\"); k_list_{list}_append_from(&picked_, source_, p_); }} picked_; }}))",
                            args[0], args[1]
                        ),
                        _ => format!("k_list_{list}_reversed({})", args[0]),
                    });
                }
                if let Some(operation) = name.strip_prefix("numeric.") {
                    return self.numerical_rhs(operation, atoms, ty, at);
                }
                if let Some(operation) = name.strip_prefix("variable.") {
                    return self.variable_rhs(operation, atoms, ty, at);
                }
                if let Some(operation) = name.strip_prefix("tensor.") {
                    return self.tensor_rhs(operation, atoms, ty, at);
                }
                if let Some(operation) = name.strip_prefix("bigint.") {
                    let Ty::Record(id) = input else {
                        return Err("A BigInt operation needs its layout".into());
                    };
                    if operation == "literal" {
                        let Atom::Text(text) = &atoms[0] else {
                            return Err("A BigInt literal needs its spelling".into());
                        };
                        return Ok(format!(
                            "(KRecord_{id}){{k_bigint_clone({})}}",
                            bigint_constant(text)
                        ));
                    }
                    if operation == "fromInt" {
                        return Ok(format!("(KRecord_{id}){{k_bigint_fromInt({})}}", args[0]));
                    }
                    if operation == "fromNat" {
                        return Ok(format!("(KRecord_{id}){{k_bigint_fromNat({})}}", args[0]));
                    }
                    let operands = atoms
                        .iter()
                        .map(|atom| self.bigint_pointer(atom))
                        .collect::<Vec<_>>();
                    let comparison = matches!(
                        operation,
                        "eq" | "ne" | "lt" | "le" | "gt" | "ge" | "compare"
                    );
                    if comparison {
                        let compare = format!("k_bigint_compare({}, {})", operands[0], operands[1]);
                        return Ok(match operation {
                            "compare" => compare,
                            op => format!(
                                "({compare} {} 0)",
                                match op {
                                    "eq" => "==",
                                    "ne" => "!=",
                                    "lt" => "<",
                                    "le" => "<=",
                                    "gt" => ">",
                                    _ => ">=",
                                }
                            ),
                        });
                    }
                    let mut operands = operands.join(", ");
                    if matches!(
                        operation,
                        "checkedToInt" | "checkedToDouble" | "checkedDivide"
                    ) {
                        let Ty::Record(result) = ty else {
                            return Err("A checked BigInt operation needs a union result".into());
                        };
                        if operation == "checkedDivide" {
                            operands.push_str(&format!(", {at}"));
                        }
                        return Ok(format!("k_checked_bigint_{operation}_{result}({operands})"));
                    }
                    if matches!(operation, "div" | "rem" | "pow") {
                        operands.push_str(&format!(", {at}"));
                    }
                    let call = format!("k_bigint_{operation}({operands})");
                    return Ok(if operation == "text" {
                        call
                    } else {
                        format!("(KRecord_{id}){{{call}}}")
                    });
                }
                if let Some(resource) = name.strip_prefix("resource.") {
                    let (id, method) = resource
                        .split_once('.')
                        .ok_or("Malformed resource operation")?;
                    let id: usize = id.parse().map_err(|_| "Malformed resource type")?;
                    let mut args = args;
                    if method == "new" && !atoms.is_empty()
                        || matches!(method, "send" | "insert" | "replace")
                    {
                        let index = match method {
                            "send" | "insert" => 1,
                            "replace" => 2,
                            _ => 0,
                        };
                        args[index] = self.take(&atoms[index], self.atom_type(&atoms[index]));
                    }
                    if method == "get"
                        && matches!(
                            self.program.layouts[id].kind,
                            LayoutKind::Resource(crate::core::Resource::Arena(_))
                        )
                        && let Ty::Record(option) = ty
                    {
                        return Ok(format!("k_resource_{id}_get_{option}({})", args.join(", ")));
                    }
                    return Ok(format!("k_resource_{id}_{method}({})", args.join(", ")));
                }
                if let Some(function) = name.strip_prefix("c:") {
                    if self.program.c_text_calls.contains_key(function) {
                        return Ok(format!("k_c_{function}({})", args.join(", ")));
                    }
                    return Ok(format!("{function}({})", args.join(", ")));
                }
                if name == "testSelected" {
                    return Ok(format!("(k_test_index == {})", args[0]));
                }
                if name == "testResult" {
                    // A test's `void or Error` result: an Error ends the run as a failure.
                    let Ty::Record(id) = input else {
                        return Err("A test result is not a union".into());
                    };
                    let LayoutKind::Union(_, cases) = &self.program.layouts[*id].kind else {
                        return Err("A test result is not a union".into());
                    };
                    let value = self.atom(&atoms[0]);
                    let failure = cases.iter().enumerate().find_map(|(case, variant)| {
                        match variant.fields.first() {
                            Some(Ty::Error) => {
                                Some(format!("({value}).tag == {} ? k_test_error(({value}).data.c{case}.f0)", case + 1))
                            }
                            Some(Ty::Record(error)) if self.program.layouts[*error].is_error => Some(format!(
                                "({value}).tag == {} ? k_test_error(k_error_box_{error}(({value}).data.c{case}.f0))",
                                case + 1
                            )),
                            _ => None,
                        }
                    });
                    return Ok(match failure {
                        Some(failure) => format!("({failure} : (void)0)"),
                        None => "((void)0)".into(),
                    });
                }
                if name == "assertFailed" {
                    return Ok(format!("k_assert_failed({}, {}, {at})", args[0], args[1]));
                }
                if name == "assertNotEqFailed" {
                    return Ok(format!(
                        "k_check_failed(\"assertNotEq\", k_string_concat(K_TEXT(\"both sides: \"), {}), {at})",
                        args[0]
                    ));
                }
                if name == "assertTrue" || name == "assertFalse" {
                    let wanted = name == "assertTrue";
                    return Ok(format!(
                        "((void)(({}) == {wanted} || (k_check_failed(\"{name}\", K_TEXT(\"the condition was {}\"), {at}), 0)))",
                        args[0], !wanted
                    ));
                }
                if name == "assertNear" {
                    return Ok(format!(
                        "k_assert_near({}, {}, {}, {at})",
                        args[0], args[1], args[2]
                    ));
                }
                if name == "hashOf" {
                    let value = &args[0];
                    return Ok(match input {
                        Ty::Double => format!(
                            "(int64_t)k_hash_mix({value}==0.0 ? 0 : k_double_bits({value}))"
                        ),
                        Ty::String => format!("(int64_t)k_hash_string({value})"),
                        _ => format!("(int64_t)k_hash_mix((uint64_t)({value}))"),
                    });
                }
                if name == "stringDebug" {
                    return Ok(format!("k_string_debug({})", args[0]));
                }
                if name == "compareOf" {
                    let (a, b) = (&args[0], &args[1]);
                    return Ok(match input {
                        Ty::Double => format!("k_compare_double({a}, {b})"),
                        Ty::String => format!("k_compare_string({a}, {b})"),
                        _ => format!("((int64_t)(({a}) > ({b})) - (int64_t)(({a}) < ({b})))"),
                    });
                }
                if name == "hashCombine" {
                    return Ok(format!(
                        "(int64_t)k_hash_mix(((uint64_t)({}) * UINT64_C(0x100000001b3)) ^ (uint64_t)({}))",
                        args[0], args[1]
                    ));
                }
                if name == "scriptFail" {
                    return Ok(format!("k_script_fail({})", args[0]));
                }
                if name == "sessionAbort" {
                    return Ok("(void)(k_session_abort = 1)".into());
                }
                if name == "ensuresFailed" {
                    return Ok(format!(
                        "k_trap({at}, {})",
                        quoted(&format!("Postcondition failed: {}", loc.expression))
                    ));
                }
                if name == "invariantFailed" {
                    let Atom::Text(message) = &atoms[0] else {
                        return Err("An invariant failure needs its message".into());
                    };
                    return Ok(format!("k_trap({at}, {})", quoted(message)));
                }
                if name == "arrayIndex" {
                    return Ok(format!("k_array_index({}, {}, {at})", args[0], args[1]));
                }
                if name == "stringAt" {
                    return Ok(format!("k_string_at({}, {}, {at})", args[0], args[1]));
                }
                if name == "stringGet" {
                    let Ty::Record(option) = ty else {
                        unreachable!()
                    };
                    return Ok(format!(
                        "({{ bool found_ = false; KString character_ = k_string_get_scalar({}, {}, &found_); found_ ? k_aggregate_{option}_0(character_) : k_aggregate_{option}_1(); }})",
                        args[0], args[1]
                    ));
                }
                // The stored value of an Option when it is Some, otherwise the fallback (Q127). Scalars move out.
                if name == "optionOr" {
                    let Ty::Record(option) = self.atom_type(&atoms[0]) else {
                        unreachable!()
                    };
                    return Ok(format!(
                        "(({{ KRecord_{option} found_ = {}; found_.tag == 1 ? found_.data.c0.f0 : {}; }}))",
                        args[0], args[1]
                    ));
                }
                if let Some(id) = name.strip_prefix("sortedBy.") {
                    return Ok(format!("k_list_{id}_sorted_by_key({})", args[0]));
                }
                if let Some(id) = name.strip_prefix("listSorted.") {
                    return Ok(format!("k_list_{id}_sorted({})", args[0]));
                }
                if let Some(id) = name.strip_prefix("arraySort.") {
                    let owner = &atoms[0];
                    let pointer = if matches!(self.atom_type(owner), Ty::Ref(_) | Ty::Boxed(_)) {
                        self.atom(owner)
                    } else {
                        format!("&{}", self.atom(owner))
                    };
                    return Ok(format!("k_array_{id}_sort({pointer})"));
                }
                if let Some(id) = name.strip_prefix("arraySwap.") {
                    let owner = &atoms[0];
                    let pointer = if matches!(self.atom_type(owner), Ty::Ref(_) | Ty::Boxed(_)) {
                        self.atom(owner)
                    } else {
                        format!("&{}", self.atom(owner))
                    };
                    return Ok(format!(
                        "k_array_{id}_swap({pointer}, {}, {}, {at})",
                        args[1], args[2]
                    ));
                }
                if let Some(id) = name.strip_prefix("arrayPop.") {
                    let Ty::Record(option) = ty else {
                        unreachable!()
                    };
                    let owner = &atoms[0];
                    let pointer = if matches!(self.atom_type(owner), Ty::Ref(_) | Ty::Boxed(_)) {
                        self.atom(owner)
                    } else {
                        format!("&{}", self.atom(owner))
                    };
                    return Ok(format!(
                        "(({pointer})->length ? k_aggregate_{option}_0(k_array_{id}_take({pointer})) : k_aggregate_{option}_1())"
                    ));
                }
                if let Some(id) = name.strip_prefix("arrayPrepend.") {
                    let owner = &atoms[0];
                    let pointer = if matches!(self.atom_type(owner), Ty::Ref(_) | Ty::Boxed(_)) {
                        self.atom(owner)
                    } else {
                        format!("&{}", self.atom(owner))
                    };
                    return Ok(format!("k_array_{id}_prepend({pointer})"));
                }
                if let Some(id) = name.strip_prefix("arrayAppend.") {
                    let owner = &atoms[0];
                    let pointer = if matches!(self.atom_type(owner), Ty::Ref(_) | Ty::Boxed(_)) {
                        self.atom(owner)
                    } else {
                        format!("&{}", self.atom(owner))
                    };
                    return Ok(format!("k_array_{id}_append({pointer})"));
                }
                if name == "cIsNull" {
                    return Ok(format!("({} == NULL)", args[0]));
                }
                if let Some(ids) = name.strip_prefix("cursorBox.") {
                    let ids = ids.split('.').collect::<Vec<_>>();
                    let [iterator, owned, view, cell] = ids.as_slice() else {
                        return Err("Malformed scoped cursor".into());
                    };
                    let Ty::Boxed(concrete) = input else {
                        return Err("A scoped cursor needs owner storage".into());
                    };
                    return Ok(format!(
                        "({} ? (KDyn_{iterator}){{k_box_{concrete}_take(&{}),UINT32_C({owned})}} : (KDyn_{iterator}){{k_box_{cell}_new(k_aggregate_{cell}_tuple({})),UINT32_C({view})}})",
                        args[1], args[1], args[0]
                    ));
                }
                if name == "__columnTop" {
                    return Ok(format!(
                        "k_kesdas_top({}, {}, {})",
                        args[0], args[1], args[2]
                    ));
                }
                if name == "__mergeLeftPositions" {
                    return Ok(format!("k_kesdas_merge_left({}, {})", args[0], args[1]));
                }
                if name == "__factorCodes" {
                    return Ok(format!("k_kesdas_factor_codes({})", args[0]));
                }
                if name == "__columnArithmetic" {
                    return Ok(format!(
                        "k_kesdas_arithmetic({}, {}, {})",
                        args[0], args[1], args[2]
                    ));
                }
                if name == "__columnCodeCounts" {
                    return Ok(format!("k_kesdas_code_counts({}, {})", args[0], args[1]));
                }
                if name == "__columnGroupStat" {
                    return Ok(format!(
                        "k_kesdas_group_stat({}, {}, {}, {}, {})",
                        args[0], args[1], args[2], args[3], args[4]
                    ));
                }
                if name == "__columnCodes" {
                    return Ok(format!("k_kesdas_column_codes({})", args[0]));
                }
                if name == "__columnTile" {
                    return Ok(format!("k_kesdas_tile({}, {})", args[0], args[1]));
                }
                if name == "__columnConcat" {
                    return Ok(format!("k_kesdas_concat({}, {})", args[0], args[1]));
                }
                if name == "__columnIndex" {
                    return Ok(format!("k_kesdas_index({})", args[0]));
                }
                if name == "__columnRepeatNames" {
                    return Ok(format!("k_kesdas_repeat_names({}, {})", args[0], args[1]));
                }
                if name == "__columnCompareFloat" {
                    return Ok(format!(
                        "k_kesdas_compare_float({}, {}, {})",
                        args[0], args[1], args[2]
                    ));
                }
                if name == "__columnBoth" {
                    return Ok(format!("k_kesdas_both({}, {})", args[0], args[1]));
                }
                if name == "__columnRollingMean" {
                    return Ok(format!(
                        "k_kesdas_rolling_mean({}, {}, {})",
                        args[0], args[1], args[2]
                    ));
                }
                if name == "__columnCorrelation" {
                    return Ok(format!("k_kesdas_correlation({}, {})", args[0], args[1]));
                }
                if name == "__columnGather" {
                    return Ok(format!("k_kesdas_gather({}, {})", args[0], args[1]));
                }
                if name == "__columnOrder" {
                    return Ok(format!(
                        "k_kesdas_order({}, {}, {}, {})",
                        args[0], args[1], args[2], args[3]
                    ));
                }
                if name == "__kesdasReadCsv" {
                    return Ok(format!(
                        "k_kesdas_read_csv({}, {}, {}, {}, {})",
                        args[0], args[1], args[2], args[3], args[4]
                    ));
                }
                if let Some(id) = name.strip_prefix("listAt.") {
                    let id: usize = id.parse().map_err(|_| "Malformed List operation")?;
                    let owner = self.atom(&atoms[0]);
                    let index = &args[1];
                    return Ok(format!("k_list_{id}_at({owner}, {index}, {at})"));
                }
                if let Some(id) = name.strip_prefix("arraySplit.") {
                    let id: usize = id.parse().map_err(|_| "Malformed Array split")?;
                    return Ok(format!(
                        "k_array_{id}_split({}, {}, {at})",
                        args[0], args[1]
                    ));
                }
                if let Some(id) = name.strip_prefix("arrayNew.") {
                    return Ok(format!("k_array_{id}_new({}, {}, {at})", args[0], args[1]));
                }
                if let Some(id) = name.strip_prefix("arrayAllocate.") {
                    return Ok(format!("k_array_{id}_allocate({}, {at})", args[0]));
                }
                if let Some(id) = name.strip_prefix("arrayGet.") {
                    let id: usize = id.parse().map_err(|_| "Malformed Array operation")?;
                    let LayoutKind::Array(element) = self.program.layouts[id].kind else {
                        unreachable!()
                    };
                    let Ty::Record(option) = ty else {
                        unreachable!()
                    };
                    let owner = self.through(&atoms[0]);
                    let index = &args[1];
                    let item = format!("{owner}items[{index}]");
                    let field = match &self.program.layouts[option].kind {
                        LayoutKind::Enum(cases) | LayoutKind::Step(cases) => cases[0].fields[0],
                        _ => unreachable!(),
                    };
                    let item = if matches!(element, Ty::Record(_)) && matches!(field, Ty::Ref(_)) {
                        format!("&{item}")
                    } else if copies(self.program, element) || element == Ty::Python {
                        format!("k_{}_copy({item})", suffix(element))
                    } else {
                        item
                    };
                    return Ok(format!(
                        "(({index}) >= 0 && ({index}) < {owner}length ? k_aggregate_{option}_0({item}) : k_aggregate_{option}_1())"
                    ));
                }
                if let Some(index) = name.strip_prefix("cmake.") {
                    let Ty::Record(id) = ty else { unreachable!() };
                    return Ok(format!("k_cmake_{id}_{index}({})", args[0]));
                }
                match name.as_str() {
                    "checkedToInt" if matches!(input, Ty::C(_)) => {
                        let Ty::Record(id) = ty else { unreachable!() };
                        format!("k_checked_cuint64ToInt_{id}({})", args[0])
                    }
                    "toInt" => format!("((int64_t)({}))", args[0]),
                    "checkedAdd" | "checkedSubtract" | "checkedMultiply" | "checkedDivide" => {
                        let Ty::Record(id) = ty else { unreachable!() };
                        format!("k_checked_{name}_{id}({}, {})", args[0], args[1])
                    }
                    "parseInt" | "parseDouble" | "parseBigInt" | "checkedToInt" => {
                        let Ty::Record(id) = ty else { unreachable!() };
                        format!("k_checked_{name}_{id}({})", args[0])
                    }
                    "print" => format!("k_print_{}({})", suffix(*input), args[0]),
                    "readLine" => "k_readLine()".into(),
                    "compare" if *input == Ty::Double => {
                        format!("k_compare_double({}, {})", args[0], args[1])
                    }
                    "compare" if *input == Ty::String => {
                        format!("k_compare_string({}, {})", args[0], args[1])
                    }
                    "compare" => format!(
                        "((int64_t)(({}) > ({})) - (int64_t)(({}) < ({})))",
                        args[0], args[1], args[0], args[1]
                    ),
                    "length" => format!("k_string_length({})", args[0]),
                    "trim" => format!("k_string_trim({})", args[0]),
                    // Python's str methods (Q136). Overloads are told apart by their argument count.
                    _ if *input == Ty::String && string_method_name(name, args.len()) => {
                        string_method(name, &args, ty, &at)
                    }
                    "toUpper" | "toLower" => format!(
                        "k_string_case({}, {})",
                        args[0],
                        if name == "toUpper" { "true" } else { "false" }
                    ),
                    "contains" if *input == Ty::String => {
                        format!("k_string_contains({}, {})", args[0], args[1])
                    }
                    "min" | "max" => format!(
                        "(({}) {} ({}) ? ({}) : ({}))",
                        args[0],
                        if name == "min" { "<" } else { ">" },
                        args[1],
                        args[0],
                        args[1]
                    ),
                    "startsWith" => format!("k_string_starts_with({}, {})", args[0], args[1]),
                    "endsWith" => format!("k_string_ends_with({}, {})", args[0], args[1]),
                    "floor" => format!("floor({})", args[0]),
                    "ceil" => format!("ceil({})", args[0]),
                    "round" => format!("round({})", args[0]),
                    "isEven" => format!("(({}) % 2 == 0)", args[0]),
                    "formatInt" if *input == Ty::Nat => {
                        format!("k_format_nat({}, {}, {at})", args[0], args[1])
                    }
                    "formatInt" => {
                        format!("k_format_int((int64_t)({}), {}, {at})", args[0], args[1])
                    }
                    "formatDouble" => format!("k_format_double({}, {})", args[0], args[1]),
                    "formatText" => format!("k_format_text({}, {})", args[0], args[1]),
                    "chr" => format!("k_string_chr({}, {at})", args[0]),
                    "sign" if *input == Ty::Int => {
                        format!("((int64_t)(({0}) > 0) - (int64_t)(({0}) < 0))", args[0])
                    }
                    "sign" => format!(
                        "(isnan({0}) ? ({0}) : (double)((({0}) > 0) - (({0}) < 0)))",
                        args[0]
                    ),
                    "isqrt" => format!("k_int_isqrt({}, {at})", args[0]),
                    "popCount" => {
                        format!("((int64_t)__builtin_popcountll((uint64_t)({})))", args[0])
                    }
                    "isPrime" => format!("k_int_is_prime({})", args[0]),
                    "gcd" => format!("k_int_gcd({}, {}, {at})", args[0], args[1]),
                    "lcm" => format!("k_int_lcm({}, {}, {at})", args[0], args[1]),
                    "mod" => format!("k_int_mod({}, {}, {at})", args[0], args[1]),
                    "clamp" if *input == Ty::Int => {
                        format!("k_int_clamp({}, {}, {}, {at})", args[0], args[1], args[2])
                    }
                    "clamp" => format!(
                        "k_double_clamp({}, {}, {}, {at})",
                        args[0], args[1], args[2]
                    ),
                    "isNaN" => format!("isnan({})", args[0]),
                    "trunc" => format!("trunc({})", args[0]),
                    "cbrt" => format!("k_cbrt({})", args[0]),
                    "hypot" => format!("hypot({}, {})", args[0], args[1]),
                    "atan2" => format!("atan2({}, {})", args[0], args[1]),
                    "isOdd" => format!("(({}) % 2 != 0)", args[0]),
                    "natSucc" => format!("k_nat_succ({}, {at})", args[0]),
                    "toDouble" => format!("((double){})", args[0]),
                    "isFinite" => format!("isfinite({})", args[0]),
                    "abs" if *input == Ty::Int => format!("k_abs_int({}, {at})", args[0]),
                    "abs" => format!("fabs({})", args[0]),
                    "sqrt" => format!("sqrt({})", args[0]),
                    "pow" => format!("pow({}, {})", args[0], args[1]),
                    "assertError" => format!(
                        "({{ if (!({})) k_check_failed(\"assertError\", K_TEXT(\"Expected an Error, received a success value\"), {at}); }})",
                        args[0]
                    ),
                    "assertEq" => format!(
                        "k_assert_{}({}, {}, {at})",
                        suffix(*input),
                        args[0],
                        args[1]
                    ),
                    _ => return Err(format!("Unsupported native builtin {name}")),
                }
            }
            Rhs::Text(input, atom, precision) => {
                let value = self.atom(atom);
                if let Some(precision) = precision {
                    format!("k_text_fixed({value}, UINT32_C({precision}))")
                } else if *input == Ty::String {
                    format!("k_string_copy({value})")
                } else if *input == Ty::Error {
                    format!("k_error_text({value})")
                } else {
                    format!("k_text_{}({value})", suffix(*input))
                }
            }
            Rhs::Test(test) => self.test(test),
            Rhs::ClosureNew(layout, code, env) => {
                let env = match env {
                    None => "NULL".into(),
                    Some((env, values)) => {
                        let arguments = values
                            .iter()
                            .map(|(atom, ty, by_ref)| {
                                if *by_ref {
                                    format!("&{}", self.atom(atom))
                                } else if copies(self.program, *ty) {
                                    format!("k_{}_copy({})", suffix(*ty), self.atom(atom))
                                } else {
                                    self.take(atom, *ty)
                                }
                            })
                            .collect::<Vec<_>>();
                        format!("k_env_{env}_new({})", arguments.join(", "))
                    }
                };
                format!("(KRecord_{layout}) {{ {env}, k_closure_code_{code} }}")
            }
            Rhs::StringAppend(slot, piece) => {
                format!(
                    "k_string_append(&{}, {})",
                    self.slot(*slot),
                    self.atom(piece)
                )
            }
            Rhs::StringAppendText(slot, native, atom) => {
                let suffix = match native {
                    Ty::Int => "int",
                    Ty::Nat => "nat",
                    Ty::Double => "double",
                    _ => "bool",
                };
                format!(
                    "k_string_append_{suffix}(&{}, {})",
                    self.slot(*slot),
                    self.atom(atom)
                )
            }
            Rhs::Python(site, atoms) => format!(
                "k_python_site_{site}({})",
                atoms
                    .iter()
                    .map(|a| self.atom(a))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Rhs::ClosureCall(layout, closure, arguments) => {
                let LayoutKind::Closure { parameters, .. } = &self.program.layouts[*layout].kind
                else {
                    unreachable!()
                };
                let access = if matches!(self.atom_type(closure), Ty::Ref(_)) {
                    "->"
                } else {
                    "."
                };
                let closure = format!("({})", self.atom(closure));
                let mut values = vec![format!("{closure}{access}env")];
                for (atom, (ty, passing)) in arguments.iter().zip(parameters) {
                    values.push(if *passing == Passing::Move {
                        self.take(atom, *ty)
                    } else if *passing == Passing::Mut && !matches!(ty, Ty::Ref(_)) {
                        format!("&{}", self.atom(atom))
                    } else if copies(self.program, *ty) || *ty == Ty::Python {
                        format!("k_{}_copy({})", suffix(*ty), self.atom(atom))
                    } else {
                        self.atom(atom)
                    });
                }
                format!("{closure}{access}code({})", values.join(", "))
            }
            Rhs::MakeDyn(trait_id, implementation, pointer) => format!(
                "(KDyn_{trait_id}) {{ (void *)({}), UINT32_C({implementation}) }}",
                self.atom(pointer)
            ),
            Rhs::DynBoxNew(trait_id, implementation, value, boxed) => {
                let concrete = dyn_info(self.program, *trait_id).impls[*implementation].concrete;
                let pointer = if *boxed {
                    format!("k_box_{concrete}_take(&{})", self.atom(value))
                } else if matches!(self.program.layouts[concrete].kind, LayoutKind::Cell)
                    && self.atom_type(value) != Ty::Record(concrete)
                {
                    let inner = self.program.layouts[concrete].fields[0].ty;
                    format!(
                        "k_box_{concrete}_new(k_aggregate_{concrete}_tuple({}))",
                        self.take(value, inner)
                    )
                } else {
                    format!(
                        "k_box_{concrete}_new({})",
                        self.take(value, Ty::Record(concrete))
                    )
                };
                format!("(KDyn_{trait_id}) {{ (void *){pointer}, UINT32_C({implementation}) }}")
            }
            Rhs::DynCall(trait_id, method, receiver, arguments) => {
                let info = dyn_info(self.program, *trait_id);
                let mut values = vec![self.atom(receiver)];
                for (atom, (ty, passing)) in arguments.iter().zip(&info.methods[*method].parameters)
                {
                    values.push(if *passing == Passing::Move {
                        self.take(atom, *ty)
                    } else if *passing == Passing::Mut && !matches!(ty, Ty::Ref(_)) {
                        format!("&{}", self.atom(atom))
                    } else if copies(self.program, *ty) {
                        format!("k_{}_copy({})", suffix(*ty), self.atom(atom))
                    } else {
                        self.atom(atom)
                    });
                }
                format!("k_dyn_{trait_id}_{method}({})", values.join(", "))
            }
            Rhs::BoxNew(id, value, ty) => {
                let owned = self.take(value, *ty);
                if matches!(self.program.layouts[*id].kind, LayoutKind::Cell) {
                    format!("k_box_{id}_new(k_aggregate_{id}_tuple({owned}))")
                } else {
                    format!("k_box_{id}_new({owned})")
                }
            }
            Rhs::BoxUnwrap(id, value) => format!("k_box_{id}_unwrap(&{})", self.atom(value)),
            Rhs::ErrorBox(id, atom, borrowed) => {
                if *borrowed {
                    format!(
                        "(KError) {{ NULL, UINT64_C({}), {} }}",
                        id + 1,
                        self.atom(atom)
                    )
                } else {
                    format!("k_error_box_{id}({})", self.take(atom, Ty::Record(*id)))
                }
            }

            Rhs::Aggregate(id, case, values) => {
                let fields = if let Some(case) = case {
                    let cases = self.program.layouts[*id].kind.cases().unwrap();
                    cases[*case].fields.clone()
                } else {
                    self.program.layouts[*id]
                        .fields
                        .iter()
                        .map(|f| f.ty)
                        .collect()
                };
                let arguments = values
                    .iter()
                    .zip(fields)
                    .map(|((atom, input), target)| {
                        let unshared = matches!(atom, Atom::Slot(slot) if self.consumable.contains(slot))
                            && self.atom_type(atom) == *input;
                        let owned = if unshared {
                            // Nothing else reads this temporary, so the new value takes it.
                            self.take(atom, *input)
                        } else if copies(self.program, *input) {
                            format!("k_{}_copy({})", suffix(*input), self.atom(atom))
                        } else {
                            self.take(atom, *input)
                        };
                        if let Ty::Boxed(child) = target
                            && *input == Ty::Record(child)
                        {
                            return format!("k_box_{child}_new({owned})");
                        }
                        owned
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "k_aggregate_{id}_{}({arguments})",
                    case.map(|c| c.to_string())
                        .unwrap_or_else(|| "tuple".into())
                )
            }
            Rhs::Join(atoms) => {
                if atoms.is_empty() {
                    "k_empty".into()
                } else {
                    format!(
                        "k_string_join({}, (KString[]) {{ {} }})",
                        atoms.len(),
                        atoms
                            .iter()
                            .map(|a| self.atom(a))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            }
        })
    }
    fn line(&mut self, depth: usize, text: &str) {
        writeln!(self.output, "{}{text}", "    ".repeat(depth)).unwrap();
    }
    fn drop_slots(&mut self, slots: &[Slot], depth: usize) {
        for slot in slots.iter().rev() {
            let ty = self.ty(*slot);
            if ty.owns() {
                self.line(
                    depth,
                    &format!("k_{}_drop(&{});", suffix(ty), self.slot(*slot)),
                );
            }
        }
    }
    fn task_captures<'a>(&self, rhs: &'a Rhs) -> Vec<(&'a Atom, Ty, Passing)> {
        match rhs {
            Rhs::Call(target, args) => args
                .iter()
                .zip(&self.program.functions[*target].parameters)
                .map(|((atom, mode), (_, ty, _))| (atom, *ty, *mode))
                .collect(),
            Rhs::Builtin(name, _, args) => args
                .iter()
                .enumerate()
                .map(|(i, atom)| {
                    (
                        atom,
                        self.atom_type(atom),
                        if name.ends_with(".send") && i == 1 {
                            Passing::Move
                        } else {
                            Passing::Borrow
                        },
                    )
                })
                .collect(),
            _ => unreachable!("checked task invocation"),
        }
    }
    fn has_error(&self, layout: usize, value: &str) -> String {
        let LayoutKind::Union(parts, cases) = &self.program.layouts[layout].kind else {
            return "false".into();
        };
        let cases = parts.iter().zip(cases).enumerate().filter(|(_, (ty,case))| **ty == crate::types::Type::named("Error") || case.fields.iter().any(|field|matches!(field,Ty::Record(id) | Ty::Ref(id) if self.program.layouts[*id].is_error))).map(|(case,_)|format!("{value}.tag == {}",case+1)).collect::<Vec<_>>();
        if cases.is_empty() {
            "false".into()
        } else {
            cases.join(" || ")
        }
    }
    fn cleanups(&mut self, actions: &[Cleanup], depth: usize, result: bool) -> Result<(), String> {
        for action in actions {
            match action {
                Cleanup::Drop(slot) => self.drop_slots(&[*slot], depth),
                Cleanup::File(owner) => {
                    let Ty::Ref(id) = self.atom_type(owner) else {
                        return Err("File cleanup needs a view".into());
                    };
                    self.line(depth, "{");
                    self.line(
                        depth + 1,
                        &format!(
                            "KError k_cleanup = k_resource_{id}_close({});",
                            self.atom(owner)
                        ),
                    );
                    if result {
                        if let Ty::Record(layout) = self.function.result {
                            let LayoutKind::Union(parts, _) = &self.program.layouts[layout].kind
                            else {
                                return Err("A fallible cleanup needs an Error return".into());
                            };
                            let error = parts
                                .iter()
                                .position(|p| *p == crate::types::Type::named("Error"))
                                .ok_or("A fallible cleanup needs Error")?;
                            self.line(depth + 1, &format!("if (k_cleanup.type && !({})) k_record_{layout}_set(&k_result, k_aggregate_{layout}_{error}(k_error_take(&k_cleanup)));", self.has_error(layout,"k_result")));
                        } else if self.function.result != Ty::Error {
                            return Err("A fallible cleanup needs an Error return".into());
                        }
                    } else {
                        self.line(depth + 1, "if (k_cleanup.type) k_fatal(\"Cannot close a file while leaving a loop\");");
                    }
                    self.line(depth + 1, "k_error_drop(&k_cleanup);");
                    self.line(depth, "}");
                }
                Cleanup::Join(slot, fails) => {
                    self.line(depth, "{");
                    self.line(
                        depth + 1,
                        &format!("KError k_cleanup = k_tasks_join(&{});", self.slot(*slot)),
                    );
                    if *fails && result {
                        if let Ty::Record(layout) = self.function.result {
                            let LayoutKind::Union(parts, _) = &self.program.layouts[layout].kind
                            else {
                                return Err("A fallible task scope needs an Error return".into());
                            };
                            let error = parts
                                .iter()
                                .position(|p| *p == crate::types::Type::named("Error"))
                                .ok_or("A fallible task scope needs Error")?;
                            self.line(depth+1,&format!("if (k_cleanup.type && !({})) k_record_{layout}_set(&k_result,k_aggregate_{layout}_{error}(k_error_take(&k_cleanup)));",self.has_error(layout,"k_result")));
                        } else if self.function.result != Ty::Error {
                            return Err("A fallible task scope needs Error".into());
                        }
                    }
                    self.line(depth + 1, "k_error_drop(&k_cleanup);");
                    self.line(depth, "}");
                }
            }
        }
        Ok(())
    }
    fn statements(&mut self, code: &[Statement], depth: usize) -> Result<(), String> {
        for statement in code {
            match statement {
                Statement::Set(slot, rhs, loc) => {
                    let ty = self.ty(*slot);
                    let value = self.rhs(rhs, ty, loc)?;
                    if ty.owns() {
                        self.line(
                            depth,
                            &format!("k_{}_set(&{}, {value});", suffix(ty), self.slot(*slot)),
                        );
                    } else if ty == Ty::Unit {
                        self.line(depth, &format!("{value};"));
                    } else {
                        self.line(depth, &format!("{} = {value};", self.slot(*slot)));
                    }
                }
                Statement::Store(atom, ty, rhs, loc) => {
                    let value = self.rhs(rhs, *ty, loc)?;
                    if ty.owns() {
                        self.line(
                            depth,
                            &format!("k_{}_set(&{}, {value});", suffix(*ty), self.atom(atom)),
                        );
                    } else {
                        self.line(depth, &format!("{} = {value};", self.atom(atom)));
                    }
                }
                Statement::Evaluate(rhs, loc) => {
                    self.line(depth, &format!("{};", self.rhs(rhs, Ty::Unit, loc)?));
                }
                Statement::Print(ty, atom) => {
                    self.line(
                        depth,
                        &format!("k_print_{}({});", suffix(*ty), self.atom(atom)),
                    );
                }
                Statement::If(condition, left, right) => {
                    self.line(depth, &format!("if ({}) {{", self.atom(condition)));
                    self.statements(left, depth + 1)?;
                    self.line(depth, "} else {");
                    self.statements(right, depth + 1)?;
                    self.line(depth, "}");
                }
                Statement::Match(id, arms) => {
                    for arm in arms {
                        self.line(depth, "{");
                        self.statements(&arm.test, depth + 1)?;
                        self.line(depth + 1, &format!("if ({}) {{", self.atom(&arm.condition)));
                        self.statements(&arm.bindings, depth + 2)?;
                        let body_depth = if let Some((code, condition)) = &arm.guard {
                            self.statements(code, depth + 2)?;
                            self.line(depth + 2, &format!("if ({}) {{", self.atom(condition)));
                            depth + 3
                        } else {
                            depth + 2
                        };
                        self.statements(&arm.body, body_depth)?;
                        self.statements(&arm.cleanup, body_depth)?;
                        self.line(body_depth, &format!("goto k_match_{id};"));
                        if arm.guard.is_some() {
                            self.line(depth + 2, "}");
                        }
                        self.line(depth + 1, "}");
                        self.statements(&arm.cleanup, depth + 1)?;
                        self.line(depth, "}");
                    }
                    self.line(depth, "k_fatal(\"An exhaustive match reached no case\");");
                    self.line(depth, &format!("k_match_{id}: ;"));
                }
                Statement::Spawn {
                    id, scope, call, ..
                } => {
                    let captures = self.task_captures(call);
                    self.line(depth, "{");
                    self.line(depth+1,&format!("KTaskEnv_{id} *environment = malloc(sizeof *environment); if (!environment) k_fatal(\"Out of memory\");"));
                    if !self.program.python_sites.is_empty() {
                        self.line(
                            depth + 1,
                            "environment->python_callback = k_python_callback_depth != 0;",
                        );
                    }
                    for (i, (atom, ty, mode)) in captures.iter().enumerate() {
                        let value = if *mode == Passing::Mut {
                            format!("&{}", self.atom(atom))
                        } else if copies(self.program, *ty) {
                            format!("k_{}_copy({})", suffix(*ty), self.atom(atom))
                        } else if *mode == Passing::Move {
                            self.take(atom, *ty)
                        } else {
                            self.atom(atom)
                        };
                        self.line(depth + 1, &format!("environment->a{i} = {value};"));
                    }
                    self.line(
                        depth + 1,
                        &format!(
                            "k_tasks_start(&{},k_task_run_{id},environment);",
                            self.slot(*scope)
                        ),
                    );
                    self.line(depth, "}");
                }
                Statement::Loop {
                    id,
                    condition,
                    value,
                    body,
                    step,
                } => {
                    self.line(depth, "for (;;) {");
                    self.statements(condition, depth + 1)?;
                    self.line(
                        depth + 1,
                        &format!("if (!{}) goto k_break_{id};", self.atom(value)),
                    );
                    self.statements(body, depth + 1)?;
                    self.line(depth + 1, &format!("k_continue_{id}: ;"));
                    self.statements(step, depth + 1)?;
                    self.line(depth, "}");
                    self.line(depth, &format!("k_break_{id}: ;"));
                }
                Statement::Break(id, slots) | Statement::Continue(id, slots) => {
                    self.cleanups(slots, depth, false)?;
                    let jump = if matches!(statement, Statement::Break(..)) {
                        "break"
                    } else {
                        "continue"
                    };
                    self.line(depth, &format!("goto k_{jump}_{id};"));
                }
                Statement::Return(atom, cleanups) => {
                    self.line(depth, "{");
                    let ty = self.function.result;
                    if ty != Ty::Unit {
                        let through_view = matches!(
                            (ty, self.atom_type(atom)),
                            (Ty::Record(id), Ty::Ref(other) | Ty::Boxed(other)) if id == other
                        );
                        let handed_over = matches!(ty, Ty::Record(id) if self.program.copyable_layouts.contains(&id))
                            && cleanups.iter().all(|c| matches!(c, Cleanup::Drop(_)))
                            && matches!(atom, Atom::Slot(slot) if !slot.global
                                && self.ty(*slot) == ty
                                && self.function.locals[slot.id].owned
                                && !self.function.locals[slot.id].parameter
                                && cleanups.iter().any(|c| matches!(c, Cleanup::Drop(dropped) if dropped == slot)));
                        let value = if copies(self.program, ty) && through_view {
                            // A copyable result returned from a view is copied out of it.
                            format!("k_{}_copy(*({}))", suffix(ty), self.atom(atom))
                        } else if handed_over {
                            // This return drops the local next, so the result takes it whole.
                            self.take(atom, ty)
                        } else if copies(self.program, ty) {
                            format!("k_{}_copy({})", suffix(ty), self.atom(atom))
                        } else {
                            self.take(atom, ty)
                        };
                        self.line(depth + 1, &format!("{} k_result = {value};", c_type(ty)));
                    }
                    self.cleanups(cleanups, depth + 1, true)?;
                    self.line(
                        depth + 1,
                        if ty == Ty::Unit {
                            "return;"
                        } else {
                            "return k_result;"
                        },
                    );
                    self.line(depth, "}");
                }
                Statement::TailCall(target, args) => {
                    self.line(depth, "{");
                    let parameters = &self.program.functions[*target].parameters;
                    for (i, ((atom, mode), (_, ty, passing))) in
                        args.iter().zip(parameters).enumerate()
                    {
                        // A forwarded mut argument points at the caller's storage, not this frame.
                        let value = if *passing == Passing::Mut {
                            format!("&{}", self.atom(atom))
                        } else if (copies(self.program, *ty) || *ty == Ty::Python)
                            && *mode != Passing::Move
                        {
                            format!("k_{}_copy({})", suffix(*ty), self.atom(atom))
                        } else {
                            self.take(atom, *ty)
                        };
                        self.line(
                            depth + 1,
                            &format!("{} k_arg_{i} = {value};", parameter_type(*ty, *passing)),
                        );
                    }
                    let owned = self
                        .function
                        .locals
                        .iter()
                        .enumerate()
                        .filter(|(_, l)| l.owned)
                        .map(|(id, _)| Slot { id, global: false })
                        .collect::<Vec<_>>();
                    self.drop_slots(&owned, depth + 1);
                    let arguments = (0..args.len())
                        .map(|i| format!("k_arg_{i}"))
                        .collect::<Vec<_>>();
                    let group = tail_group(self.program, self.index);
                    if group.is_some() && group == tail_group(self.program, *target) {
                        for (i, argument) in arguments.iter().enumerate() {
                            self.line(
                                depth + 1,
                                &format!("k_state->u.f{target}.a{i} = {argument};"),
                            );
                        }
                        self.line(
                            depth + 1,
                            &format!("__attribute__((musttail)) return k_tail_{target}(k_state);"),
                        );
                    } else if group.is_none()
                        && tail_signature(self.function)
                            == tail_signature(&self.program.functions[*target])
                        && register_arguments(&self.program.functions[*target])
                    {
                        self.line(
                            depth + 1,
                            &format!(
                                "__attribute__((musttail)) return k_fn_{target}({});",
                                arguments.join(", ")
                            ),
                        );
                    } else {
                        // Outside any cycle, a call with another signature returns normally.
                        self.line(
                            depth + 1,
                            &format!("return k_fn_{target}({});", arguments.join(", ")),
                        );
                    }
                    self.line(depth, "}");
                }
                Statement::Drop(slots) => self.drop_slots(slots, depth),
            }
        }
        Ok(())
    }
}
fn tail_group(program: &Program, index: usize) -> Option<usize> {
    program
        .tail_groups
        .iter()
        .position(|members| members.contains(&index))
}
/// Whether every argument travels in a register. A larger struct, such as a String, goes
/// by a pointer to a copy in the caller's frame, which a `musttail` call has already
/// released when the callee reads it (found with AddressSanitizer, REVIEW2 section 4).
fn register_arguments(function: &Function) -> bool {
    function.parameters.iter().all(|(_, ty, mode)| {
        *mode == Passing::Mut
            || matches!(
                ty,
                Ty::Int | Ty::Nat | Ty::Double | Ty::Bool | Ty::Ref(_) | Ty::Pointer(_)
            )
    })
}
fn tail_signature(function: &Function) -> (Vec<(Ty, bool)>, Ty) {
    (
        function
            .parameters
            .iter()
            .map(|(_, ty, mode)| (*ty, *mode == Passing::Mut))
            .collect(),
        function.result,
    )
}
fn prototype(function: &Function, index: usize) -> String {
    let parameters = function
        .parameters
        .iter()
        .map(|(slot, ty, mode)| {
            format!(
                "{}{} v{}",
                c_type(*ty),
                if *mode == Passing::Mut { " *" } else { "" },
                slot.id
            )
        })
        .collect::<Vec<_>>();
    format!(
        "static {} k_fn_{index}({})",
        c_type(function.result),
        if parameters.is_empty() {
            "void".into()
        } else {
            parameters.join(", ")
        }
    )
}

pub fn emit_c(program: &Program) -> Result<String, String> {
    emit(program, false)
}

fn emit(program: &Program, session: bool) -> Result<String, String> {
    let python = !program.python_sites.is_empty();
    // Python.h must come before the standard headers.
    let mut output = if python {
        "#define PY_SSIZE_T_CLEAN\n#include <Python.h>\n".to_owned()
    } else {
        String::new()
    };
    if session {
        output.push_str("#define K_SESSION_RUNTIME\n#include \"runtime.h\"\n");
        if python {
            output.push_str("#include \"kestrel_python.h\"\n");
        }
    } else {
        output.push_str(include_str!("platform_runtime.c"));
        output.push_str(include_str!("runtime.c"));
        output.push_str(include_str!("blas_runtime.c"));
        output.push_str(include_str!("bigint_runtime.c"));
        output.push_str(include_str!("tensor_runtime.c"));
        output.push_str(include_str!("autograd_runtime.c"));
        output.push_str(include_str!("numerical_runtime.c"));
        output.push_str(include_str!("probability_runtime.c"));
        if python {
            output.push_str(include_str!("python_runtime.c"));
        }
        output.push_str(include_str!("unicode_data.c"));
        output.push_str(include_str!("unicode_runtime.c"));
        output.push_str(include_str!("grapheme_data.c"));
        output.push_str(include_str!("grapheme_runtime.c"));
        output.push_str(include_str!("sort_runtime.c"));
        output.push_str(include_str!("system_runtime.c"));
        output.push_str(include_str!("regex_runtime.c"));
    }
    for header in &program.c_headers {
        writeln!(output, "#include \"{header}\"").unwrap();
    }
    emit_c_text_calls(program, &mut output);
    emit_layouts(program, &mut output);
    emit_aggregate_copies(program, &mut output);
    for (group, members) in program.tail_groups.iter().enumerate() {
        writeln!(output, "typedef struct {{ union {{").unwrap();
        for member in members {
            let parameters = &program.functions[*member].parameters;
            write!(output, "    struct {{ ").unwrap();
            if parameters.is_empty() {
                write!(output, "char empty; ").unwrap();
            }
            for (i, (_, ty, mode)) in parameters.iter().enumerate() {
                write!(output, "{} a{i}; ", parameter_type(*ty, *mode)).unwrap();
            }
            writeln!(output, "}} f{member};").unwrap();
        }
        writeln!(output, "}} u; }} KTail_{group};").unwrap();
    }
    for (index, function) in program.functions.iter().enumerate() {
        writeln!(output, "{};", prototype(function, index)).unwrap();
        if let Some(group) = tail_group(program, index) {
            writeln!(
                output,
                "static {} k_tail_{index}(KTail_{group} *k_state);",
                c_type(function.result)
            )
            .unwrap();
        }
    }
    collections::emit(program, &mut output);
    maps::emit(program, &mut output);
    emit_trait_objects(program, &mut output, session);
    emit_closures(program, &mut output);
    emit_arrays(program, &mut output);
    emit_kesdas_csv(program, &mut output);
    emit_kesdas_order(program, &mut output);
    emit_kesdas_gather(program, &mut output);
    emit_kesdas_correlation(program, &mut output);
    emit_kesdas_arithmetic(program, &mut output);
    emit_kesdas_factor_codes(program, &mut output);
    emit_kesdas_column_codes(program, &mut output);
    emit_kesdas_merge_left(program, &mut output);
    emit_kesdas_top(program, &mut output);
    if session {
        output.push_str("static void **k_session_globals;\n");
    }
    for (id, ty) in program.globals.iter().enumerate() {
        if *ty != Ty::Unit {
            if session {
                writeln!(
                    output,
                    "#define g{id} (*(({} *)k_session_globals[{id}]))",
                    c_type(*ty)
                )
                .unwrap();
            } else {
                writeln!(output, "static {} g{id} = {};", c_type(*ty), initial(*ty)).unwrap();
            }
        }
    }
    emit_static_fields(program, &mut output, session);
    if let Some((runtime, layout)) = program
        .layouts
        .iter()
        .enumerate()
        .find(|(_, layout)| layout.source == crate::types::Type::named("std.core.RuntimeError"))
    {
        let error_name = if session {
            "k_cell_runtime_error"
        } else {
            "k_runtime_error"
        };
        writeln!(output, "static KError {error_name}(uint32_t tag, KString message) {{ KRecord_{runtime} value = {{0}}; value.tag = tag + 1; switch (tag) {{").unwrap();
        for case in 0..layout.kind.cases().unwrap().len() {
            writeln!(
                output,
                "case {case}: value.data.c{case}.f0 = message; break;"
            )
            .unwrap();
        }
        writeln!(output, "default: k_fatal(\"Unknown runtime error case\"); }} return k_error_box_{runtime}(value); }}").unwrap();
        emit_checked_operations(program, &mut output);
        emit_c_boundary(program, &mut output);
        emit_error_text(program, &mut output);
        python::emit(program, &mut output);
        resources::emit(program, &mut output);
    }
    emit_tasks(program, &mut output)?;
    if program.tests {
        emit_test_support(program, &mut output);
    }
    for (index, function) in program.functions.iter().enumerate() {
        writeln!(output, "{} {{", prototype(function, index)).unwrap();
        if let Some(group) = tail_group(program, index) {
            // The ordinary entry fills the cycle's argument block and runs the twin.
            writeln!(output, "    KTail_{group} k_state;").unwrap();
            for (i, (slot, _, _)) in function.parameters.iter().enumerate() {
                writeln!(output, "    k_state.u.f{index}.a{i} = v{};", slot.id).unwrap();
            }
            writeln!(output, "    return k_tail_{index}(&k_state);\n}}").unwrap();
            writeln!(
                output,
                "static {} k_tail_{index}(KTail_{group} *k_state) {{",
                c_type(function.result)
            )
            .unwrap();
            for (i, (slot, ty, mode)) in function.parameters.iter().enumerate() {
                writeln!(
                    output,
                    "    {} v{} = k_state->u.f{index}.a{i};",
                    parameter_type(*ty, *mode),
                    slot.id
                )
                .unwrap();
            }
        }
        for (id, local) in function.locals.iter().enumerate() {
            if !local.parameter && local.ty != Ty::Unit {
                writeln!(
                    output,
                    "    {} v{id} = {};",
                    c_type(local.ty),
                    initial(local.ty)
                )
                .unwrap();
            }
        }
        let mut emitter = Emitter {
            program,
            function,
            index,
            consumable: moves::consumable(program, function),
            output: String::new(),
        };
        emitter.statements(&function.body, 1)?;
        output.push_str(&emitter.output);
        output.push_str("}\n");
    }
    if session {
        session::entry_points(program, &mut output);
        return Ok(output);
    }
    output.push_str(
        "int main(int argc,char **argv) {\n    k_platform_start();\n    k_overflow_guard();\n",
    );
    // A test program's first argument selects the test; script arguments follow it.
    let first_argument = if program.tests {
        output.push_str("    if (argc < 2) { fputs(\"Run this test program through kestrel test.\\n\", stderr); return 2; }\n    k_test_index = strtoll(argv[1], NULL, 10);\n");
        2
    } else {
        1
    };
    if let Some(slot) = program.process_args {
        let Ty::Record(id) = program.globals[slot.id] else {
            unreachable!()
        };
        writeln!(output,"    for(int i={first_argument};i<argc;++i) {{KString arg=k_string_bytes(argv[i],strlen(argv[i]));if(!k_valid_utf8(arg)) k_fatal(\"A command-line argument is not valid UTF-8\");k_list_{id}_append(&g{},arg);}}",slot.id).unwrap();
    }
    if program.statics.iter().any(|field| field.mutable) {
        output.push_str("    k_static_init_all();\n");
    }
    output.push_str("    k_fn_0();\n");
    if let Some(slot) = program.process_args {
        writeln!(
            output,
            "    k_{}_drop(&g{});",
            suffix(program.globals[slot.id]),
            slot.id
        )
        .unwrap();
    }
    output.push_str("    if (fflush(stdout) == EOF) k_fatal(\"Cannot write standard output\");\n    return 0;\n}\n");
    Ok(output)
}

fn emit_tasks(program: &Program, output: &mut String) -> Result<(), String> {
    fn collect<'a>(code: &'a [Statement], tasks: &mut Vec<&'a Statement>) {
        for statement in code {
            match statement {
                Statement::Spawn { .. } => tasks.push(statement),
                Statement::If(_, a, b) => {
                    collect(a, tasks);
                    collect(b, tasks);
                }
                Statement::Match(_, arms) => {
                    for arm in arms {
                        collect(&arm.test, tasks);
                        if let Some((code, _)) = &arm.guard {
                            collect(code, tasks);
                        }
                        collect(&arm.body, tasks);
                    }
                }
                Statement::Loop {
                    condition,
                    body,
                    step,
                    ..
                } => {
                    collect(condition, tasks);
                    collect(body, tasks);
                    collect(step, tasks);
                }
                _ => {}
            }
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    for parent in &program.functions {
        let source = Emitter {
            program,
            function: parent,
            index: usize::MAX,
            consumable: Default::default(),
            output: String::new(),
        };
        let mut tasks = vec![];
        collect(&parent.body, &mut tasks);
        for task in tasks {
            let Statement::Spawn {
                id,
                call,
                result,
                location,
                ..
            } = task
            else {
                unreachable!()
            };
            if !seen.insert(*id) {
                continue;
            }
            let captures = source.task_captures(call);
            writeln!(output, "typedef struct KTaskEnv_{id} {{").unwrap();
            if !program.python_sites.is_empty() {
                writeln!(output, "    bool python_callback;").unwrap();
            }
            if captures.is_empty() {
                writeln!(output, "    unsigned char empty;").unwrap();
            }
            for (i, (_, ty, mode)) in captures.iter().enumerate() {
                writeln!(
                    output,
                    "    {}{} a{i};",
                    c_type(*ty),
                    if *mode == Passing::Mut { " *" } else { "" }
                )
                .unwrap();
            }
            writeln!(output,"}} KTaskEnv_{id};\nstatic KError k_task_run_{id}(void *payload) {{\n    KTaskEnv_{id} *environment = payload;").unwrap();
            if !program.python_sites.is_empty() {
                writeln!(output, "    size_t previous_callback = k_python_callback_depth;\n    k_python_callback_depth = environment->python_callback ? 1 : 0;").unwrap();
            }
            let mut function = Function {
                name: format!("task_{id}"),
                parameters: vec![],
                result: *result,
                locals: vec![],
                body: vec![],
            };
            for (i, (_, ty, mode)) in captures.iter().enumerate() {
                function.locals.push(crate::core::Local {
                    scope: 0,
                    ty: *ty,
                    parameter: true,
                    owned: ty.owns() && *mode != Passing::Mut,
                });
                function.parameters.push((
                    Slot {
                        id: i,
                        global: false,
                    },
                    *ty,
                    *mode,
                ));
                writeln!(
                    output,
                    "    {}{} v{i} = environment->a{i};",
                    c_type(*ty),
                    if *mode == Passing::Mut { " *" } else { "" }
                )
                .unwrap();
            }
            writeln!(output, "    free(environment);").unwrap();
            let call = match call {
                Rhs::Call(target, args) => Rhs::Call(
                    *target,
                    args.iter()
                        .enumerate()
                        .map(|(i, (_, mode))| {
                            (
                                Atom::Slot(Slot {
                                    id: i,
                                    global: false,
                                }),
                                *mode,
                            )
                        })
                        .collect(),
                ),
                Rhs::Builtin(name, input, args) => Rhs::Builtin(
                    name.clone(),
                    *input,
                    args.iter()
                        .enumerate()
                        .map(|(i, _)| {
                            Atom::Slot(Slot {
                                id: i,
                                global: false,
                            })
                        })
                        .collect(),
                ),
                _ => unreachable!(),
            };
            let mut emitter = Emitter {
                program,
                function: &function,
                index: usize::MAX,
                consumable: Default::default(),
                output: String::new(),
            };
            let expression = emitter.rhs(&call, *result, location)?;
            if *result == Ty::Unit {
                writeln!(output, "    {expression};").unwrap();
            } else {
                writeln!(output, "    {} result = {expression};", c_type(*result)).unwrap();
            }
            writeln!(output, "    KError error = {{0}};").unwrap();
            if let Ty::Record(layout) = result {
                let LayoutKind::Union(parts, cases) = &program.layouts[*layout].kind else {
                    return Err("Task result needs a void/Error union".into());
                };
                for (case, part) in parts.iter().enumerate() {
                    if *part == crate::types::Type::named("void") {
                        continue;
                    }
                    let native = cases[case].fields[0];
                    let value = format!("result.data.c{case}.f0");
                    let error = match native {
                        Ty::Error => format!("k_error_take(&{value})"),
                        Ty::Record(error_id) => {
                            format!("k_error_box_{error_id}(k_record_{error_id}_take(&{value}))")
                        }
                        Ty::Ref(error_id) => {
                            format!("(KError) {{NULL,UINT64_C({}),{value}}}", error_id + 1)
                        }
                        _ => return Err("Unsupported task error layout".into()),
                    };
                    writeln!(
                        output,
                        "    if (result.tag == {}) error = {error};",
                        case + 1
                    )
                    .unwrap();
                }
                writeln!(output, "    k_record_{layout}_drop(&result);").unwrap();
            }
            let owned = function
                .locals
                .iter()
                .enumerate()
                .filter(|(_, l)| l.owned)
                .map(|(id, _)| Slot { id, global: false })
                .collect::<Vec<_>>();
            emitter.drop_slots(&owned, 1);
            output.push_str(&emitter.output);
            if !program.python_sites.is_empty() {
                writeln!(output, "    k_python_callback_depth = previous_callback;").unwrap();
            }
            writeln!(output, "    return error;\n}}").unwrap();
        }
    }
    Ok(())
}

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
pub struct TemporaryDirectory {
    path: PathBuf,
}
impl TemporaryDirectory {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn new(parent: &Path) -> io::Result<Self> {
        for _ in 0..128 {
            let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(".kestrel-build-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Cannot reserve a native build directory",
        ))
    }
}
impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Compile beside the destination, replacing an existing artifact only after Clang succeeds.
/// Compiler and linker flags for the embedded interpreter, from the installed python3-config.
fn python_flags(program: &Program) -> Result<Vec<String>, String> {
    if program.python_sites.is_empty() {
        return Ok(vec![]);
    }
    if cfg!(windows) {
        return Err("Python interop is not supported on Windows yet: it embeds CPython through python3-config, which Windows installations do not provide.".into());
    }
    let mut flags = vec![];
    for arguments in [&["--includes"][..], &["--ldflags", "--embed"][..]] {
        let output = Command::new("python3-config")
            .args(arguments)
            .output()
            .map_err(|e| format!("Cannot start python3-config: {e}. Python interop needs a CPython installation with its development files."))?;
        if !output.status.success() {
            return Err("python3-config could not describe the Python installation.".into());
        }
        for flag in String::from_utf8_lossy(&output.stdout).split_whitespace() {
            // Python's headers are outside this program's warning policy.
            match flag.strip_prefix("-I") {
                Some(folder) => flags.extend(["-isystem".to_owned(), folder.to_owned()]),
                None => flags.push(flag.to_owned()),
            }
        }
    }
    Ok(flags)
}

/// The file `build` writes for a requested output path. Windows runs only files with an
/// executable extension, so a path without one gains `.exe` there.
pub fn executable_path(destination: &Path) -> PathBuf {
    if cfg!(windows) && destination.extension().is_none() {
        destination.with_extension("exe")
    } else {
        destination.to_path_buf()
    }
}

/// The manifest that makes a Windows program use UTF-8 for its command line, environment
/// and file names, as Kestrel strings do.
const WINDOWS_MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly manifestVersion="1.0" xmlns="urn:schemas-microsoft-com:asm.v1">
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    </windowsSettings>
  </application>
</assembly>
"#;

/// The system libraries a program's emitted C links: the math library on Unix. The
/// Windows C runtime includes it.
pub fn system_libraries() -> &'static [&'static str] {
    if cfg!(windows) { &[] } else { &["-lm"] }
}

/// A compiled resource file (`.res`) holding the manifest as resource type 24, id 1.
/// The linker reads it directly, so the build needs no resource compiler.
fn manifest_resource(manifest: &str) -> Vec<u8> {
    fn entry(out: &mut Vec<u8>, data: &[u8], kind: u16, name: u16, language: u16) {
        let size = u32::try_from(data.len()).expect("a small manifest");
        out.extend(size.to_le_bytes());
        out.extend(32u32.to_le_bytes());
        out.extend([0xFF, 0xFF]);
        out.extend(kind.to_le_bytes());
        out.extend([0xFF, 0xFF]);
        out.extend(name.to_le_bytes());
        out.extend(0u32.to_le_bytes());
        out.extend(0x0030u16.to_le_bytes());
        out.extend(language.to_le_bytes());
        out.extend(0u32.to_le_bytes());
        out.extend(0u32.to_le_bytes());
        out.extend(data);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
    }
    let mut out = vec![];
    // Every .res file opens with an empty entry.
    entry(&mut out, &[], 0, 0, 0);
    entry(&mut out, manifest.as_bytes(), 24, 1, 0x0409);
    out
}

/// Flags for a program's link step on this platform. Windows gives the main thread the
/// 8 MiB stack that Linux and macOS give it, and embeds the UTF-8 manifest.
fn platform_link_flags(folder: &Path) -> Result<Vec<std::ffi::OsString>, String> {
    if !cfg!(windows) {
        return Ok(system_libraries().iter().map(Into::into).collect());
    }
    let resource = folder.join("manifest.res");
    fs::write(&resource, manifest_resource(WINDOWS_MANIFEST)).map_err(|e| e.to_string())?;
    if mingw() {
        // The MinGW linker takes GNU-style flags, reads the .res file as an input, and
        // writes no manifest of its own.
        return Ok(vec![
            "-Wl,--stack,8388608".into(),
            resource.into_os_string(),
        ]);
    }
    Ok(vec![
        "-Xlinker".into(),
        "/STACK:8388608".into(),
        "-Xlinker".into(),
        "/MANIFEST:NO".into(),
        "-Xlinker".into(),
        resource.into_os_string(),
    ])
}

/// On Windows a sanitizer build loads Clang's sanitizer runtime DLL, which lives in
/// Clang's runtime folder rather than on `PATH`. This is `PATH` with that folder first.
/// Other platforms link the runtime by full path and need nothing.
pub fn sanitizer_runtime_path() -> Option<std::ffi::OsString> {
    if !cfg!(windows) {
        return None;
    }
    // Clang releases keep the DLL in the per-target runtime folder or in lib/windows under
    // the resource folder, depending on how LLVM was built; put both first.
    let ask = |flag: &str| {
        let output = clang_command().arg(flag).output().ok()?;
        Some(PathBuf::from(String::from_utf8(output.stdout).ok()?.trim()))
    };
    let mut folders = vec![];
    folders.extend(ask("-print-runtime-dir"));
    let resource = ask("-print-resource-dir");
    folders.extend(resource.iter().map(|root| root.join("lib").join("windows")));
    // llvm-mingw keeps its runtime DLLs in the target folder at the toolchain root,
    // three levels above lib/clang/<version>.
    folders.extend(resource.iter().filter_map(|root| {
        Some(
            root.parent()?
                .parent()?
                .parent()?
                .join("x86_64-w64-mingw32")
                .join("bin"),
        )
    }));
    let rest = std::env::var_os("PATH").unwrap_or_default();
    std::env::join_paths(folders.into_iter().chain(std::env::split_paths(&rest))).ok()
}

/// What Clang and the linker said about a failed build. The MSVC linker writes its
/// errors to standard output, so both streams count.
fn compiler_messages(output: &std::process::Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text
}

/// Clang flags every C compile shares. Windows Clang has no -pthread: threads come from
/// the Win32 API there.
pub fn thread_flags() -> &'static [&'static str] {
    if cfg!(windows) { &[] } else { &["-pthread"] }
}

pub fn build(program: &Program, destination: &Path) -> Result<(), String> {
    let destination = &executable_path(destination);
    let source = emit_c(program)?;
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temp = TemporaryDirectory::new(parent).map_err(|e| e.to_string())?;
    let c_path = temp.path.join("program.c");
    let binary = executable_path(&temp.path.join("program"));
    fs::write(&c_path, source).map_err(|e| e.to_string())?;
    let includes = program
        .c_include_dirs
        .iter()
        .flat_map(|d| [std::ffi::OsStr::new("-I"), d.as_os_str()])
        .collect::<Vec<_>>();
    // Project C files keep their own warning policy, so they compile apart from program.c.
    let mut objects = vec![];
    for (index, source) in program.c_sources.iter().enumerate() {
        let object = temp.path.join(format!("source{index}.o"));
        let output = clang_command()
            .args(["-std=c11", "-O3", "-c"])
            .args(thread_flags())
            .arg(source)
            .args(&includes)
            .arg("-o")
            .arg(&object)
            .output()
            .map_err(|e| {
                format!("Cannot start Clang: {e}. Install a C toolchain with clang on PATH.")
            })?;
        if !output.status.success() {
            return Err(format!(
                "Compiling {} failed:\n{}",
                source.display(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        objects.push(object);
    }
    let output = clang_command()
        .args(thread_flags())
        .args([
            "-std=c11",
            "-O3",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-Wno-unused-function",
            "-Wno-unused-parameter",
            "-Wno-unused-variable",
            "-Wno-unused-but-set-variable",
            "-Wno-unused-label",
        ])
        .arg(&c_path)
        .args(&objects)
        .args(&includes)
        .args(python_flags(program)?)
        // Leak checks need every block from malloc, so the collection pool is off for them.
        .args(
            ["KESTREL_LEAK_CHECK", "KESTREL_NO_POOL"]
                .iter()
                .any(|name| std::env::var_os(name).is_some())
                .then_some("-DK_NO_POOL"),
        )
        .args(program.c_libraries.iter().map(|name| format!("-l{name}")))
        .args(platform_link_flags(&temp.path)?)
        .arg("-o")
        .arg(&binary)
        .output()
        .map_err(|e| {
            format!("Cannot start Clang: {e}. Install a C toolchain with clang on PATH.")
        })?;
    if !output.status.success() {
        let detail = compiler_messages(&output);
        let hint = if detail.contains("Undefined symbols")
            || detail.contains("undefined reference")
            || detail.contains("unresolved external symbol")
        {
            "\nA C function's library may be missing; name it with --link, as in --link z."
        } else {
            ""
        };
        return Err(format!("Native compilation failed:\n{detail}{hint}"));
    }
    fs::rename(&binary, destination)
        .map_err(|e| format!("Cannot save {}: {e}", destination.display()))
}

pub fn write_c(program: &Program, destination: &Path) -> Result<(), String> {
    let source = emit_c(program)?;
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temp = TemporaryDirectory::new(parent).map_err(|e| e.to_string())?;
    let source_path = temp.path.join("program.c");
    fs::write(&source_path, source).map_err(|e| e.to_string())?;
    fs::rename(&source_path, destination)
        .map_err(|e| format!("Cannot write {}: {e}", destination.display()))
}
pub fn run(program: &Program) -> Result<ExitStatus, String> {
    run_with_args(program, &[])
}

pub fn run_with_args(program: &Program, arguments: &[String]) -> Result<ExitStatus, String> {
    let temp = TemporaryDirectory::new(&std::env::temp_dir()).map_err(|e| e.to_string())?;
    let binary = executable_path(&temp.path.join("program"));
    build(program, &binary)?;
    Command::new(&binary)
        .args(arguments)
        .status()
        .map_err(|e| format!("Cannot execute compiled program: {e}"))
}

/// Copy a static field value out of its storage. Counted values retain their
/// storage; scalars, C widths, pointers and arena handles copy by value.
pub(super) fn static_copy(program: &Program, ty: Ty, expr: &str) -> String {
    match ty {
        Ty::String => format!("k_string_copy({expr})"),
        Ty::Record(id) if copies(program, ty) => {
            format!("k_record_{id}_copy({expr})")
        }
        _ => expr.to_owned(),
    }
}

/// Drop a stored static field value before a write replaces it.
fn static_drop(program: &Program, ty: Ty, expr: &str) -> Option<String> {
    match ty {
        Ty::String => Some(format!("k_string_drop(&{expr});")),
        Ty::Record(id)
            if matches!(
                program.layouts[id].kind,
                LayoutKind::List(_) | LayoutKind::Map(_)
            ) =>
        {
            Some(format!("k_record_{id}_drop(&{expr});"))
        }
        _ => None,
    }
}

/// Static final fields use a lazy initialized slot. Static vars use a mutex
/// around a value initialized before the program's top-level code runs.
fn static_key(key: &str) -> String {
    let mut hex = String::with_capacity(key.len() * 2);
    for byte in key.as_bytes() {
        use std::fmt::Write as _;
        write!(hex, "{byte:02x}").unwrap();
    }
    hex
}

fn emit_static_fields(program: &Program, output: &mut String, session: bool) {
    for (id, field) in program.statics.iter().enumerate() {
        let ty = c_type(field.ty);
        if field.mutable && session {
            let key = static_key(&field.key);
            writeln!(
                output,
                "static pthread_once_t k_static_once_{id} = PTHREAD_ONCE_INIT;\nstatic KSessionStatic *k_static_cell_{id};\nstatic void k_static_init_{id}(void *slot) {{ *(({ty} *)slot) = k_fn_{}(); }}\nstatic void k_static_attach_once_{id}(void) {{ k_static_cell_{id} = k_session_static(\"{key}\", sizeof({ty}), k_static_init_{id}); }}\nstatic void k_static_attach_{id}(void) {{ k_pthread_check(pthread_once(&k_static_once_{id}, k_static_attach_once_{id})); }}",
                field.function
            )
            .unwrap();
            let copied = static_copy(
                program,
                field.ty,
                &format!("*(({ty} *)k_static_cell_{id}->value)"),
            );
            writeln!(
                output,
                "static {ty} k_static_read_{id}(void) {{\n    k_static_attach_{id}();\n    k_pthread_check(pthread_mutex_lock(&k_static_cell_{id}->mutex));\n    {ty} value = {copied};\n    k_pthread_check(pthread_mutex_unlock(&k_static_cell_{id}->mutex));\n    return value;\n}}"
            )
            .unwrap();
            let drop = static_drop(
                program,
                field.ty,
                &format!("*(({ty} *)k_static_cell_{id}->value)"),
            )
            .unwrap_or_default();
            writeln!(
                output,
                "static void k_static_write_{id}({ty} value) {{\n    k_static_attach_{id}();\n    k_pthread_check(pthread_mutex_lock(&k_static_cell_{id}->mutex));\n    {drop}\n    *(({ty} *)k_static_cell_{id}->value) = value;\n    k_pthread_check(pthread_mutex_unlock(&k_static_cell_{id}->mutex));\n}}"
            )
            .unwrap();
            continue;
        }
        writeln!(output, "static {ty} k_static_value_{id};").unwrap();
        writeln!(
            output,
            "static void k_static_init_{id}(void) {{ k_static_value_{id} = k_fn_{}(); }}",
            field.function
        )
        .unwrap();
        if field.mutable {
            writeln!(
                output,
                "static pthread_mutex_t k_static_mutex_{id} = PTHREAD_MUTEX_INITIALIZER;"
            )
            .unwrap();
            let copied = static_copy(program, field.ty, &format!("k_static_value_{id}"));
            writeln!(
                output,
                "static {ty} k_static_read_{id}(void) {{\n    k_pthread_check(pthread_mutex_lock(&k_static_mutex_{id}));\n    {ty} value = {copied};\n    k_pthread_check(pthread_mutex_unlock(&k_static_mutex_{id}));\n    return value;\n}}"
            )
            .unwrap();
            let drop =
                static_drop(program, field.ty, &format!("k_static_value_{id}")).unwrap_or_default();
            writeln!(
                output,
                "static void k_static_write_{id}({ty} value) {{\n    k_pthread_check(pthread_mutex_lock(&k_static_mutex_{id}));\n    {drop}\n    k_static_value_{id} = value;\n    k_pthread_check(pthread_mutex_unlock(&k_static_mutex_{id}));\n}}"
            )
            .unwrap();
        } else {
            writeln!(output, "static atomic_int k_static_state_{id};").unwrap();
            writeln!(output, "static pthread_t k_static_owner_{id};").unwrap();
            let copied = static_copy(program, field.ty, &format!("k_static_value_{id}"));
            writeln!(
                output,
                "static {ty} k_static_read_{id}(void) {{\n    if (atomic_load_explicit(&k_static_state_{id}, memory_order_acquire) != 2) {{\n        pthread_t me = pthread_self();\n        int expected = 0;\n        if (atomic_compare_exchange_strong_explicit(&k_static_state_{id}, &expected, 1, memory_order_acq_rel, memory_order_relaxed)) {{\n            k_static_owner_{id} = me;\n            k_static_init_{id}();\n            atomic_store_explicit(&k_static_state_{id}, 2, memory_order_release);\n        }} else if (pthread_equal(k_static_owner_{id}, me)) {{\n            k_fatal(\"Static initializer refers to itself\");\n        }} else {{\n            while (atomic_load_explicit(&k_static_state_{id}, memory_order_acquire) != 2) k_cpu_relax();\n        }}\n    }}\n    return {copied};\n}}"
            )
            .unwrap();
        }
    }
    let mutable = program
        .statics
        .iter()
        .enumerate()
        .filter(|(_, field)| field.mutable)
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    if !mutable.is_empty() {
        writeln!(output, "static void k_static_init_all(void) {{").unwrap();
        for id in mutable {
            if session {
                writeln!(output, "    k_static_attach_{id}();").unwrap();
            } else {
                writeln!(output, "    k_static_init_{id}();").unwrap();
            }
        }
        writeln!(output, "}}").unwrap();
    }
}

fn emit_layouts(program: &Program, output: &mut String) {
    fn payloads(layout: &crate::core::Layout) -> Vec<Vec<Ty>> {
        match &layout.kind {
            LayoutKind::Enum(cases)
            | LayoutKind::Step(cases)
            | LayoutKind::Union(_, cases)
            | LayoutKind::ClassView(cases) => cases.iter().map(|c| c.fields.clone()).collect(),
            _ => vec![layout.fields.iter().map(|f| f.ty).collect()],
        }
    }
    for id in 0..program.layouts.len() {
        writeln!(output, "typedef struct KRecord_{id} KRecord_{id};").unwrap();
    }
    for (id, layout) in program.layouts.iter().enumerate() {
        if matches!(layout.kind, LayoutKind::Trait(_)) {
            writeln!(
                output,
                "typedef struct {{ void *data; uint32_t impl; }} KDyn_{id};"
            )
            .unwrap();
            writeln!(output, "static void k_dynbox_{id}_drop(KDyn_{id} *value);").unwrap();
            writeln!(
                output,
                "static void k_dynbox_{id}_drop_task(void *payload, KDropStack *stack);"
            )
            .unwrap();
        }
    }
    fn declare(
        program: &Program,
        id: usize,
        done: &mut std::collections::BTreeSet<usize>,
        output: &mut String,
    ) {
        if !done.insert(id) {
            return;
        }
        for ty in payloads(&program.layouts[id]).into_iter().flatten() {
            if let Ty::Record(child) = ty {
                declare(program, child, done, output);
            }
        }
        writeln!(output, "struct KRecord_{id} {{").unwrap();
        if matches!(program.layouts[id].kind, LayoutKind::BigInt) {
            writeln!(output, "    KBigInt value;").unwrap();
        } else if matches!(program.layouts[id].kind, LayoutKind::Tensor) {
            writeln!(output, "    KTensor value;").unwrap();
        } else if let LayoutKind::Closure { parameters, result } = &program.layouts[id].kind {
            let types = std::iter::once("KClosure *".to_owned())
                .chain(
                    parameters
                        .iter()
                        .map(|(ty, mode)| parameter_type(*ty, *mode)),
                )
                .collect::<Vec<_>>();
            writeln!(
                output,
                "    KClosure *env;\n    {} (*code)({});",
                c_type(*result),
                types.join(", ")
            )
            .unwrap();
        } else if let LayoutKind::Env(by_ref) = &program.layouts[id].kind {
            writeln!(output, "    KClosure header;").unwrap();
            for (i, field) in program.layouts[id].fields.iter().enumerate() {
                let pointer = if by_ref[i] { " *" } else { "" };
                writeln!(output, "    {}{pointer} f{i};", c_type(field.ty)).unwrap();
            }
        } else if matches!(program.layouts[id].kind, LayoutKind::Map(_)) {
            writeln!(output, "    struct KMapData_{id} *handle;").unwrap();
        } else if matches!(program.layouts[id].kind, LayoutKind::List(_)) {
            writeln!(output, "    struct KListData_{id} *handle;").unwrap();
        } else if let LayoutKind::Array(element) = program.layouts[id].kind {
            writeln!(
                output,
                "    int64_t length;\n    {} *items;\n    int64_t capacity;",
                c_type(element)
            )
            .unwrap();
        } else if let LayoutKind::Resource(resource) = program.layouts[id].kind {
            resources::declare(program, id, resource, output);
        } else if let Some(cases) = program.layouts[id].kind.cases() {
            if matches!(program.layouts[id].kind, LayoutKind::Step(_)) {
                writeln!(output, "    KOwner *owner;").unwrap();
            }
            if enum_niche(&program.layouts[id]).is_some() {
                for (i, ty) in cases[1].fields.iter().enumerate() {
                    writeln!(output, "    {} f{i};", c_type(*ty)).unwrap();
                }
            } else {
                writeln!(output, "    uint32_t tag;\n    union {{").unwrap();
                for (case, variant) in cases.iter().enumerate() {
                    writeln!(output, "        struct {{").unwrap();
                    if variant.fields.is_empty() {
                        writeln!(output, "            unsigned char empty;").unwrap();
                    }
                    for (i, ty) in variant.fields.iter().enumerate() {
                        writeln!(output, "            {} f{i};", c_type(*ty)).unwrap();
                    }
                    writeln!(output, "        }} c{case};").unwrap();
                }
                writeln!(output, "    }} data;").unwrap();
            }
        } else {
            if program.layouts[id].fields.is_empty() {
                writeln!(output, "    unsigned char empty;").unwrap();
            }
            for (i, field) in program.layouts[id].fields.iter().enumerate() {
                writeln!(output, "    {} f{i};", c_type(field.ty)).unwrap();
            }
        }
        writeln!(output, "}};").unwrap();
    }
    let mut done = std::collections::BTreeSet::new();
    for id in 0..program.layouts.len() {
        declare(program, id, &mut done, output);
    }
    for id in &program.copyable_layouts {
        writeln!(output, "typedef struct {{ atomic_size_t references; KRecord_{id} value; }} KSharedBox_{id};\nstatic KSharedBox_{id} *k_box_{id}_header(KRecord_{id} *value) {{ return (KSharedBox_{id} *)((char *)value - offsetof(KSharedBox_{id}, value)); }}\nstatic KRecord_{id} *k_box_{id}_copy(KRecord_{id} *value) {{ if (value && atomic_fetch_add_explicit(&k_box_{id}_header(value)->references, 1, memory_order_relaxed) == SIZE_MAX) k_fatal(\"Enum reference count overflow\"); return value; }}").unwrap();
        // A sole owner skips the atomic write: no other thread holds a reference to add one from.
        writeln!(output, "static bool k_box_{id}_release_reference(KRecord_{id} *value) {{ KSharedBox_{id} *box = k_box_{id}_header(value); return atomic_load_explicit(&box->references, memory_order_acquire) == 1 || atomic_fetch_sub_explicit(&box->references, 1, memory_order_acq_rel) == 1; }}").unwrap();
    }
    resources::declare_data(program, output);
    collections::declare_data(program, output);
    maps::declare_data(program, output);
    for id in 0..program.layouts.len() {
        writeln!(
            output,
            "static void k_record_{id}_drop(KRecord_{id} *value);"
        )
        .unwrap();
        writeln!(output, "static void k_box_{id}_drop(KRecord_{id} **value);").unwrap();
        writeln!(
            output,
            "static void k_record_{id}_drop_task(void *payload, KDropStack *stack);"
        )
        .unwrap();
        writeln!(
            output,
            "static void k_box_{id}_drop_task(void *payload, KDropStack *stack);"
        )
        .unwrap();
        writeln!(
            output,
            "static void k_box_{id}_release(void *payload, KDropStack *stack);"
        )
        .unwrap();
        writeln!(
            output,
            "static void k_record_{id}_drop_deep(KRecord_{id} *value, unsigned depth, KDropStack *stack);"
        )
        .unwrap();
    }
    fn needs_work(
        program: &Program,
        id: usize,
        seen: &mut std::collections::BTreeSet<usize>,
    ) -> bool {
        if !seen.insert(id) {
            return false;
        }
        payloads(&program.layouts[id])
            .into_iter()
            .flatten()
            .any(|ty| match ty {
                Ty::Boxed(_) | Ty::Error | Ty::DynBox(_) | Ty::Owner => true,
                Ty::Record(id) => needs_work(program, id, seen),
                _ => false,
            })
    }
    /// Boxed children drop by direct recursion up to depth 48, then through the work stack,
    /// so deep chains never grow the C stack without bound.
    fn deep_body(program: &Program, layout: &crate::core::Layout, output: &mut String) {
        if matches!(layout.kind, LayoutKind::Step(_)) {
            writeln!(output, "    k_owner_drop_task(&value->owner, stack);").unwrap();
        }
        let cases = layout.kind.cases().is_some();
        let niche = enum_niche(layout).is_some();
        if cases {
            if niche {
                writeln!(output, "    switch (value->f0 ? 2 : 1) {{").unwrap();
            } else {
                writeln!(output, "    switch (value->tag) {{").unwrap();
            }
        }
        for (case, fields) in payloads(layout).iter().enumerate() {
            let prefix = if cases {
                writeln!(output, "    case {}:", case + 1).unwrap();
                if niche {
                    "value->".into()
                } else {
                    format!("value->data.c{case}.")
                }
            } else {
                "value->".into()
            };
            for (i, ty) in fields.iter().enumerate() {
                match ty {
                    // The last reference frees the node by direct recursion, as an uncounted box does.
                    Ty::Boxed(child) if program.copyable_layouts.contains(child) => writeln!(output, "        {{ KRecord_{child} *child = {prefix}f{i}; {prefix}f{i} = NULL; if (child && k_box_{child}_release_reference(child)) {{ KSharedBox_{child} *box = k_box_{child}_header(child); if (depth < 48) {{ k_record_{child}_drop_deep(child, depth + 1, stack); k_small_free(box, sizeof *box); }} else {{ k_drop_push(stack, box, k_box_{child}_release); k_drop_push(stack, child, k_record_{child}_drop_task); }} }} }}").unwrap(),
                    Ty::Boxed(child) => writeln!(output, "        {{ KRecord_{child} *child = {prefix}f{i}; {prefix}f{i} = NULL; if (child) {{ if (depth < 48) {{ k_record_{child}_drop_deep(child, depth + 1, stack); k_small_free(child, sizeof *child); }} else {{ k_drop_push(stack, child, k_box_{child}_release); k_drop_push(stack, child, k_record_{child}_drop_task); }} }} }}").unwrap(),
                    // Called now, not queued: the parent may be freed as soon as this returns.
                    ty if ty.owns() => writeln!(output, "        k_{}_drop_task(&{prefix}f{i}, stack);", suffix(*ty)).unwrap(),
                    _ => {}
                }
            }
            if cases {
                writeln!(output, "        break;").unwrap();
            }
        }
        if cases {
            writeln!(output, "    default: break;\n    }}").unwrap();
        }
    }
    fn fields_body(layout: &crate::core::Layout, output: &mut String, tasks: bool) {
        if matches!(layout.kind, LayoutKind::Step(_)) {
            writeln!(output, "    k_owner_drop(&value->owner);").unwrap();
        }
        let niche = enum_niche(layout).is_some();
        if layout.kind.cases().is_some() {
            if niche {
                writeln!(output, "    switch (value->f0 ? 2 : 1) {{").unwrap();
            } else {
                writeln!(output, "    switch (value->tag) {{").unwrap();
            }
        }
        for (case, fields) in payloads(layout).iter().enumerate() {
            let prefix = if layout.kind.cases().is_some() {
                writeln!(output, "    case {}:", case + 1).unwrap();
                if niche {
                    "value->".into()
                } else {
                    format!("value->data.c{case}.")
                }
            } else {
                "value->".into()
            };
            let indices = if tasks {
                (0..fields.len()).collect::<Vec<_>>()
            } else {
                (0..fields.len()).rev().collect::<Vec<_>>()
            };
            for i in indices {
                let ty = fields[i];
                if ty.owns() {
                    if tasks {
                        writeln!(
                            output,
                            "        k_drop_push(stack, &{prefix}f{i}, k_{}_drop_task);",
                            suffix(ty)
                        )
                        .unwrap();
                    } else {
                        writeln!(output, "        k_{}_drop(&{prefix}f{i});", suffix(ty)).unwrap();
                    }
                }
            }
            if layout.kind.cases().is_some() {
                writeln!(output, "        break;").unwrap();
            }
        }
        if layout.kind.cases().is_some() {
            writeln!(output, "    default: break;\n    }}").unwrap();
        }
    }
    for (id, layout) in program.layouts.iter().enumerate() {
        writeln!(output, "static void k_record_{id}_drop_task(void *payload, KDropStack *stack) {{ KRecord_{id} *value = payload;").unwrap();
        if matches!(layout.kind, LayoutKind::BigInt) {
            writeln!(output, "    (void)stack; k_bigint_drop(&value->value);").unwrap();
        } else if matches!(layout.kind, LayoutKind::Tensor) {
            writeln!(output, "    (void)stack; k_tensor_drop(&value->value);").unwrap();
        } else if matches!(layout.kind, LayoutKind::Closure { .. }) {
            writeln!(output, "    KClosure *env = value->env;\n    *value = (KRecord_{id}) {{0}};\n    if (env && atomic_fetch_sub_explicit(&env->references, 1, memory_order_acq_rel) == 1) env->release(env, stack);").unwrap();
        } else if let LayoutKind::Env(by_ref) = &layout.kind {
            for (i, field) in layout.fields.iter().enumerate() {
                if field.ty.owns() && !by_ref[i] {
                    writeln!(
                        output,
                        "    k_drop_push(stack, &value->f{i}, k_{}_drop_task);",
                        suffix(field.ty)
                    )
                    .unwrap();
                }
            }
        } else if let LayoutKind::Map(info) = layout.kind {
            maps::drop_body(id, info, output);
        } else if let LayoutKind::List(element) = layout.kind {
            collections::drop_body(id, element, output);
        } else if let LayoutKind::Array(element) = layout.kind {
            // Element drop tasks finish with the slot before the block is freed.
            writeln!(output, "    int64_t length = value->length;\n    {} *items = value->items;\n    *value = (KRecord_{id}) {{0}};", c_type(element)).unwrap();
            if element.owns() {
                // Each element drains its own stack, so freeing a large Array
                // never queues one entry per element.
                writeln!(
                    output,
                    "    for (int64_t i = 0; i < length; ++i) k_drop_run(&items[i], k_{}_drop_task);",
                    suffix(element)
                )
                .unwrap();
            } else {
                writeln!(output, "    (void)length;").unwrap();
            }
            writeln!(output, "    free(items);").unwrap();
        } else if let LayoutKind::Resource(resource) = layout.kind {
            resources::drop_body(program, id, resource, output);
        } else {
            writeln!(output, "    k_record_{id}_drop_deep(value, 0, stack);").unwrap();
        }
        writeln!(output, "}}").unwrap();
        writeln!(
            output,
            "static void k_record_{id}_drop(KRecord_{id} *value) {{"
        )
        .unwrap();
        // A moved-out or never-set value has tag 0 or no storage, so most drops return at once.
        if layout.kind.cases().is_some() {
            if enum_niche(layout).is_some() {
                writeln!(output, "    if (!value->f0) return;").unwrap();
            } else {
                writeln!(output, "    if (!value->tag) return;").unwrap();
            }
        }
        match layout.kind {
            LayoutKind::List(_) | LayoutKind::Map(_) => {
                writeln!(output, "    if (!value->handle) return;").unwrap();
            }
            LayoutKind::Array(_) => writeln!(output, "    if (!value->items) return;").unwrap(),
            LayoutKind::Closure { .. } => writeln!(output, "    if (!value->env) return;").unwrap(),
            _ => {}
        }
        if matches!(
            layout.kind,
            LayoutKind::BigInt
                | LayoutKind::Tensor
                | LayoutKind::Resource(_)
                | LayoutKind::List(_)
                | LayoutKind::Array(_)
                | LayoutKind::Map(_)
                | LayoutKind::Closure { .. }
                | LayoutKind::Env(_)
        ) || needs_work(program, id, &mut std::collections::BTreeSet::new())
        {
            writeln!(output, "    k_drop_run(value, k_record_{id}_drop_task);").unwrap();
        } else {
            fields_body(layout, output, false);
        }
        writeln!(output, "    *value = (KRecord_{id}) {{0}};\n}}").unwrap();
        writeln!(output, "static KRecord_{id} k_record_{id}_take(KRecord_{id} *value) {{ KRecord_{id} result = *value; *value = (KRecord_{id}) {{0}}; return result; }}").unwrap();
        writeln!(output, "static void k_record_{id}_set(KRecord_{id} *value, KRecord_{id} owned) {{ k_record_{id}_drop(value); *value = owned; }}").unwrap();
        if program.copyable_layouts.contains(&id) {
            writeln!(output, "static KRecord_{id} *k_box_{id}_new(KRecord_{id} owned) {{ KSharedBox_{id} *box = k_small_alloc(sizeof *box); if (!box) k_fatal(\"Out of memory\"); atomic_init(&box->references, 1); box->value = owned; return &box->value; }}").unwrap();
        } else {
            writeln!(output, "static KRecord_{id} *k_box_{id}_new(KRecord_{id} owned) {{ KRecord_{id} *value = k_small_alloc(sizeof *value); if (!value) k_fatal(\"Out of memory\"); *value = owned; return value; }}").unwrap();
        }
        writeln!(output, "static void k_record_{id}_drop_deep(KRecord_{id} *value, unsigned depth, KDropStack *stack) {{ (void)depth; (void)stack;").unwrap();
        if matches!(
            layout.kind,
            LayoutKind::BigInt
                | LayoutKind::Tensor
                | LayoutKind::Resource(_)
                | LayoutKind::List(_)
                | LayoutKind::Array(_)
                | LayoutKind::Map(_)
                | LayoutKind::Closure { .. }
                | LayoutKind::Env(_)
                | LayoutKind::Trait(_)
        ) {
            writeln!(output, "    k_record_{id}_drop_task(value, stack);").unwrap();
        } else {
            deep_body(program, layout, output);
        }
        writeln!(output, "}}").unwrap();
        if program.copyable_layouts.contains(&id) {
            writeln!(output, "static void k_box_{id}_drop_task(void *payload, KDropStack *stack) {{ KRecord_{id} **slot = payload; KRecord_{id} *value = *slot; *slot = NULL; if (value && k_box_{id}_release_reference(value)) {{ KSharedBox_{id} *box = k_box_{id}_header(value); k_drop_push(stack, box, k_box_{id}_release); k_drop_push(stack, value, k_record_{id}_drop_task); }} }}").unwrap();
        } else {
            writeln!(output, "static void k_box_{id}_drop_task(void *payload, KDropStack *stack) {{ KRecord_{id} **slot = payload; KRecord_{id} *value = *slot; *slot = NULL; if (value) {{ k_record_{id}_drop_deep(value, 0, stack); k_small_free(value, sizeof *value); }} }}").unwrap();
        }
        let box_type = if program.copyable_layouts.contains(&id) {
            format!("KSharedBox_{id}")
        } else {
            format!("KRecord_{id}")
        };
        writeln!(output, "static void k_box_{id}_release(void *payload, KDropStack *stack) {{ (void)stack; k_small_free(payload, sizeof({box_type})); }}").unwrap();
        writeln!(output, "static void k_box_{id}_drop(KRecord_{id} **value) {{ if (*value) k_drop_run(value, k_box_{id}_drop_task); }}").unwrap();
        writeln!(output, "static KRecord_{id} *k_box_{id}_take(KRecord_{id} **value) {{ KRecord_{id} *result = *value; *value = NULL; return result; }}").unwrap();
        if program.copyable_layouts.contains(&id) {
            writeln!(output, "static KRecord_{id} k_record_{id}_copy(KRecord_{id});\nstatic KRecord_{id} k_box_{id}_unwrap(KRecord_{id} **value) {{ KRecord_{id} *box = k_box_{id}_take(value); if (!box) k_fatal(\"Cannot move an empty Box\"); KRecord_{id} result = k_record_{id}_copy(*box); k_box_{id}_drop(&box); return result; }}").unwrap();
        } else {
            writeln!(output, "static KRecord_{id} k_box_{id}_unwrap(KRecord_{id} **value) {{ KRecord_{id} *box = k_box_{id}_take(value); if (!box) k_fatal(\"Cannot move an empty Box\"); KRecord_{id} result = *box; k_small_free(box, sizeof *box); return result; }}").unwrap();
        }
        writeln!(output, "static void k_box_{id}_set(KRecord_{id} **value, KRecord_{id} *owned) {{ k_box_{id}_drop(value); *value = owned; }}").unwrap();
        writeln!(output, "static KError k_error_box_{id}(KRecord_{id} owned) {{ KError error = k_error_allocate(UINT64_C({}), sizeof owned, k_record_{id}_drop_task); *((KRecord_{id} *)error.payload) = owned; return error; }}", id+1).unwrap();
        for ty in [Ty::Record(id), Ty::Boxed(id)] {
            let name = suffix(ty);
            let c = c_type(ty);
            writeln!(output, "static KOwner *k_owner_{name}_new({c} owned) {{ KOwner *owner=k_owner_allocate(sizeof owned,k_{name}_drop_task); *(({c} *)owner->bytes)=owned; return owner; }}").unwrap();
        }
        if !matches!(
            layout.kind,
            LayoutKind::Class
                | LayoutKind::BigInt
                | LayoutKind::Tensor
                | LayoutKind::ArenaRef
                | LayoutKind::Resource(_)
                | LayoutKind::List(_)
                | LayoutKind::Array(_)
                | LayoutKind::Map(_)
                | LayoutKind::Closure { .. }
                | LayoutKind::Env(_)
                | LayoutKind::Trait(_)
        ) {
            for (case, fields) in payloads(layout).iter().enumerate() {
                let name = if layout.kind.cases().is_some() {
                    case.to_string()
                } else {
                    "tuple".into()
                };
                let parameters = fields
                    .iter()
                    .enumerate()
                    .map(|(i, ty)| format!("{} a{i}", c_type(*ty)))
                    .collect::<Vec<_>>();
                writeln!(output, "static KRecord_{id} k_aggregate_{id}_{name}({}) {{\n    KRecord_{id} result = {{0}};", if parameters.is_empty() { "void".into() } else { parameters.join(", ") }).unwrap();
                let prefix = if layout.kind.cases().is_some() {
                    if enum_niche(layout).is_some() {
                        "result.".into()
                    } else {
                        writeln!(output, "    result.tag = {};", case + 1).unwrap();
                        format!("result.data.c{case}.")
                    }
                } else {
                    "result.".into()
                };
                for i in 0..fields.len() {
                    writeln!(output, "    {prefix}f{i} = a{i};").unwrap();
                }
                writeln!(output, "    return result;\n}}").unwrap();
            }
        }
        if let LayoutKind::Step(cases) = &layout.kind {
            let payload = c_type(cases[0].fields[0]);
            writeln!(output,"static KRecord_{id} k_step_{id}({payload} value,KOwner *owner) {{ KRecord_{id} result=k_aggregate_{id}_0(value);result.owner=owner;return result; }}").unwrap();
        }
    }
}

fn emit_aggregate_copies(program: &Program, output: &mut String) {
    // Lists, maps and closures write their own copy; arrays, resources, traits, tensors and
    // BigInts never copy. Every other copyable layout, boxed ones included, needs one here.
    let ids: Vec<usize> = program
        .copyable_layouts
        .iter()
        .copied()
        .filter(|id| {
            !matches!(
                program.layouts[*id].kind,
                LayoutKind::List(_)
                    | LayoutKind::Map(_)
                    | LayoutKind::Closure { .. }
                    | LayoutKind::Env(_)
                    | LayoutKind::Array(_)
                    | LayoutKind::Resource(_)
                    | LayoutKind::Trait(_)
                    | LayoutKind::Tensor
                    | LayoutKind::BigInt
                    | LayoutKind::Step(_)
            )
        })
        .collect();
    for id in &ids {
        writeln!(
            output,
            "static KRecord_{id} k_record_{id}_copy(KRecord_{id});"
        )
        .unwrap();
    }
    for id in ids {
        let layout = &program.layouts[id];
        writeln!(
            output,
            "static KRecord_{id} k_record_{id}_copy(KRecord_{id} value) {{"
        )
        .unwrap();
        let cases: Vec<Vec<Ty>> = match layout.kind.cases() {
            Some(cases) => cases.iter().map(|case| case.fields.clone()).collect(),
            None => vec![layout.fields.iter().map(|field| field.ty).collect()],
        };
        let niche = enum_niche(layout).is_some();
        if layout.kind.cases().is_some() {
            writeln!(
                output,
                "    switch ({}) {{",
                if niche {
                    "value.f0 ? 2 : 1"
                } else {
                    "value.tag"
                }
            )
            .unwrap();
        }
        for (case, fields) in cases.iter().enumerate() {
            let prefix = if layout.kind.cases().is_some() {
                writeln!(output, "    case {}:", case + 1).unwrap();
                if niche {
                    "value.".to_owned()
                } else {
                    format!("value.data.c{case}.")
                }
            } else {
                "value.".to_owned()
            };
            for (index, ty) in fields.iter().enumerate() {
                if copies(program, *ty) {
                    writeln!(
                        output,
                        "        {prefix}f{index} = k_{}_copy({prefix}f{index});",
                        suffix(*ty)
                    )
                    .unwrap();
                }
            }
            if layout.kind.cases().is_some() {
                writeln!(output, "        break;").unwrap();
            }
        }
        if layout.kind.cases().is_some() {
            writeln!(output, "    default: break;\n    }}").unwrap();
        }
        writeln!(output, "    return value;\n}}").unwrap();
    }
}

fn emit_checked_operations(program: &Program, output: &mut String) {
    for (id, layout) in program.layouts.iter().enumerate() {
        let LayoutKind::Union(parts, cases) = &layout.kind else {
            continue;
        };
        if parts.len() != 2 {
            continue;
        }
        let Some(error) = parts
            .iter()
            .position(|part| *part == crate::types::Type::named("Error"))
        else {
            continue;
        };
        let success = 1 - error;
        match &parts[success] {
            crate::types::Type::Named(name, _) if name == "Int" => {
                writeln!(output,"static KRecord_{id} k_checked_bigint_checkedToInt_{id}(const KBigInt *value) {{int64_t result;if(!k_bigint_to_int(value,&result)) return k_aggregate_{id}_{error}(k_runtime_error(0,K_TEXT(\"BigInt is outside the Int range\")));return k_aggregate_{id}_{success}(result);}}").unwrap();
            }
            crate::types::Type::Named(name, _) if name == "double" => {
                writeln!(output,"static KRecord_{id} k_checked_bigint_checkedToDouble_{id}(const KBigInt *value) {{double result;if(!k_bigint_to_double(value,&result)) return k_aggregate_{id}_{error}(k_runtime_error(0,K_TEXT(\"BigInt is outside the finite double range\")));return k_aggregate_{id}_{success}(result);}}").unwrap();
            }
            crate::types::Type::Named(name, _) if name == "BigInt" => {
                let Ty::Record(big) = cases[success].fields[0] else {
                    unreachable!();
                };
                writeln!(output,"static KRecord_{id} k_checked_parseBigInt_{id}(KString text) {{KBigInt result;if(!k_bigint_parse_value(text,&result)) return k_aggregate_{id}_{error}(k_runtime_error(2,k_string_concat(K_TEXT(\"Cannot parse BigInt: \"),text)));return k_aggregate_{id}_{success}((KRecord_{big}){{result}});}}").unwrap();
                writeln!(output,"static KRecord_{id} k_checked_bigint_checkedDivide_{id}(const KBigInt *a,const KBigInt *b,KLocation loc) {{if(!b->sign) return k_aggregate_{id}_{error}(k_runtime_error(0,K_TEXT(\"BigInt division by zero\")));return k_aggregate_{id}_{success}((KRecord_{big}){{k_bigint_div(a,b,loc)}});}}").unwrap();
            }
            _ => {}
        }
    }
    for (id, layout) in program.layouts.iter().enumerate() {
        let LayoutKind::Union(parts, _) = &layout.kind else {
            continue;
        };
        if parts.len() != 2 {
            continue;
        }
        let (Some(success), Some(error)) = (
            parts
                .iter()
                .position(|t| *t == crate::types::Type::named("Nat")),
            parts
                .iter()
                .position(|t| *t == crate::types::Type::named("Error")),
        ) else {
            continue;
        };
        for (name, builtin, message) in [
            ("checkedAdd", "add", "Nat addition overflow"),
            ("checkedSubtract", "sub", "Nat subtraction below zero"),
        ] {
            writeln!(output,"static KRecord_{id} k_checked_{name}_{id}(uint64_t a,uint64_t b) {{uint64_t result;if(__builtin_{builtin}_overflow(a,b,&result)) return k_aggregate_{id}_{error}(k_runtime_error(0,K_TEXT({})));return k_aggregate_{id}_{success}(result);}}",quoted(message)).unwrap();
        }
    }
    for (id, layout) in program.layouts.iter().enumerate() {
        let LayoutKind::Union(parts, _) = &layout.kind else {
            continue;
        };
        let Some(success) = parts
            .iter()
            .position(|t| *t == crate::types::Type::named("double"))
        else {
            continue;
        };
        let Some(error) = parts
            .iter()
            .position(|t| *t == crate::types::Type::named("Error"))
        else {
            continue;
        };
        if parts.len() != 2 {
            continue;
        }
        writeln!(output, "static KRecord_{id} k_checked_parseDouble_{id}(KString text) {{ double result; if (!k_parse_double_value(text,&result)) return k_aggregate_{id}_{error}(k_runtime_error(2,k_string_concat((KString) {{ \"Cannot parse double: \", 21, NULL }},text))); return k_aggregate_{id}_{success}(result); }}").unwrap();
        for (name, operator) in [
            ("checkedAdd", "+"),
            ("checkedSubtract", "-"),
            ("checkedMultiply", "*"),
            ("checkedDivide", "/"),
        ] {
            writeln!(output, "static KRecord_{id} k_checked_{name}_{id}(double a, double b) {{ double result = a {operator} b; if (!isfinite(result)) return k_aggregate_{id}_{error}(k_runtime_error(0,(KString) {{ \"Non-finite double result\", 24, NULL }})); return k_aggregate_{id}_{success}(result); }}").unwrap();
        }
    }

    for (id, layout) in program.layouts.iter().enumerate() {
        let LayoutKind::Union(parts, cases) = &layout.kind else {
            continue;
        };
        let Some(success) = parts
            .iter()
            .position(|t| *t == crate::types::Type::named("Int"))
        else {
            continue;
        };
        let Some(error) = parts
            .iter()
            .position(|t| *t == crate::types::Type::named("Error"))
        else {
            continue;
        };
        if cases.len() != 2 {
            continue;
        }
        writeln!(output, "static KRecord_{id} k_checked_parseInt_{id}(KString text) {{ int64_t result; if (!k_parse_int_value(text,&result)) return k_aggregate_{id}_{error}(k_runtime_error(2,k_string_concat((KString) {{ \"Cannot parse Int: \", 18, NULL }},text))); return k_aggregate_{id}_{success}(result); }}").unwrap();
        writeln!(output, "static KRecord_{id} k_checked_checkedToInt_{id}(double value) {{ if (!isfinite(value) || value < -9223372036854775808.0 || value >= 9223372036854775808.0 || trunc(value) != value) return k_aggregate_{id}_{error}(k_runtime_error(0,(KString) {{ \"Expected an in-range whole Int\", 30, NULL }})); return k_aggregate_{id}_{success}((int64_t)value); }}").unwrap();
        for (name, expression, message) in [
            (
                "checkedAdd",
                "__builtin_add_overflow(a,b,&result)",
                "Int addition overflow",
            ),
            (
                "checkedSubtract",
                "__builtin_sub_overflow(a,b,&result)",
                "Int subtraction overflow",
            ),
            (
                "checkedMultiply",
                "__builtin_mul_overflow(a,b,&result)",
                "Int multiplication overflow",
            ),
            (
                "checkedDivide",
                "b == 0 || (a == INT64_MIN && b == -1)",
                "Int division by zero or overflow",
            ),
        ] {
            writeln!(output, "static KRecord_{id} k_checked_{name}_{id}(int64_t a, int64_t b) {{ int64_t result = 0; if ({expression}) {{ KError error = k_runtime_error(0, (KString) {{ {}, {}, NULL }}); return k_aggregate_{id}_{error}(error); }}", quoted(message), message.len()).unwrap();
            if name == "checkedDivide" {
                writeln!(output, "result = a / b;").unwrap();
            }
            writeln!(output, "return k_aggregate_{id}_{success}(result); }}").unwrap();
        }
    }
}

fn dyn_info(program: &Program, id: usize) -> &crate::core::TraitLayout {
    let LayoutKind::Trait(info) = &program.layouts[id].kind else {
        unreachable!("trait layout")
    };
    info
}

/// Open traits dispatch through a table of function pointers; sealed traits use a switch.
fn emit_trait_objects(program: &Program, output: &mut String, session: bool) {
    for (id, layout) in program.layouts.iter().enumerate() {
        let LayoutKind::Trait(info) = &layout.kind else {
            continue;
        };
        // Parameter declarations come from the specialized instance, so mut scalars stay pointers.
        let declared = |method: usize| -> Vec<String> {
            match info.impls.first() {
                Some(implementation) => program.functions[implementation.methods[method]]
                    .parameters
                    .iter()
                    .skip(1)
                    .map(|(_, ty, mode)| {
                        format!(
                            "{}{}",
                            c_type(*ty),
                            if *mode == Passing::Mut { " *" } else { "" }
                        )
                    })
                    .collect(),
                None => info.methods[method]
                    .parameters
                    .iter()
                    .map(|(ty, _)| c_type(*ty))
                    .collect(),
            }
        };
        let open = !info.sealed && !info.impls.is_empty();
        if open {
            writeln!(output, "typedef struct {{").unwrap();
            for (m, method) in info.methods.iter().enumerate() {
                let parameters = std::iter::once("void *".to_owned())
                    .chain(declared(m))
                    .collect::<Vec<_>>();
                writeln!(
                    output,
                    "    {} (*m{m})({});",
                    c_type(method.result),
                    parameters.join(", ")
                )
                .unwrap();
            }
            writeln!(output, "}} KVtable_{id};").unwrap();
        }
        for (i, implementation) in info.impls.iter().enumerate() {
            if !open {
                break;
            }
            for (m, method) in info.methods.iter().enumerate() {
                let types = declared(m);
                let parameters = std::iter::once("void *self".to_owned())
                    .chain(types.iter().enumerate().map(|(a, ty)| format!("{ty} a{a}")))
                    .collect::<Vec<_>>();
                let arguments =
                    std::iter::once(format!("(KRecord_{} *)self", implementation.concrete))
                        .chain((0..types.len()).map(|a| format!("a{a}")))
                        .collect::<Vec<_>>();
                let call = format!(
                    "k_fn_{}({})",
                    implementation.methods[m],
                    arguments.join(", ")
                );
                let body = if method.result == Ty::Unit {
                    format!("{call};")
                } else {
                    format!("return {call};")
                };
                writeln!(
                    output,
                    "static {} k_vt_{id}_{i}_{m}({}) {{ {body} }}",
                    c_type(method.result),
                    parameters.join(", ")
                )
                .unwrap();
            }
        }
        if open {
            writeln!(output, "static const KVtable_{id} k_vtables_{id}[] = {{").unwrap();
            for i in 0..info.impls.len() {
                let entries = (0..info.methods.len())
                    .map(|m| format!("k_vt_{id}_{i}_{m}"))
                    .collect::<Vec<_>>();
                writeln!(output, "    {{ {} }},", entries.join(", ")).unwrap();
            }
            writeln!(output, "}};").unwrap();
        }
        for (m, method) in info.methods.iter().enumerate() {
            let types = declared(m);
            let parameters = std::iter::once(format!("KDyn_{id} self"))
                .chain(types.iter().enumerate().map(|(a, ty)| format!("{ty} a{a}")))
                .collect::<Vec<_>>();
            let forward = (0..types.len())
                .map(|a| format!(", a{a}"))
                .collect::<String>();
            let keyword = if method.result == Ty::Unit {
                ""
            } else {
                "return "
            };
            let name = if session {
                format!("k_session_dispatch_{id}_{m}")
            } else {
                format!("k_dyn_{id}_{m}")
            };
            writeln!(
                output,
                "static {} {name}({}) {{",
                c_type(method.result),
                parameters.join(", ")
            )
            .unwrap();
            if open {
                writeln!(
                    output,
                    "    {keyword}k_vtables_{id}[self.impl].m{m}(self.data{forward});"
                )
                .unwrap();
            } else {
                writeln!(output, "    switch (self.impl) {{").unwrap();
                for (i, implementation) in info.impls.iter().enumerate() {
                    let end = if method.result == Ty::Unit {
                        " return;"
                    } else {
                        ""
                    };
                    writeln!(
                        output,
                        "    case {i}: {keyword}k_fn_{}((KRecord_{} *)self.data{forward});{end}",
                        implementation.methods[m], implementation.concrete
                    )
                    .unwrap();
                }
                writeln!(
                    output,
                    "    default: k_fatal(\"Unknown trait-object implementation\");\n    }}"
                )
                .unwrap();
            }
            writeln!(output, "}}").unwrap();
            if session {
                let types = std::iter::once(format!("KDyn_{id}"))
                    .chain(types)
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(output,"static {} k_dyn_{id}_{m}({}) {{{keyword}(({} (*)({types}))atomic_load_explicit(&k_session_version,memory_order_acquire)->dispatch[{id}][{m}])(self{forward});}}",c_type(method.result),parameters.join(", "),c_type(method.result)).unwrap();
            }
        }
        let drop_name = if session {
            format!("k_session_dyn_drop_{id}")
        } else {
            format!("k_dynbox_{id}_drop_task")
        };
        writeln!(output, "static void {drop_name}(void *payload, KDropStack *stack) {{\n    KDyn_{id} *value = payload;\n    void *data = value->data;\n    uint32_t implementation = value->impl;\n    *value = (KDyn_{id}) {{0}};\n    if (!data) return;\n    switch (implementation) {{").unwrap();
        for (i, implementation) in info.impls.iter().enumerate() {
            writeln!(
                output,
                "    case {i}: {{ KRecord_{} *owned = data; k_box_{}_drop_task(&owned, stack); break; }}",
                implementation.concrete, implementation.concrete
            )
            .unwrap();
        }
        writeln!(output, "    default: break;\n    }}\n}}").unwrap();
        if session {
            writeln!(output,"static void k_dynbox_{id}_drop_task(void *payload,KDropStack *stack) {{((void (*)(void *,KDropStack *))atomic_load_explicit(&k_session_version,memory_order_acquire)->dispatch[{id}][{}])(payload,stack);}}",info.methods.len()).unwrap();
        }
        writeln!(output, "static void k_dynbox_{id}_drop(KDyn_{id} *value) {{ k_drop_run(value, k_dynbox_{id}_drop_task); }}").unwrap();
        writeln!(output, "static KDyn_{id} k_dynbox_{id}_take(KDyn_{id} *value) {{ KDyn_{id} result = *value; *value = (KDyn_{id}) {{0}}; return result; }}").unwrap();
        writeln!(output, "static void k_dynbox_{id}_set(KDyn_{id} *value, KDyn_{id} owned) {{ k_dynbox_{id}_drop(value); *value = owned; }}").unwrap();
        writeln!(output,"static KOwner *k_owner_dynbox_{id}_new(KDyn_{id} owned) {{KOwner *owner=k_owner_allocate(sizeof owned,k_dynbox_{id}_drop_task);*((KDyn_{id} *)owner->bytes)=owned;return owner;}}").unwrap();
    }
}

/// Closure copies share a counted environment; each code wrapper restores its typed view.
fn emit_closures(program: &Program, output: &mut String) {
    for (id, layout) in program.layouts.iter().enumerate() {
        match &layout.kind {
            LayoutKind::Closure { .. } => {
                writeln!(output, "static KRecord_{id} k_record_{id}_copy(KRecord_{id} value) {{ if (value.env && atomic_fetch_add_explicit(&value.env->references, 1, memory_order_relaxed) == SIZE_MAX) k_fatal(\"Closure reference count overflow\"); return value; }}").unwrap();
            }
            LayoutKind::Env(by_ref) => {
                writeln!(output, "static void k_env_{id}_release(KClosure *env, KDropStack *stack) {{ k_drop_push(stack, env, k_drop_free); k_record_{id}_drop_task(env, stack); }}").unwrap();
                let parameters = layout
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(i, field)| {
                        format!(
                            "{}{} a{i}",
                            c_type(field.ty),
                            if by_ref[i] { " *" } else { "" }
                        )
                    })
                    .collect::<Vec<_>>();
                writeln!(output, "static KClosure *k_env_{id}_new({}) {{\n    KRecord_{id} *env = malloc(sizeof *env);\n    if (!env) k_fatal(\"Out of memory\");\n    atomic_init(&env->header.references, 1);\n    env->header.release = k_env_{id}_release;", if parameters.is_empty() { "void".into() } else { parameters.join(", ") }).unwrap();
                for i in 0..layout.fields.len() {
                    writeln!(output, "    env->f{i} = a{i};").unwrap();
                }
                writeln!(output, "    return &env->header;\n}}").unwrap();
            }
            _ => {}
        }
    }
    for (code, closure) in program.closures.iter().enumerate() {
        let LayoutKind::Closure { parameters, result } = &program.layouts[closure.layout].kind
        else {
            unreachable!()
        };
        let declared = parameters
            .iter()
            .enumerate()
            .map(|(i, (ty, mode))| format!("{} a{i}", parameter_type(*ty, *mode)))
            .collect::<Vec<_>>();
        let arguments = closure
            .env
            .map(|env| format!("(KRecord_{env} *)env"))
            .into_iter()
            .chain((0..parameters.len()).map(|i| format!("a{i}")))
            .collect::<Vec<_>>();
        let call = format!("k_fn_{}({})", closure.function, arguments.join(", "));
        let body = if *result == Ty::Unit {
            format!("(void)env; {call};")
        } else {
            format!("(void)env; return {call};")
        };
        let parameters = std::iter::once("KClosure *env".to_owned())
            .chain(declared)
            .collect::<Vec<_>>();
        writeln!(
            output,
            "static {} k_closure_code_{code}({}) {{ {body} }}",
            c_type(*result),
            parameters.join(", ")
        )
        .unwrap();
    }
}

/// Describes an Error without calling user code or invoking foreign objects.
fn emit_error_text(program: &Program, output: &mut String) {
    writeln!(
        output,
        "static KString k_error_base_text(KError error) {{\n    switch (error.type) {{"
    )
    .unwrap();
    for (id, layout) in program.layouts.iter().enumerate() {
        let Some(cases) = layout.kind.cases().filter(|_| layout.is_error) else {
            continue;
        };
        let source = layout.source.to_string();
        let name = source.rsplit('.').next().unwrap_or(&source);
        let name = name
            .strip_prefix("Session__")
            .and_then(|rest| rest.split_once('_'))
            .filter(|(serial, _)| serial.chars().all(|c| c.is_ascii_digit()))
            .map_or(name, |(_, name)| name);
        writeln!(output, "    case UINT64_C({}): {{\n        KRecord_{id} *value = error.payload;\n        switch (value->tag) {{", id + 1).unwrap();
        for (case, variant) in cases.iter().enumerate() {
            writeln!(output, "        case {}: {{", case + 1).unwrap();
            let mut pieces = vec![format!(
                "K_TEXT({})",
                quoted(&format!("{name}.{}", variant.name))
            )];
            let mut drops = vec![];
            for (i, field) in variant.fields.iter().enumerate() {
                pieces.push(format!(
                    "K_TEXT({})",
                    quoted(if i == 0 { "(" } else { ", " })
                ));
                if let Some(label) = variant.names.get(i).and_then(|name| name.as_deref()) {
                    pieces.push(format!("K_TEXT({})", quoted(&format!("{label} = "))));
                }
                let item = format!("value->data.c{case}.f{i}");
                match field {
                    Ty::String => {
                        pieces.extend(["K_TEXT(\"\\\"\")".into(), item, "K_TEXT(\"\\\"\")".into()]);
                    }
                    Ty::Int | Ty::Nat | Ty::Double | Ty::Bool => {
                        let text = format!("text{i}");
                        writeln!(
                            output,
                            "            KString {text} = k_text_{}({item});",
                            suffix(*field)
                        )
                        .unwrap();
                        pieces.push(text.clone());
                        drops.push(format!("k_string_drop(&{text});"));
                    }
                    Ty::Record(big) if matches!(program.layouts[*big].kind, LayoutKind::BigInt) => {
                        let text = format!("text{i}");
                        writeln!(
                            output,
                            "            KString {text}=k_bigint_text(&({item}).value);"
                        )
                        .unwrap();
                        pieces.push(text.clone());
                        drops.push(format!("k_string_drop(&{text});"));
                    }
                    _ => pieces.push("K_TEXT(\"...\")".into()),
                }
            }
            if !variant.fields.is_empty() {
                pieces.push("K_TEXT(\")\")".into());
            }
            writeln!(output, "            KString pieces[] = {{ {} }};\n            KString text = k_string_join({}, pieces);\n            {}\n            return text;\n        }}", pieces.join(", "), pieces.len(), drops.join(" ")).unwrap();
        }
        writeln!(output, "        }}\n        break;\n    }}").unwrap();
    }
    writeln!(output, "    }}\n    return K_TEXT(\"an Error\");\n}}").unwrap();
    writeln!(output, "static KString k_error_text(KError error) {{ KTextBuilder builder = {{0}}; while(error.owner && error.owner->cause.type) {{ k_text_put_bytes(&builder,error.owner->context,0,error.owner->context.length); k_text_put_bytes(&builder,K_TEXT(\": \"),0,2); error=error.owner->cause; }} KString base=k_error_base_text(error); k_text_put_bytes(&builder,base,0,base.length); k_string_drop(&base); return k_text_finish(&builder); }}").unwrap();
}

/// A returned Error fails only the currently running test process.
fn emit_test_support(_program: &Program, output: &mut String) {
    writeln!(output, "static int64_t k_test_index = -1;").unwrap();
    writeln!(output, "_Noreturn static void k_test_error(KError error) {{ KString text = k_error_text(error); fputs(\"test returned an Error: \", stderr); if(text.length) fwrite(text.data,1,text.length,stderr); k_string_drop(&text); fputc('\\n',stderr); exit(1); }}").unwrap();
}

fn emit_kesdas_correlation(program: &Program, output: &mut String) {
    let Some(column) = program.layouts.iter().position(|layout| {
        matches!(&layout.source, Type::Named(name, _) | Type::Constructor(name, _) if name == "std.kesdas.Column")
    }) else {
        return;
    };
    let fields = &program.layouts[column].fields;
    let position = |name: &str| {
        fields
            .iter()
            .position(|field| field.name == name)
            .unwrap_or_else(|| panic!("Column has no {name} field"))
    };
    let (kind, ints, floats, bools, missing, size, _strs) = (
        position("kind"),
        position("ints"),
        position("floats"),
        position("bools"),
        position("missing"),
        position("size"),
        position("strs"),
    );
    let code = r#"
static double kk_column_number(KRecord_$COLUMN$ *column, int64_t position) {
    int64_t kind = column->f$KIND$;
    if (kind == 0) return (double)column->f$INTS$.handle->storage[position].value;
    if (kind == 1) return column->f$FLOATS$.handle->storage[position].value;
    if (kind == 2) return column->f$BOOLS$.handle->storage[position].value ? 1.0 : 0.0;
    return 0.0;
}
static double k_kesdas_correlation(KRecord_$COLUMN$ *a, KRecord_$COLUMN$ *b) {
    size_t n = (size_t)(a->f$SIZE$ < b->f$SIZE$ ? a->f$SIZE$ : b->f$SIZE$);
    size_t count = 0;
    double sx = 0.0, sy = 0.0;
    for (size_t i = 0; i < n; ++i) {
        if (a->f$MISSING$.handle->storage[i].value || b->f$MISSING$.handle->storage[i].value) continue;
        sx += kk_column_number(a,(int64_t)i);
        sy += kk_column_number(b,(int64_t)i);
        ++count;
    }
    if (count < 2) return NAN;
    double mx = sx / (double)count, my = sy / (double)count;
    double cov = 0.0, vx = 0.0, vy = 0.0;
    for (size_t i = 0; i < n; ++i) {
        if (a->f$MISSING$.handle->storage[i].value || b->f$MISSING$.handle->storage[i].value) continue;
        double dx = kk_column_number(a,(int64_t)i) - mx;
        double dy = kk_column_number(b,(int64_t)i) - my;
        cov += dx * dy; vx += dx * dx; vy += dy * dy;
    }
    return cov / sqrt(vx * vy);
}
"#;
    let code = code
        .replace("$COLUMN$", &column.to_string())
        .replace("$KIND$", &kind.to_string())
        .replace("$INTS$", &ints.to_string())
        .replace("$FLOATS$", &floats.to_string())
        .replace("$BOOLS$", &bools.to_string())
        .replace("$MISSING$", &missing.to_string())
        .replace("$SIZE$", &size.to_string());
    output.push_str(&code);
}

fn emit_kesdas_arithmetic(program: &Program, output: &mut String) {
    let Some(column) = program.layouts.iter().position(|layout| {
        matches!(&layout.source, Type::Named(name, _) | Type::Constructor(name, _) if name == "std.kesdas.Column")
    }) else {
        return;
    };
    let fields = &program.layouts[column].fields;
    let position = |name: &str| {
        fields
            .iter()
            .position(|field| field.name == name)
            .unwrap_or_else(|| panic!("Column has no {name} field"))
    };
    let (kind, ints, floats, bools, missing, size) = (
        position("kind"),
        position("ints"),
        position("floats"),
        position("bools"),
        position("missing"),
        position("size"),
    );
    let ints_list = match fields[ints].ty {
        Ty::Record(id) => id,
        _ => unreachable!(),
    };
    let float_list = match fields[floats].ty {
        Ty::Record(id) => id,
        _ => unreachable!(),
    };
    let bools_list = match fields[bools].ty {
        Ty::Record(id) => id,
        _ => unreachable!(),
    };
    let strs = position("strs");
    let int_list = program
        .layouts
        .iter()
        .position(|layout| layout.source == Type::applied("List", vec![Type::named("Int")]))
        .expect("List[Int] layout");
    let strs_list = match fields[strs].ty {
        Ty::Record(id) => id,
        _ => unreachable!(),
    };
    let missing_list = match fields[missing].ty {
        Ty::Record(id) => id,
        _ => unreachable!(),
    };
    let code = r#"
static double kk_arithmetic_number(KRecord_$COLUMN$ *column, int64_t position) {
    int64_t kind = column->f$KIND_FIELD$;
    if (kind == 0) return (double)column->f$INTS_FIELD$.handle->storage[position].value;
    if (kind == 1) return column->f$FLOATS_FIELD$.handle->storage[position].value;
    if (kind == 2) return column->f$BOOLS_FIELD$.handle->storage[position].value ? 1.0 : 0.0;
    return 0.0;
}
static KRecord_$COLUMN$ k_kesdas_arithmetic(int64_t op, KRecord_$COLUMN$ *a, KRecord_$COLUMN$ *b) {
    size_t n = (size_t)(a->f$SIZE_FIELD$ < b->f$SIZE_FIELD$ ? a->f$SIZE_FIELD$ : b->f$SIZE_FIELD$);
    KRecord_$COLUMN$ result = {0};
    result.f$KIND_FIELD$ = 1;
    result.f$SIZE_FIELD$ = (int64_t)n;
    KRecord_$FLOATS$ values = k_list_$FLOATS$_blank(n);
    KRecord_$MISSING$ gone = k_list_$MISSING$_blank(n);
    for (size_t i = 0; i < n; ++i) {
        bool miss = a->f$MISSING_FIELD$.handle->storage[i].value || b->f$MISSING_FIELD$.handle->storage[i].value;
        gone.handle->storage[i].value = miss;
        double value = 0.0;
        if (!miss) {
            double x = kk_arithmetic_number(a,(int64_t)i);
            double y = kk_arithmetic_number(b,(int64_t)i);
            if (op == 0) value = x + y;
            else if (op == 1) value = x - y;
            else if (op == 2) value = x * y;
            else if (op == 3) value = x / y;
            else if (op == 4) value = floor(x / y);
            else if (op == 5) value = x - y * floor(x / y);
            else if (op == 6) value = pow(x,y);
        }
        values.handle->storage[i].value = value;
    }
    result.f$FLOATS_FIELD$ = values;
    result.f$MISSING_FIELD$ = gone;
    return result;
}

static KRecord_$INTLIST$ k_kesdas_code_counts(KRecord_$INTLIST$ codes, int64_t width) {
    KRecord_$INTLIST$ out = k_list_$INTLIST$_blank(width > 0 ? (size_t)width : 0);
    for (int64_t i = 0; i < width; ++i) out.handle->storage[i].value = 0;
    if (codes.handle) {
        for (size_t i = 0; i < codes.handle->length; ++i) {
            int64_t code = codes.handle->storage[i].value;
            if (code >= 0 && code < width) out.handle->storage[code].value += 1;
        }
    }
    return out;
}

static KRecord_$COLUMN$ k_kesdas_group_stat(KRecord_$COLUMN$ *column, KRecord_$INTLIST$ codes,
        KRecord_$INTLIST$ rank, int64_t groups, int64_t stat) {
    size_t n = column->f$SIZE_FIELD$ > 0 ? (size_t)column->f$SIZE_FIELD$ : 0;
    if (codes.handle && codes.handle->length < n) n = (size_t)codes.handle->length;
    int64_t kind = column->f$KIND_FIELD$;
    double *sums = calloc((size_t)(groups > 0 ? groups : 1), sizeof(double));
    int64_t *isums = calloc((size_t)(groups > 0 ? groups : 1), sizeof(int64_t));
    int64_t *counts = calloc((size_t)(groups > 0 ? groups : 1), sizeof(int64_t));
    if (!sums || !isums || !counts) k_fatal("Out of memory");
    for (size_t i = 0; i < n; ++i) {
        if (column->f$MISSING_FIELD$.handle->storage[i].value) continue;
        int64_t code = codes.handle->storage[i].value;
        if (code < 0 || code >= (int64_t)rank.handle->length) continue;
        int64_t group = rank.handle->storage[code].value;
        if (group < 0 || group >= groups) continue;
        counts[group] += 1;
        if (stat == 2) continue;
        if (kind == 1) sums[group] += column->f$FLOATS_FIELD$.handle->storage[i].value;
        else if (kind == 0) {
            if (stat == 0) isums[group] += column->f$INTS_FIELD$.handle->storage[i].value;
            else sums[group] += (double)column->f$INTS_FIELD$.handle->storage[i].value;
        }
    }
    KRecord_$COLUMN$ result = {0};
    result.f$SIZE_FIELD$ = groups;
    if (stat == 2 || (stat == 0 && kind == 0)) {
        result.f$KIND_FIELD$ = 0;
        KRecord_$INTS$ out = k_list_$INTS$_blank((size_t)(groups > 0 ? groups : 0));
        for (int64_t g = 0; g < groups; ++g) out.handle->storage[g].value = stat == 2 ? counts[g] : isums[g];
        result.f$INTS_FIELD$ = out;
    } else {
        result.f$KIND_FIELD$ = 1;
        KRecord_$FLOATS$ out = k_list_$FLOATS$_blank((size_t)(groups > 0 ? groups : 0));
        for (int64_t g = 0; g < groups; ++g)
            out.handle->storage[g].value = stat == 0 ? sums[g] : (counts[g] ? sums[g] / (double)counts[g] : (0.0 / 0.0));
        result.f$FLOATS_FIELD$ = out;
    }
    KRecord_$MISSING$ gone = k_list_$MISSING$_blank((size_t)(groups > 0 ? groups : 0));
    for (int64_t g = 0; g < groups; ++g) gone.handle->storage[g].value = false;
    result.f$MISSING_FIELD$ = gone;
    free(sums); free(isums); free(counts);
    return result;
}

static KRecord_$COLUMN$ k_kesdas_tile(KRecord_$COLUMN$ *source, int64_t times) {
    size_t n = source->f$SIZE_FIELD$ > 0 ? (size_t)source->f$SIZE_FIELD$ : 0;
    if (times < 0) times = 0;
    size_t total = n * (size_t)times;
    KRecord_$COLUMN$ result = {0};
    int64_t kind = source->f$KIND_FIELD$;
    result.f$KIND_FIELD$ = kind;
    result.f$SIZE_FIELD$ = (int64_t)total;
    KRecord_$MISSING$ gone = k_list_$MISSING$_blank(total);
    for (size_t t = 0; t < (size_t)times; ++t)
        if (n) memcpy(gone.handle->storage + t * n, source->f$MISSING_FIELD$.handle->storage, n * sizeof(KListCell_$MISSING$));
    if (kind == 0) {
        KRecord_$INTS$ out = k_list_$INTS$_blank(total);
        for (size_t t = 0; t < (size_t)times; ++t)
            if (n) memcpy(out.handle->storage + t * n, source->f$INTS_FIELD$.handle->storage, n * sizeof(KListCell_$INTS$));
        result.f$INTS_FIELD$ = out;
    } else if (kind == 1) {
        KRecord_$FLOATS$ out = k_list_$FLOATS$_blank(total);
        for (size_t t = 0; t < (size_t)times; ++t)
            if (n) memcpy(out.handle->storage + t * n, source->f$FLOATS_FIELD$.handle->storage, n * sizeof(KListCell_$FLOATS$));
        result.f$FLOATS_FIELD$ = out;
    } else if (kind == 2) {
        KRecord_$BOOLS$ out = k_list_$BOOLS$_blank(total);
        for (size_t t = 0; t < (size_t)times; ++t)
            if (n) memcpy(out.handle->storage + t * n, source->f$BOOLS_FIELD$.handle->storage, n * sizeof(KListCell_$BOOLS$));
        result.f$BOOLS_FIELD$ = out;
    } else {
        KRecord_$STRS$ out = {0};
        for (size_t t = 0; t < (size_t)times; ++t)
            for (size_t i = 0; i < n; ++i)
                k_list_$STRS$_append(&out, k_string_copy(source->f$STRS_FIELD$.handle->items[i]->value));
        result.f$STRS_FIELD$ = out;
    }
    result.f$MISSING_FIELD$ = gone;
    return result;
}

static KRecord_$COLUMN$ k_kesdas_concat(KRecord_$COLUMN$ *a, KRecord_$COLUMN$ *b) {
    size_t na = a->f$SIZE_FIELD$ > 0 ? (size_t)a->f$SIZE_FIELD$ : 0;
    size_t nb = b->f$SIZE_FIELD$ > 0 ? (size_t)b->f$SIZE_FIELD$ : 0;
    size_t total = na + nb;
    KRecord_$COLUMN$ result = {0};
    int64_t kind = a->f$KIND_FIELD$;
    result.f$KIND_FIELD$ = kind;
    result.f$SIZE_FIELD$ = (int64_t)total;
    KRecord_$MISSING$ gone = k_list_$MISSING$_blank(total);
    if (na) memcpy(gone.handle->storage, a->f$MISSING_FIELD$.handle->storage, na * sizeof(KListCell_$MISSING$));
    if (nb) memcpy(gone.handle->storage + na, b->f$MISSING_FIELD$.handle->storage, nb * sizeof(KListCell_$MISSING$));
    if (kind == 0) {
        KRecord_$INTS$ out = k_list_$INTS$_blank(total);
        if (na) memcpy(out.handle->storage, a->f$INTS_FIELD$.handle->storage, na * sizeof(KListCell_$INTS$));
        if (nb) memcpy(out.handle->storage + na, b->f$INTS_FIELD$.handle->storage, nb * sizeof(KListCell_$INTS$));
        result.f$INTS_FIELD$ = out;
    } else if (kind == 1) {
        KRecord_$FLOATS$ out = k_list_$FLOATS$_blank(total);
        if (na) memcpy(out.handle->storage, a->f$FLOATS_FIELD$.handle->storage, na * sizeof(KListCell_$FLOATS$));
        if (nb) memcpy(out.handle->storage + na, b->f$FLOATS_FIELD$.handle->storage, nb * sizeof(KListCell_$FLOATS$));
        result.f$FLOATS_FIELD$ = out;
    } else if (kind == 2) {
        KRecord_$BOOLS$ out = k_list_$BOOLS$_blank(total);
        if (na) memcpy(out.handle->storage, a->f$BOOLS_FIELD$.handle->storage, na * sizeof(KListCell_$BOOLS$));
        if (nb) memcpy(out.handle->storage + na, b->f$BOOLS_FIELD$.handle->storage, nb * sizeof(KListCell_$BOOLS$));
        result.f$BOOLS_FIELD$ = out;
    } else {
        KRecord_$STRS$ out = {0};
        for (size_t i = 0; i < na; ++i) k_list_$STRS$_append(&out, k_string_copy(a->f$STRS_FIELD$.handle->items[i]->value));
        for (size_t i = 0; i < nb; ++i) k_list_$STRS$_append(&out, k_string_copy(b->f$STRS_FIELD$.handle->items[i]->value));
        result.f$STRS_FIELD$ = out;
    }
    result.f$MISSING_FIELD$ = gone;
    return result;
}

static KRecord_$COLUMN$ k_kesdas_index(int64_t count) {
    size_t n = count > 0 ? (size_t)count : 0;
    KRecord_$COLUMN$ result = {0};
    result.f$KIND_FIELD$ = 0;
    result.f$SIZE_FIELD$ = (int64_t)n;
    KRecord_$INTS$ out = k_list_$INTS$_blank(n);
    KRecord_$MISSING$ gone = k_list_$MISSING$_blank(n);
    for (size_t i = 0; i < n; ++i) {
        out.handle->storage[i].value = (int64_t)i;
        gone.handle->storage[i].value = false;
    }
    result.f$INTS_FIELD$ = out;
    result.f$MISSING_FIELD$ = gone;
    return result;
}

static KRecord_$COLUMN$ k_kesdas_repeat_names(KRecord_$STRS$ names, int64_t count) {
    size_t width = names.handle ? (size_t)names.handle->length : 0;
    size_t each = count > 0 ? (size_t)count : 0;
    size_t total = width * each;
    KRecord_$COLUMN$ result = {0};
    result.f$KIND_FIELD$ = 3;
    result.f$SIZE_FIELD$ = (int64_t)total;
    KRecord_$STRS$ out = {0};
    for (size_t i = 0; i < width; ++i) {
        KString name = names.handle->items[i]->value;
        for (size_t j = 0; j < each; ++j) k_list_$STRS$_append(&out, k_string_copy(name));
    }
    KRecord_$MISSING$ gone = k_list_$MISSING$_blank(total);
    for (size_t i = 0; i < total; ++i) gone.handle->storage[i].value = false;
    result.f$STRS_FIELD$ = out;
    result.f$MISSING_FIELD$ = gone;
    return result;
}

static KRecord_$COLUMN$ k_kesdas_compare_float(KRecord_$COLUMN$ *column, int64_t op, double other) {
    size_t n = column->f$SIZE_FIELD$ > 0 ? (size_t)column->f$SIZE_FIELD$ : 0;
    KRecord_$COLUMN$ result = {0};
    result.f$KIND_FIELD$ = 2;
    result.f$SIZE_FIELD$ = (int64_t)n;
    KRecord_$BOOLS$ answers = k_list_$BOOLS$_blank(n);
    KRecord_$MISSING$ gone = k_list_$MISSING$_blank(n);
    int64_t kind = column->f$KIND_FIELD$;
    for (size_t i = 0; i < n; ++i) {
        bool missing = column->f$MISSING_FIELD$.handle && column->f$MISSING_FIELD$.handle->length > i
            && column->f$MISSING_FIELD$.handle->storage[i].value;
        bool answer = false;
        if (kind == 0 || kind == 1 || kind == 2) {
            double value = kind == 1 ? column->f$FLOATS_FIELD$.handle->storage[i].value
                : kind == 0 ? (double)column->f$INTS_FIELD$.handle->storage[i].value
                : column->f$BOOLS_FIELD$.handle->storage[i].value ? 1.0 : 0.0;
            int order = value < other ? -1 : value > other ? 1 : 0;
            answer = missing ? op == 1
                : op == 0 ? order == 0
                : op == 1 ? order != 0
                : op == 2 ? order < 0
                : op == 3 ? order <= 0
                : op == 4 ? order > 0
                : order >= 0;
        } else {
            answer = op == 1;
        }
        answers.handle->storage[i].value = answer;
        gone.handle->storage[i].value = false;
    }
    result.f$BOOLS_FIELD$ = answers;
    result.f$MISSING_FIELD$ = gone;
    return result;
}

static KRecord_$COLUMN$ k_kesdas_both(KRecord_$COLUMN$ *left, KRecord_$COLUMN$ *right) {
    size_t n = left->f$SIZE_FIELD$ < right->f$SIZE_FIELD$ ? (size_t)left->f$SIZE_FIELD$ : (size_t)right->f$SIZE_FIELD$;
    KRecord_$COLUMN$ result = {0};
    result.f$KIND_FIELD$ = 2;
    result.f$SIZE_FIELD$ = (int64_t)n;
    KRecord_$BOOLS$ answers = k_list_$BOOLS$_blank(n);
    KRecord_$MISSING$ gone = k_list_$MISSING$_blank(n);
    for (size_t i = 0; i < n; ++i) {
        bool l = left->f$BOOLS_FIELD$.handle->storage[i].value
            && !(left->f$MISSING_FIELD$.handle && left->f$MISSING_FIELD$.handle->storage[i].value);
        bool r = right->f$BOOLS_FIELD$.handle->storage[i].value
            && !(right->f$MISSING_FIELD$.handle && right->f$MISSING_FIELD$.handle->storage[i].value);
        answers.handle->storage[i].value = l && r;
        gone.handle->storage[i].value = false;
    }
    result.f$BOOLS_FIELD$ = answers;
    result.f$MISSING_FIELD$ = gone;
    return result;
}

static KRecord_$COLUMN$ k_kesdas_rolling_mean(KRecord_$COLUMN$ *source, int64_t window, int64_t needed) {
    size_t n = source->f$SIZE_FIELD$ > 0 ? (size_t)source->f$SIZE_FIELD$ : 0;
    KRecord_$COLUMN$ result = {0};
    result.f$KIND_FIELD$ = 1;
    result.f$SIZE_FIELD$ = (int64_t)n;
    KRecord_$FLOATS$ floats = k_list_$FLOATS$_blank(n);
    KRecord_$MISSING$ gone = k_list_$MISSING$_blank(n);
    int64_t kind = source->f$KIND_FIELD$;
    double total = 0.0;
    int64_t count = 0;
    for (size_t i = 0; i < n; ++i) {
        bool missing = source->f$MISSING_FIELD$.handle && source->f$MISSING_FIELD$.handle->length > i
            && source->f$MISSING_FIELD$.handle->storage[i].value;
        if (!missing) {
            double value = kind == 1 ? source->f$FLOATS_FIELD$.handle->storage[i].value
                : kind == 0 ? (double)source->f$INTS_FIELD$.handle->storage[i].value
                : source->f$BOOLS_FIELD$.handle->storage[i].value ? 1.0 : 0.0;
            total += value; count += 1;
        }
        if (window >= 0 && (int64_t)i >= window) {
            size_t old = i - (size_t)window;
            bool oldMissing = source->f$MISSING_FIELD$.handle->storage[old].value;
            if (!oldMissing) {
                double oldValue = kind == 1 ? source->f$FLOATS_FIELD$.handle->storage[old].value
                    : kind == 0 ? (double)source->f$INTS_FIELD$.handle->storage[old].value
                    : source->f$BOOLS_FIELD$.handle->storage[old].value ? 1.0 : 0.0;
                total -= oldValue; count -= 1;
            }
        }
        bool valid = count >= needed && count > 0;
        floats.handle->storage[i].value = valid ? total / (double)count : (0.0 / 0.0);
        gone.handle->storage[i].value = !valid;
    }
    result.f$FLOATS_FIELD$ = floats;
    result.f$MISSING_FIELD$ = gone;
    return result;
}
"#;
    let code = code
        .replace("$COLUMN$", &column.to_string())
        .replace("$KIND$", &kind.to_string())
        .replace("$INTS$", &ints_list.to_string())
        .replace("$FLOATS$", &float_list.to_string())
        .replace("$BOOLS$", &bools_list.to_string())
        .replace("$STRS$", &strs_list.to_string())
        .replace("$INTLIST$", &int_list.to_string())
        .replace("$MISSING$", &missing_list.to_string())
        .replace("$KIND_FIELD$", &kind.to_string())
        .replace("$INTS_FIELD$", &ints.to_string())
        .replace("$FLOATS_FIELD$", &floats.to_string())
        .replace("$BOOLS_FIELD$", &bools.to_string())
        .replace("$STRS_FIELD$", &strs.to_string())
        .replace("$MISSING_FIELD$", &missing.to_string())
        .replace("$SIZE_FIELD$", &size.to_string());
    output.push_str(&code);
}

fn emit_kesdas_gather(program: &Program, output: &mut String) {
    let Some(column) = program.layouts.iter().position(|layout| {
        matches!(&layout.source, Type::Named(name, _) | Type::Constructor(name, _) if name == "std.kesdas.Column")
    }) else {
        return;
    };
    let Some(int_list) = program
        .layouts
        .iter()
        .position(|layout| layout.source == Type::applied("List", vec![Type::named("Int")]))
    else {
        return;
    };
    let fields = &program.layouts[column].fields;
    let position = |name: &str| {
        fields
            .iter()
            .position(|field| field.name == name)
            .unwrap_or_else(|| panic!("Column has no {name} field"))
    };
    let (kind, ints, floats, bools, strs, missing, size) = (
        position("kind"),
        position("ints"),
        position("floats"),
        position("bools"),
        position("strs"),
        position("missing"),
        position("size"),
    );
    let record = |index: usize| match fields[index].ty {
        Ty::Record(id) => id,
        _ => panic!("Column field {index} is not a List"),
    };
    let (ints_id, floats_id, bools_id, strs_id, missing_id) = (
        record(ints),
        record(floats),
        record(bools),
        record(strs),
        record(missing),
    );
    let code = r#"
static KRecord_$COLUMN$ k_kesdas_gather(KRecord_$COLUMN$ *source, KRecord_$INTLIST$ positions) {
    KRecord_$COLUMN$ result = {0};
    result.f$KIND_FIELD$ = source->f$KIND_FIELD$;
    size_t n = positions.handle ? (size_t)positions.handle->length : 0;
    result.f$SIZE_FIELD$ = (int64_t)n;
    KRecord_$INTS$ ints = {0};
    KRecord_$FLOATS$ floats = {0};
    KRecord_$BOOLS$ bools = {0};
    KRecord_$STRS$ strs = {0};
    KRecord_$MISSING$ gone = k_list_$MISSING$_blank(n);
    int64_t kind = source->f$KIND_FIELD$;
    if (kind == 0) ints = k_list_$INTS$_blank(n);
    else if (kind == 1) floats = k_list_$FLOATS$_blank(n);
    else if (kind == 2) bools = k_list_$BOOLS$_blank(n);
    for (size_t k = 0; k < n; ++k) {
        int64_t p = positions.handle->storage[k].value;
        if (p < 0 || p >= source->f$SIZE_FIELD$) k_fatal("List index out of bounds");
        bool miss = source->f$MISSING_FIELD$.handle->storage[p].value;
        gone.handle->storage[k].value = miss;
        if (kind == 0) {
            ints.handle->storage[k].value = miss ? 0 : source->f$INTS_FIELD$.handle->storage[p].value;
        } else if (kind == 1) {
            floats.handle->storage[k].value = miss ? 0.0 : source->f$FLOATS_FIELD$.handle->storage[p].value;
        } else if (kind == 2) {
            bools.handle->storage[k].value = miss ? false : source->f$BOOLS_FIELD$.handle->storage[p].value;
        } else {
            KString value = miss ? k_empty : k_string_copy(source->f$STRS_FIELD$.handle->items[p]->value);
            k_list_$STRS$_append(&strs,value);
        }
    }
    result.f$INTS_FIELD$ = ints;
    result.f$FLOATS_FIELD$ = floats;
    result.f$BOOLS_FIELD$ = bools;
    result.f$STRS_FIELD$ = strs;
    result.f$MISSING_FIELD$ = gone;
    return result;
}
"#;
    let code = code
        .replace("$COLUMN$", &column.to_string())
        .replace("$INTLIST$", &int_list.to_string())
        .replace("$INTS$", &ints_id.to_string())
        .replace("$FLOATS$", &floats_id.to_string())
        .replace("$BOOLS$", &bools_id.to_string())
        .replace("$STRS$", &strs_id.to_string())
        .replace("$MISSING$", &missing_id.to_string())
        .replace("$INTS_FIELD$", &ints.to_string())
        .replace("$FLOATS_FIELD$", &floats.to_string())
        .replace("$BOOLS_FIELD$", &bools.to_string())
        .replace("$STRS_FIELD$", &strs.to_string())
        .replace("$MISSING_FIELD$", &missing.to_string())
        .replace("$KIND_FIELD$", &kind.to_string())
        .replace("$SIZE_FIELD$", &size.to_string());
    output.push_str(&code);
}

fn emit_kesdas_top(program: &Program, output: &mut String) {
    let Some(column) = program.layouts.iter().position(|layout| {
        matches!(&layout.source, Type::Named(name, _) | Type::Constructor(name, _) if name == "std.kesdas.Column")
    }) else {
        return;
    };
    let Some(int_list) = program
        .layouts
        .iter()
        .position(|layout| layout.source == Type::applied("List", vec![Type::named("Int")]))
    else {
        return;
    };
    let fields = &program.layouts[column].fields;
    let position = |name: &str| {
        fields
            .iter()
            .position(|field| field.name == name)
            .unwrap_or_else(|| panic!("Column has no {name} field"))
    };
    let (kind, ints, floats, bools, strs, missing, size) = (
        position("kind"),
        position("ints"),
        position("floats"),
        position("bools"),
        position("strs"),
        position("missing"),
        position("size"),
    );
    let code = r#"
typedef struct { KRecord_$COLUMN$ *column; bool ascending; } KKTopContext;
static int64_t kk_top_cmp(KKTopContext *c, size_t a, size_t b) {
    int64_t kind = c->column->f$KIND_FIELD$;
    int64_t order = 0;
    if (kind == 0) {
        int64_t x = c->column->f$INTS_FIELD$.handle->storage[a].value;
        int64_t y = c->column->f$INTS_FIELD$.handle->storage[b].value;
        order = (x > y) - (x < y);
    } else if (kind == 1) {
        double x = c->column->f$FLOATS_FIELD$.handle->storage[a].value;
        double y = c->column->f$FLOATS_FIELD$.handle->storage[b].value;
        order = k_compare_double(x,y);
    } else if (kind == 2) {
        bool x = c->column->f$BOOLS_FIELD$.handle->storage[a].value;
        bool y = c->column->f$BOOLS_FIELD$.handle->storage[b].value;
        order = ((int)x > (int)y) - ((int)x < (int)y);
    } else {
        order = k_compare_string(c->column->f$STRS_FIELD$.handle->items[a]->value,
                                 c->column->f$STRS_FIELD$.handle->items[b]->value);
    }
    // Equal values rank by their original row, earliest first, for largest and
    // smallest alike. pandas' top-k keeps the first occurrence of a tie.
    if (order == 0) return (a > b) - (a < b);
    return c->ascending ? order : -order;
}
static int64_t kk_top_at(void *context, size_t left, size_t right) {
    return kk_top_cmp((KKTopContext *)context,left,right);
}
static KRecord_$INTLIST$ k_kesdas_top(KRecord_$COLUMN$ *column, int64_t n, bool ascending) {
    KRecord_$INTLIST$ result = {0};
    size_t total = (size_t)column->f$SIZE_FIELD$;
    if (n <= 0 || total == 0) return result;
    size_t *present = malloc(total * sizeof *present);
    if (!present) k_fatal("Out of memory");
    size_t count = 0;
    for (size_t i = 0; i < total; ++i) {
        if (!column->f$MISSING_FIELD$.handle->storage[i].value) present[count++] = i;
    }
    size_t limit = (size_t)n < count ? (size_t)n : count;
    KKTopContext context = { column, ascending };
    // The root is the worst kept candidate. A smaller rank means better.
    for (size_t i = 0; i < limit; ++i) {
        size_t child = i;
        while (child > 0) {
            size_t parent = (child - 1) / 2;
            if (kk_top_cmp(&context,present[parent],present[child]) >= 0) break;
            size_t swap = present[parent]; present[parent] = present[child]; present[child] = swap;
            child = parent;
        }
    }
    for (size_t i = limit; i < count; ++i) {
        if (kk_top_cmp(&context,present[i],present[0]) >= 0) continue;
        present[0] = present[i];
        size_t parent = 0;
        for (;;) {
            size_t left = parent * 2 + 1, right = left + 1, worse = parent;
            if (left < limit && kk_top_cmp(&context,present[left],present[worse]) > 0) worse = left;
            if (right < limit && kk_top_cmp(&context,present[right],present[worse]) > 0) worse = right;
            if (worse == parent) break;
            size_t swap = present[parent]; present[parent] = present[worse]; present[worse] = swap;
            parent = worse;
        }
    }
    k_sort_indices(present,limit,kk_top_at,&context);
    for (size_t i = 0; i < limit; ++i) k_list_$INTLIST$_append(&result,(int64_t)present[i]);
    free(present);
    return result;
}
"#;
    let code = code
        .replace("$COLUMN$", &column.to_string())
        .replace("$INTLIST$", &int_list.to_string())
        .replace("$KIND_FIELD$", &kind.to_string())
        .replace("$INTS_FIELD$", &ints.to_string())
        .replace("$FLOATS_FIELD$", &floats.to_string())
        .replace("$BOOLS_FIELD$", &bools.to_string())
        .replace("$STRS_FIELD$", &strs.to_string())
        .replace("$MISSING_FIELD$", &missing.to_string())
        .replace("$SIZE_FIELD$", &size.to_string());
    output.push_str(&code);
}

fn emit_kesdas_merge_left(program: &Program, output: &mut String) {
    let Some(strings) = program
        .layouts
        .iter()
        .position(|layout| layout.source == Type::applied("List", vec![Type::named("String")]))
    else {
        return;
    };
    let Some(int_list) = program
        .layouts
        .iter()
        .position(|layout| layout.source == Type::applied("List", vec![Type::named("Int")]))
    else {
        return;
    };
    let Some(tuple) = program.layouts.iter().position(|layout| {
        matches!(&layout.source, Type::Tuple(parts) if parts.len() == 2
            && parts[0] == Type::applied("List", vec![Type::named("Int")])
            && parts[1] == Type::applied("List", vec![Type::named("Int")]))
    }) else {
        return;
    };
    let code = r#"
typedef struct { uint64_t hash; int64_t next; KString key; } KKMergeEntry;
static KRecord_$TUPLE$ k_kesdas_merge_left(KRecord_$STRINGS$ left, KRecord_$STRINGS$ right) {
    size_t right_count = right.handle ? (size_t)right.handle->length : 0;
    size_t capacity = 16;
    while (capacity < right_count * 2 + 1) {
        if (capacity > SIZE_MAX / 2) k_fatal("Too many keys");
        capacity *= 2;
    }
    int64_t *buckets = malloc(capacity * sizeof *buckets);
    KKMergeEntry *entries = malloc((right_count ? right_count : 1) * sizeof *entries);
    if (!buckets || !entries) k_fatal("Out of memory");
    for (size_t i = 0; i < capacity; ++i) buckets[i] = -1;
    for (size_t j = 0; j < right_count; ++j) {
        KString key = right.handle->items[j]->value;
        uint64_t hash = UINT64_C(1469598103934665603);
        for (size_t k = 0; k < key.length; ++k) { hash ^= (unsigned char)key.data[k]; hash *= UINT64_C(1099511628211); }
        entries[j].hash = hash; entries[j].key = key;
        size_t slot = (size_t)hash & (capacity - 1);
        while (buckets[slot] >= 0) slot = (slot + 1) & (capacity - 1);
        entries[j].next = -1;
        if (buckets[slot] < 0) {
            // empty bucket; insert directly
        }
        buckets[slot] = (int64_t)j;
    }
    // Rebuild chains so duplicate keys find every right row.
    for (size_t i = 0; i < capacity; ++i) buckets[i] = -1;
    for (size_t j = 0; j < right_count; ++j) {
        size_t slot = (size_t)entries[j].hash & (capacity - 1);
        while (buckets[slot] >= 0) {
            int64_t old = buckets[slot];
            if (entries[old].hash == entries[j].hash && entries[old].key.length == entries[j].key.length
                && memcmp(entries[old].key.data,entries[j].key.data,entries[j].key.length) == 0) {
                entries[j].next = old;
                buckets[slot] = (int64_t)j;
                goto inserted_$TUPLE$;
            }
            slot = (slot + 1) & (capacity - 1);
        }
        entries[j].next = -1;
        buckets[slot] = (int64_t)j;
        inserted_$TUPLE$: ;
    }
    KRecord_$INTLIST$ lr = {0}, rr = {0};
    int64_t *matches = malloc((right_count ? right_count : 1) * sizeof *matches);
    if (!matches) k_fatal("Out of memory");
    size_t left_count = left.handle ? (size_t)left.handle->length : 0;
    for (size_t i = 0; i < left_count; ++i) {
        KString key = left.handle->items[i]->value;
        uint64_t hash = UINT64_C(1469598103934665603);
        for (size_t k = 0; k < key.length; ++k) { hash ^= (unsigned char)key.data[k]; hash *= UINT64_C(1099511628211); }
        size_t slot = (size_t)hash & (capacity - 1);
        int64_t found = -1;
        for (;;) {
            int64_t entry = buckets[slot];
            if (entry < 0) break;
            if (entries[entry].hash == hash && entries[entry].key.length == key.length
                && memcmp(entries[entry].key.data,key.data,key.length) == 0) { found = entry; break; }
            slot = (slot + 1) & (capacity - 1);
        }
        if (found < 0) {
            k_list_$INTLIST$_append(&lr,(int64_t)i);
            k_list_$INTLIST$_append(&rr,(int64_t)-1);
        } else {
            // The chain holds every right row with this key; a left join keeps
            // all of them, so the buffer is sized by the right table.
            size_t count = 0;
            for (int64_t entry = found; entry >= 0; entry = entries[entry].next) {
                matches[count++] = entry;
            }
            for (size_t k = count; k > 0; --k) {
                k_list_$INTLIST$_append(&lr,(int64_t)i);
                k_list_$INTLIST$_append(&rr,matches[k-1]);
            }
        }
    }
    free(buckets); free(entries); free(matches);
    KRecord_$TUPLE$ result = {0};
    result.f0 = lr; result.f1 = rr;
    return result;
}
"#;
    let code = code
        .replace("$TUPLE$", &tuple.to_string())
        .replace("$STRINGS$", &strings.to_string())
        .replace("$INTLIST$", &int_list.to_string());
    output.push_str(&code);
}

fn emit_kesdas_factor_codes(program: &Program, output: &mut String) {
    let Some(strings) = program
        .layouts
        .iter()
        .position(|layout| layout.source == Type::applied("List", vec![Type::named("String")]))
    else {
        return;
    };
    let Some(int_list) = program
        .layouts
        .iter()
        .position(|layout| layout.source == Type::applied("List", vec![Type::named("Int")]))
    else {
        return;
    };
    let Some(tuple) = program.layouts.iter().position(|layout| {
        matches!(&layout.source, Type::Tuple(parts) if parts.len() == 2
            && parts[0] == Type::applied("List", vec![Type::named("Int")])
            && parts[1] == Type::named("Int"))
    }) else {
        return;
    };
    let code = r#"
typedef struct { uint64_t hash; int64_t code; KString key; } KKFactorEntry;
static KRecord_$TUPLE$ k_kesdas_factor_codes(KRecord_$STRINGS$ keys) {
    size_t n = keys.handle ? (size_t)keys.handle->length : 0;
    size_t capacity = 16;
    while (capacity < n * 2 + 1) {
        if (capacity > SIZE_MAX / 2) k_fatal("Too many distinct keys");
        capacity *= 2;
    }
    int64_t *buckets = malloc(capacity * sizeof *buckets);
    KKFactorEntry *entries = malloc((n ? n : 1) * sizeof *entries);
    if (!buckets || !entries) k_fatal("Out of memory");
    for (size_t i = 0; i < capacity; ++i) buckets[i] = -1;
    KRecord_$INTLIST$ codes = k_list_$INTLIST$_blank(n);
    int64_t width = 0;
    for (size_t i = 0; i < n; ++i) {
        KString key = keys.handle->items[i]->value;
        uint64_t hash = UINT64_C(1469598103934665603);
        for (size_t k = 0; k < key.length; ++k) { hash ^= (unsigned char)key.data[k]; hash *= UINT64_C(1099511628211); }
        size_t slot = (size_t)hash & (capacity - 1);
        int64_t code = -1;
        for (;;) {
            int64_t entry = buckets[slot];
            if (entry < 0) {
                code = width;
                entries[width].hash = hash;
                entries[width].code = code;
                entries[width].key = key;
                buckets[slot] = width;
                ++width;
                break;
            }
            KKFactorEntry *found = &entries[entry];
            if (found->hash == hash && found->key.length == key.length
                && memcmp(found->key.data,key.data,key.length) == 0) {
                code = found->code;
                break;
            }
            slot = (slot + 1) & (capacity - 1);
        }
        codes.handle->storage[i].value = code;
    }
    free(buckets); free(entries);
    KRecord_$TUPLE$ result = {0};
    result.f0 = codes;
    result.f1 = width;
    return result;
}
"#;
    let code = code
        .replace("$TUPLE$", &tuple.to_string())
        .replace("$STRINGS$", &strings.to_string())
        .replace("$INTLIST$", &int_list.to_string());
    output.push_str(&code);
}

fn emit_kesdas_column_codes(program: &Program, output: &mut String) {
    let Some(column) = program.layouts.iter().position(|layout| {
        matches!(&layout.source, Type::Named(name, _) | Type::Constructor(name, _) if name == "std.kesdas.Column")
    }) else {
        return;
    };
    let Some(int_list) = program
        .layouts
        .iter()
        .position(|layout| layout.source == Type::applied("List", vec![Type::named("Int")]))
    else {
        return;
    };
    let Some(tuple) = program.layouts.iter().position(|layout| {
        matches!(&layout.source, Type::Tuple(parts) if parts.len() == 2
            && parts[0] == Type::applied("List", vec![Type::named("Int")])
            && parts[1] == Type::named("Int"))
    }) else {
        return;
    };
    let fields = &program.layouts[column].fields;
    let position = |name: &str| {
        fields
            .iter()
            .position(|field| field.name == name)
            .unwrap_or_else(|| panic!("Column has no {name} field"))
    };
    let kind = position("kind");
    let ints = position("ints");
    let floats = position("floats");
    let bools = position("bools");
    let strs = position("strs");
    let size = position("size");
    let strs_list = match fields[strs].ty {
        Ty::Record(id) => id,
        _ => unreachable!(),
    };
    let code = r#"
typedef struct { uint64_t hash; int64_t code; int64_t key; } KKColumnCodeEntry;

static KRecord_$TUPLE$ k_kesdas_column_codes(KRecord_$COLUMN$ *column) {
    if (column->f$KIND_FIELD$ == 3) return k_kesdas_factor_codes(column->f$STRS_FIELD$);
    size_t n = column->f$SIZE_FIELD$ > 0 ? (size_t)column->f$SIZE_FIELD$ : 0;
    size_t capacity = 16;
    while (capacity < n * 2 + 1) {
        if (capacity > SIZE_MAX / 2) k_fatal("Too many distinct keys");
        capacity *= 2;
    }
    int64_t *buckets = malloc(capacity * sizeof *buckets);
    KKColumnCodeEntry *entries = malloc((n ? n : 1) * sizeof *entries);
    if (!buckets || !entries) k_fatal("Out of memory");
    for (size_t i = 0; i < capacity; ++i) buckets[i] = -1;
    KRecord_$INTLIST$ codes = k_list_$INTLIST$_blank(n);
    int64_t kind = column->f$KIND_FIELD$;
    int64_t width = 0;
    for (size_t i = 0; i < n; ++i) {
        int64_t key = kind == 1
            ? (int64_t)column->f$FLOATS_FIELD$.handle->storage[i].value
            : kind == 0 ? column->f$INTS_FIELD$.handle->storage[i].value
            : column->f$BOOLS_FIELD$.handle->storage[i].value ? 1 : 0;
        uint64_t hash = UINT64_C(1469598103934665603) ^ (uint64_t)key;
        hash *= UINT64_C(1099511628211);
        size_t slot = (size_t)hash & (capacity - 1);
        int64_t code = -1;
        for (;;) {
            int64_t entry = buckets[slot];
            if (entry < 0) {
                code = width;
                entries[width].hash = hash;
                entries[width].code = code;
                entries[width].key = key;
                buckets[slot] = width;
                ++width;
                break;
            }
            KKColumnCodeEntry *found = &entries[entry];
            if (found->hash == hash && found->key == key) { code = found->code; break; }
            slot = (slot + 1) & (capacity - 1);
        }
        codes.handle->storage[i].value = code;
    }
    free(buckets); free(entries);
    KRecord_$TUPLE$ result = {0};
    result.f0 = codes;
    result.f1 = width;
    return result;
}
"#;
    let code = code
        .replace("$TUPLE$", &tuple.to_string())
        .replace("$INTLIST$", &int_list.to_string())
        .replace("$COLUMN$", &column.to_string())
        .replace("$KIND_FIELD$", &kind.to_string())
        .replace("$INTS_FIELD$", &ints.to_string())
        .replace("$FLOATS_FIELD$", &floats.to_string())
        .replace("$BOOLS_FIELD$", &bools.to_string())
        .replace("$STRS_FIELD$", &strs.to_string())
        .replace("$SIZE_FIELD$", &size.to_string());
    let _ = strs_list;
    output.push_str(&code);
}

fn emit_kesdas_order(program: &Program, output: &mut String) {
    let Some(column) = program.layouts.iter().position(|layout| {
        matches!(&layout.source, Type::Named(name, _) | Type::Constructor(name, _) if name == "std.kesdas.Column")
    }) else {
        return;
    };
    let Some(int_list) = program
        .layouts
        .iter()
        .position(|layout| layout.source == Type::applied("List", vec![Type::named("Int")]))
    else {
        return;
    };
    let fields = &program.layouts[column].fields;
    let position = |name: &str| {
        fields
            .iter()
            .position(|field| field.name == name)
            .unwrap_or_else(|| panic!("Column has no {name} field"))
    };
    let (kind, ints, floats, bools, strs, missing) = (
        position("kind"),
        position("ints"),
        position("floats"),
        position("bools"),
        position("strs"),
        position("missing"),
    );
    let code = r#"
typedef struct { KRecord_$COLUMN$ *column; } KKOrderContext;
static int64_t kk_order_compare(void *context, size_t left, size_t right) {
    KKOrderContext *c = context;
    int64_t kind = c->column->f$KIND$;
    if (kind == 0) {
        int64_t a = c->column->f$INTS$.handle->storage[left].value;
        int64_t b = c->column->f$INTS$.handle->storage[right].value;
        return (a > b) - (a < b);
    }
    if (kind == 1) {
        double a = c->column->f$FLOATS$.handle->storage[left].value;
        double b = c->column->f$FLOATS$.handle->storage[right].value;
        return k_compare_double(a,b);
    }
    if (kind == 2) {
        bool a = c->column->f$BOOLS$.handle->storage[left].value;
        bool b = c->column->f$BOOLS$.handle->storage[right].value;
        return ((int)a > (int)b) - ((int)a < (int)b);
    }
    KString a = c->column->f$STRS$.handle->items[left]->value;
    KString b = c->column->f$STRS$.handle->items[right]->value;
    return k_compare_string(a,b);
}
static KRecord_$INTLIST$ k_kesdas_order(KRecord_$COLUMN$ *column, KRecord_$INTLIST$ start, bool ascending, bool naFirst) {
    size_t n = start.handle ? (size_t)start.handle->length : 0;
    size_t *present = malloc((n ? n : 1) * sizeof *present);
    size_t *absent = malloc((n ? n : 1) * sizeof *absent);
    if (!present || !absent) k_fatal("Out of memory");
    size_t present_count = 0, absent_count = 0;
    KRecord_$MISSING_ID$ gone = column->f$MISSING_FIELD$;
    for (size_t k = 0; k < n; ++k) {
        int64_t position = start.handle->storage[k].value;
        bool is_missing = gone.handle && position >= 0 && (size_t)position < gone.handle->length
            ? gone.handle->storage[position].value : true;
        if (is_missing) absent[absent_count++] = (size_t)position;
        else present[present_count++] = (size_t)position;
    }
    if (!ascending) {
        for (size_t i = 0, j = present_count ? present_count - 1 : 0; i < j; ++i, --j) {
            size_t swap = present[i]; present[i] = present[j]; present[j] = swap;
        }
    }
    KKOrderContext context = { column };
    k_sort_indices(present, present_count, kk_order_compare, &context);
    if (!ascending) {
        for (size_t i = 0, j = present_count ? present_count - 1 : 0; i < j; ++i, --j) {
            size_t swap = present[i]; present[i] = present[j]; present[j] = swap;
        }
    }
    KRecord_$INTLIST$ result = {0};
    if (naFirst) for (size_t i = 0; i < absent_count; ++i) k_list_$INTLIST$_append(&result,(int64_t)absent[i]);
    for (size_t i = 0; i < present_count; ++i) k_list_$INTLIST$_append(&result,(int64_t)present[i]);
    if (!naFirst) for (size_t i = 0; i < absent_count; ++i) k_list_$INTLIST$_append(&result,(int64_t)absent[i]);
    free(present); free(absent);
    return result;
}
"#;
    let missing_id = match &program.layouts[column].fields[missing].ty {
        Ty::Record(id) => id.to_string(),
        _ => unreachable!(),
    };
    let code = code
        .replace("$COLUMN$", &column.to_string())
        .replace("$INTLIST$", &int_list.to_string())
        .replace("$KIND$", &kind.to_string())
        .replace("$INTS$", &ints.to_string())
        .replace("$FLOATS$", &floats.to_string())
        .replace("$BOOLS$", &bools.to_string())
        .replace("$STRS$", &strs.to_string())
        .replace("$MISSING_ID$", &missing_id)
        .replace("$MISSING_FIELD$", &missing.to_string());
    output.push_str(&code);
}

fn emit_kesdas_csv(program: &Program, output: &mut String) {
    let Some(column) = program.layouts.iter().position(|layout| {
        matches!(&layout.source, Type::Named(name, _) | Type::Constructor(name, _) if name == "std.kesdas.Column")
    }) else {
        return;
    };
    let Some(tuple) = program.layouts.iter().position(|layout| {
        matches!(&layout.source, Type::Tuple(parts) if parts.len() == 3
            && parts[0] == Type::applied("List", vec![Type::named("String")])
            && parts[1] == Type::applied("List", vec![Type::named("std.kesdas.Column")])
            && parts[2] == Type::named("String"))
    }) else {
        return;
    };
    let column_layout = &program.layouts[column];
    let field = |name: &str| {
        column_layout
            .fields
            .iter()
            .position(|f| f.name == name)
            .unwrap_or_else(|| panic!("Column has no {name} field"))
    };
    let record = |name: &str| match column_layout.fields[field(name)].ty {
        Ty::Record(id) => id,
        _ => panic!("Column.{name} is not a List"),
    };
    let (kind, ints, floats, bools, strs, missing, size) = (
        field("kind"),
        record("ints"),
        record("floats"),
        record("bools"),
        record("strs"),
        record("missing"),
        field("size"),
    );
    let Ty::Record(names) = program.layouts[tuple].fields[0].ty else {
        return;
    };
    let Ty::Record(columns) = program.layouts[tuple].fields[1].ty else {
        return;
    };
    let code = r#"
typedef struct { const char *p; size_t n; } KKCsvSlice;
typedef struct { KKCsvSlice *items; size_t length, capacity; } KKCsvVec;
// The cell table stores offsets into the file text, which halves its size;
// every field points into the same buffer.
typedef struct { uint32_t start, length; } KKCsvOffset;
typedef struct { KKCsvOffset *items; size_t length, capacity; } KKCsvOffsetVec;
static void kk_csv_push_offset(KKCsvOffsetVec *v, const char *base, KKCsvSlice field) {
    if (v->length == v->capacity) {
        size_t next = v->capacity ? v->capacity * 2 : 64;
        KKCsvOffset *items = realloc(v->items, next * sizeof *items);
        if (!items) k_fatal("Out of memory");
        v->items = items; v->capacity = next;
    }
    v->items[v->length++] = (KKCsvOffset){ (uint32_t)(field.p - base), (uint32_t)field.n };
}
static KKCsvSlice kk_csv_cell(const KKCsvOffsetVec *v, const char *base, size_t index) {
    KKCsvOffset offset = v->items[index];
    return (KKCsvSlice){ base + offset.start, offset.length };
}

static bool kk_csv_missing(const char *p, size_t n) {
    return n == 0
        || (n == 3 && (memcmp(p,"NaN",3)==0 || memcmp(p,"nan",3)==0))
        || (n == 2 && memcmp(p,"NA",2)==0)
        || (n == 3 && memcmp(p,"N/A",3)==0)
        || (n == 4 && (memcmp(p,"null",4)==0 || memcmp(p,"NULL",4)==0 || memcmp(p,"None",4)==0));
}
static bool kk_csv_int(const char *p, size_t n, int64_t *out) {
    size_t i = 0; bool neg = false;
    if (i < n && (p[i] == '-' || p[i] == '+')) { neg = p[i] == '-'; ++i; }
    if (i == n) return false;
    uint64_t value = 0;
    for (; i < n; ++i) {
        if (p[i] < '0' || p[i] > '9') return false;
        unsigned digit = (unsigned)(p[i] - '0');
        if (value > (UINT64_MAX - digit) / 10) return false;
        value = value * 10 + digit;
    }
    if (neg) {
        if (value > (uint64_t)INT64_MAX + 1) return false;
        *out = value == (uint64_t)INT64_MAX + 1 ? INT64_MIN : -(int64_t)value;
    } else {
        if (value > (uint64_t)INT64_MAX) return false;
        *out = (int64_t)value;
    }
    return true;
}
static bool kk_csv_double(const char *p, size_t n, double *out) {
    // Accept the same decimal shapes the interpreted parser accepts. strtod
    // alone would also take hexadecimal floats such as 0x10, which would make
    // a column numeric here but text when the file takes the quoted fallback.
    size_t i = 0, digits = 0;
    if (i < n && (p[i] == '-' || p[i] == '+')) ++i;
    while (i < n && p[i] >= '0' && p[i] <= '9') { ++i; ++digits; }
    if (i < n && p[i] == '.') { ++i; while (i < n && p[i] >= '0' && p[i] <= '9') { ++i; ++digits; } }
    if (digits == 0) return false;
    if (i < n && (p[i] == 'e' || p[i] == 'E')) {
        ++i;
        if (i < n && (p[i] == '+' || p[i] == '-')) ++i;
        size_t exponent = 0;
        while (i < n && p[i] >= '0' && p[i] <= '9') { ++i; ++exponent; }
        if (exponent == 0) return false;
    }
    if (i != n || n >= 63) return false;
    char buffer[64]; memcpy(buffer,p,n); buffer[n] = 0;
    char *end = NULL; errno = 0;
    double value = strtod(buffer,&end);
    if (end != buffer + n || !isfinite(value)) return false;
    *out = value; return true;
}
static bool kk_csv_bool(const char *p, size_t n, bool *out) {
    if (n == 4 && (memcmp(p,"True",4)==0 || memcmp(p,"true",4)==0)) { *out = true; return true; }
    if (n == 5 && (memcmp(p,"False",5)==0 || memcmp(p,"false",5)==0)) { *out = false; return true; }
    return false;
}
static void kk_csv_push(KKCsvVec *v, KKCsvSlice item) {
    if (v->length == v->capacity) {
        size_t next = v->capacity ? v->capacity * 2 : 64;
        KKCsvSlice *items = realloc(v->items,next*sizeof *items);
        if (!items) k_fatal("Out of memory");
        v->items = items; v->capacity = next;
    }
    v->items[v->length++] = item;
}
static void kk_csv_fields(KKCsvVec *v, const char *line, size_t n, char separator) {
    v->length = 0;
    size_t begin = 0;
    for (size_t i = 0; i <= n; ++i) {
        if (i == n || line[i] == separator) {
            KKCsvSlice field = { line + begin, i - begin };
            kk_csv_push(v,field);
            begin = i + 1;
        }
    }
}
static char *kk_csv_read(const char *path, size_t *length) {
    FILE *file = fopen(path,"rb");
    if (!file) return NULL;
    size_t capacity = 65536, used = 0;
    char *buffer = malloc(capacity);
    if (!buffer) { fclose(file); return NULL; }
    for (;;) {
        if (used == capacity) {
            if (capacity > SIZE_MAX/2) { free(buffer); fclose(file); return NULL; }
            capacity *= 2; char *grown = realloc(buffer,capacity);
            if (!grown) { free(buffer); fclose(file); return NULL; }
            buffer = grown;
        }
        size_t got = fread(buffer+used,1,capacity-used,file);
        used += got;
        if (!got) { if (ferror(file)) { free(buffer); fclose(file); return NULL; } break; }
    }
    fclose(file); *length = used; return buffer;
}

static KRecord_$TUPLE$ k_kesdas_read_csv(KString path, KString separator, bool header, int64_t skip, int64_t limit) {
    KRecord_$TUPLE$ result = {0};
    result.f0 = (KRecord_$NAMES$){0};
    result.f1 = (KRecord_$COLUMNS$){0};
    if (path.length == 0 || separator.length != 1) { result.f2 = K_TEXT("__KESDAS_CSV_FALLBACK__"); return result; }
    char *pathz = malloc(path.length + 1);
    if (!pathz) k_fatal("Out of memory");
    memcpy(pathz,path.data,path.length); pathz[path.length] = 0;
    size_t length = 0; char *raw = kk_csv_read(pathz,&length); free(pathz);
    if (!raw) { result.f2 = K_TEXT("Cannot read the CSV file"); return result; }
    // Text cells share one owner for the file text instead of each allocating
    // its own copy; the slices keep the buffer alive.
    KString text_string = k_string_new(length);
    if (length) memcpy((char *)text_string.data, raw, length);
    free(raw);
    const char *text = text_string.data;
    if (length > UINT32_MAX || memchr(text,'"',length) || memchr(text,0,length) || !k_valid_utf8((KString){text,length,NULL})) {
        k_string_drop(&text_string); result.f2 = K_TEXT("__KESDAS_CSV_FALLBACK__"); return result;
    }
    KKCsvVec names = {0}, fields = {0}, header_fields = {0};
    KKCsvOffsetVec cells = {0};
    // Columns with no missing values share one all-false mask instead of
    // building one mask list per column.
    KRecord_$MISSING_ID$ shared_missing = {0};
    bool shared_missing_ready = false;
    size_t width = 0, rows = 0, record = 0;
    bool have_header = false;
    size_t position = 0;
    while (position <= length) {
        size_t line = position;
        while (position < length && text[position] != '\n') ++position;
        size_t end = position;
        if (position < length) ++position;
        if (end > line && text[end-1] == '\r') --end;
        size_t n = end - line;
        bool blank = true;
        for (size_t i = 0; i < n; ++i) { if (!(text[line+i]==' ' || text[line+i]=='\t' || text[line+i]=='\r' || text[line+i]=='\n')) { blank = false; break; } }
        if (blank) { if (position >= length) break; continue; }
        if (record < (size_t)(skip < 0 ? 0 : skip)) { ++record; if (position >= length) break; continue; }
        if (!have_header) {
            kk_csv_fields(&header_fields,text+line,n,(char)separator.data[0]);
            width = header_fields.length;
            if (header) {
                for (size_t c = 0; c < width; ++c) {
                    KKCsvSlice field = header_fields.items[c];
                    const char *p = field.p; size_t z = field.n;
                    while (z && (*p==' ' || *p=='\t')) { ++p; --z; }
                    while (z && (p[z-1]==' ' || p[z-1]=='\t')) --z;
                    kk_csv_push(&names,(KKCsvSlice){p,z});
                }
            }
            have_header = true;
            if (header) { ++record; if (position >= length) break; continue; }
        }
        if (limit >= 0 && rows >= (size_t)limit) break;
        kk_csv_fields(&fields,text+line,n,(char)separator.data[0]);
        for (size_t c = 0; c < width; ++c) {
            KKCsvSlice field = c < fields.length ? fields.items[c] : (KKCsvSlice){text+end,0};
            kk_csv_push_offset(&cells,text,field);
        }
        ++rows; ++record;
        if (position >= length) break;
    }
    if (width == 0) {
        k_record_$MISSING_ID$_drop(&shared_missing);
    k_string_drop(&text_string);
    free(names.items); free(fields.items); free(cells.items); free(header_fields.items);
        result.f2 = K_TEXT(""); return result;
    }
    if (!header) {
        for (size_t c = 0; c < width; ++c) {
            char number[24]; int z = snprintf(number,sizeof number,"%zu",c);
            k_list_$NAMES$_append(&result.f0,k_string_bytes(number,(size_t)z));
        }
    } else {
        for (size_t c = 0; c < names.length; ++c)
            k_list_$NAMES$_append(&result.f0,k_string_bytes(names.items[c].p,names.items[c].n));
    }
    for (size_t c = 0; c < width; ++c) {
        bool any = false, anyMissing = false, ints = true, floats = true, bools = true;
        for (size_t r = 0; r < rows; ++r) {
            KKCsvSlice s = kk_csv_cell(&cells,text,r*width+c);
            if (kk_csv_missing(s.p,s.n)) { anyMissing = true; continue; }
            any = true;
            int64_t integer; double real; bool truth;
            if (ints && !kk_csv_int(s.p,s.n,&integer)) ints = false;
            if (floats && !kk_csv_double(s.p,s.n,&real)) floats = false;
            if (bools && !kk_csv_bool(s.p,s.n,&truth)) bools = false;
            if (!ints && !floats && !bools) break;
        }
        int kind = !any ? 3 : ints ? 0 : floats ? 1 : bools ? 2 : 3;
        KRecord_$COLUMN$ col = {0};
        KRecord_$INTS_ID$ integers = {0};
        KRecord_$FLOATS_ID$ reals = {0};
        KRecord_$BOOLS_ID$ truth = {0};
        KRecord_$STRS_ID$ strings = {0};
        KRecord_$MISSING_ID$ gone = {0};
        for (size_t r = 0; r < rows; ++r) {
            KKCsvSlice s = kk_csv_cell(&cells,text,r*width+c);
            bool miss = kk_csv_missing(s.p,s.n);
            if (anyMissing) k_list_$MISSING_ID$_append(&gone,miss);
            if (kind == 0) {
                int64_t value = 0; if (!miss) (void)kk_csv_int(s.p,s.n,&value);
                k_list_$INTS_ID$_append(&integers,value);
            } else if (kind == 1) {
                double value = 0.0; if (!miss) (void)kk_csv_double(s.p,s.n,&value);
                k_list_$FLOATS_ID$_append(&reals,value);
            } else if (kind == 2) {
                bool value = false; if (!miss) (void)kk_csv_bool(s.p,s.n,&value);
                k_list_$BOOLS_ID$_append(&truth,value);
            } else {
                k_list_$STRS_ID$_append(&strings,miss ? k_empty : k_string_slice_shared(text_string,(size_t)(s.p-text),(size_t)(s.p-text)+s.n));
            }
        }
        if (!anyMissing) {
            if (!shared_missing_ready) {
                for (size_t r = 0; r < rows; ++r) k_list_$MISSING_ID$_append(&shared_missing,false);
                shared_missing_ready = true;
            }
            gone = k_record_$MISSING_ID$_copy(shared_missing);
        }
        col.f$KIND_FIELD$ = kind;
        col.f$INTS_FIELD$ = integers;
        col.f$FLOATS_FIELD$ = reals;
        col.f$BOOLS_FIELD$ = truth;
        col.f$STRS_FIELD$ = strings;
        col.f$MISSING_FIELD$ = gone;
        col.f$SIZE_FIELD$ = (int64_t)rows;
        k_list_$COLUMNS$_append(&result.f1,col);
    }
    k_record_$MISSING_ID$_drop(&shared_missing);
    k_string_drop(&text_string);
    free(names.items); free(fields.items); free(cells.items); free(header_fields.items);
    result.f2 = K_TEXT("");
    return result;
}
"#;
    let code = code
        .replace("$TUPLE$", &tuple.to_string())
        .replace("$NAMES$", &names.to_string())
        .replace("$COLUMNS$", &columns.to_string())
        .replace("$COLUMN$", &column.to_string())
        .replace("$INTS_ID$", &ints.to_string())
        .replace("$FLOATS_ID$", &floats.to_string())
        .replace("$BOOLS_ID$", &bools.to_string())
        .replace("$STRS_ID$", &strs.to_string())
        .replace("$MISSING_ID$", &missing.to_string())
        .replace("$INTS_FIELD$", &field("ints").to_string())
        .replace("$FLOATS_FIELD$", &field("floats").to_string())
        .replace("$BOOLS_FIELD$", &field("bools").to_string())
        .replace("$STRS_FIELD$", &field("strs").to_string())
        .replace("$MISSING_FIELD$", &field("missing").to_string())
        .replace("$KIND_FIELD$", &kind.to_string())
        .replace("$SIZE_FIELD$", &size.to_string());
    output.push_str(&code);
}

fn emit_arrays(program: &Program, output: &mut String) {
    for (id, layout) in program.layouts.iter().enumerate() {
        let LayoutKind::Array(element) = layout.kind else {
            continue;
        };
        let c = c_type(element);
        writeln!(output,"static KRecord_{id} k_array_{id}_allocate(int64_t size,KLocation loc) {{if(size<0) k_trap(loc,\"Array size is negative\");if((uint64_t)size>SIZE_MAX/sizeof({c})) k_trap(loc,\"Array is too large\");KRecord_{id} array={{size,NULL,size}};if(size) {{array.items=calloc((size_t)size,sizeof({c}));if(!array.items) k_fatal(\"Out of memory\");}}return array;}}").unwrap();
        if element == Ty::String {
            writeln!(output, "static KRecord_{id} k_array_{id}_split(KString text, KString separator, KLocation loc) {{\n    if (!separator.length) {{\n        size_t count = 0; for (size_t i = 0; i < text.length;) {{ size_t next = i; k_utf8_next(text, &next); ++count; i = next; }}\n        KRecord_{id} result = k_array_{id}_allocate((int64_t)count, loc);\n        size_t out = 0, begin = 0; while (begin < text.length) {{ size_t next = begin; k_utf8_next(text, &next); result.items[out++] = k_string_slice_shared(text, begin, next); begin = next; }}\n        return result;\n    }}\n    if (separator.length > text.length) {{ KRecord_{id} result = k_array_{id}_allocate(1, loc); result.items[0] = k_string_copy(text); return result; }}\n    if (separator.length > SIZE_MAX/sizeof(size_t)) k_fatal(\"String is too large\");\n    size_t *prefix = malloc(separator.length*sizeof *prefix); if (!prefix) k_fatal(\"Out of memory\");\n    prefix[0] = 0; for (size_t i = 1, n = 0; i < separator.length; ++i) {{ while (n && separator.data[i] != separator.data[n]) n = prefix[n-1]; if (separator.data[i] == separator.data[n]) ++n; prefix[i] = n; }}\n    size_t count = 1; for (size_t i = 0, n = 0; i < text.length; ++i) {{ while (n && text.data[i] != separator.data[n]) n = prefix[n-1]; if (text.data[i] == separator.data[n]) ++n; if (n == separator.length) {{ ++count; n = 0; }} }}\n    KRecord_{id} result = k_array_{id}_allocate((int64_t)count, loc);\n    size_t begin = 0, out = 0; for (size_t i = 0, n = 0; i < text.length; ++i) {{ while (n && text.data[i] != separator.data[n]) n = prefix[n-1]; if (text.data[i] == separator.data[n]) ++n; if (n == separator.length) {{ result.items[out++] = k_string_slice_shared(text, begin, i+1-separator.length); begin = i+1; n = 0; }} }}\n    result.items[out++] = k_string_slice_shared(text, begin, text.length); free(prefix); return result;\n}}").unwrap();
        }
        writeln!(output, "static void k_array_{id}_reserve(KRecord_{id} *array) {{\n    if (array->length == array->capacity) {{\n        if ((uint64_t)array->capacity >= SIZE_MAX / sizeof({c}) / 2) k_fatal(\"Array is too large\");\n        int64_t capacity = array->capacity ? array->capacity * 2 : 4;\n        {c} *items = realloc(array->items, (size_t)capacity * sizeof({c}));\n        if (!items) k_fatal(\"Out of memory\");\n        \n        array->items = items;\n        array->capacity = capacity;\n    }}\n}}").unwrap();
        writeln!(output, "static void k_array_{id}_append(KRecord_{id} *array) {{\n    k_array_{id}_reserve(array);\n    array->items[array->length] = ({c}){{0}};\n    array->length += 1;\n}}").unwrap();
        // prepend moves every item one slot right. The vacated slot 0 is zeroed, so the store that follows drops nothing.
        writeln!(output, "static void k_array_{id}_prepend(KRecord_{id} *array) {{\n    k_array_{id}_reserve(array);\n    memmove(array->items + 1, array->items, (size_t)array->length * sizeof({c}));\n    memset(array->items, 0, sizeof({c}));\n    array->length += 1;\n}}").unwrap();
        // take moves the last item out and zeroes its slot; swap exchanges two slots bitwise. Neither
        // drops anything, so the owned values keep exactly one owner.
        writeln!(output, "static {c} k_array_{id}_take(KRecord_{id} *array) {{\n    {c} value = array->items[array->length - 1];\n    memset(&array->items[array->length - 1], 0, sizeof({c}));\n    array->length -= 1;\n    return value;\n}}").unwrap();
        writeln!(output, "static void k_array_{id}_swap(KRecord_{id} *array, int64_t i, int64_t j, KLocation loc) {{\n    (void)k_array_index(i, array->length, loc);\n    (void)k_array_index(j, array->length, loc);\n    {c} value = array->items[i];\n    array->items[i] = array->items[j];\n    array->items[j] = value;\n}}").unwrap();
        collections::emit_array_sort(program, id, element, &c, output);
        if element.owns() && !copies(program, element) {
            continue;
        }
        let copy = if copies(program, element) {
            format!("k_{}_copy(value)", suffix(element))
        } else {
            "value".into()
        };
        writeln!(output, "static KRecord_{id} k_array_{id}_new(int64_t size, {c} value, KLocation loc) {{\n    if (size < 0) k_trap(loc, \"Array size is negative\");\n    if ((uint64_t)size > SIZE_MAX / sizeof({c})) k_trap(loc, \"Array is too large\");\n    KRecord_{id} array = {{ size, NULL, size }};\n    if (!size) return array;\n    array.items = malloc((size_t)size * sizeof({c}));\n    if (!array.items) k_fatal(\"Out of memory\");\n    for (int64_t i = 0; i < size; ++i) array.items[i] = {copy};\n    return array;\n}}").unwrap();
    }
}

/// A C function that takes `const char *` receives NUL-terminated copies, freed after the call.
fn emit_c_text_calls(program: &Program, output: &mut String) {
    for (function, (parameters, result)) in &program.c_text_calls {
        let actual = function
            .rsplit_once("__kestrel_args_")
            .map_or(function.as_str(), |(name, _)| name);
        let declared = parameters
            .iter()
            .enumerate()
            .map(|(i, ty)| format!("{} a{i}", c_type(*ty)))
            .collect::<Vec<_>>();
        let texts = (0..parameters.len()).filter(|i| parameters[*i] == Ty::String);
        let arguments = parameters
            .iter()
            .enumerate()
            .map(|(i, ty)| {
                if *ty == Ty::String {
                    format!("t{i}")
                } else {
                    format!("a{i}")
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            output,
            "static {} k_c_{function}({}) {{",
            c_type(*result),
            declared.join(", ")
        )
        .unwrap();
        for i in texts.clone() {
            writeln!(output, "    char *t{i} = k_c_text(a{i});").unwrap();
        }
        if *result == Ty::Unit {
            writeln!(output, "    {actual}({arguments});").unwrap();
        } else {
            writeln!(output, "    {} r = {actual}({arguments});", c_type(*result)).unwrap();
        }
        for i in texts {
            writeln!(output, "    free(t{i});").unwrap();
        }
        if *result != Ty::Unit {
            writeln!(output, "    return r;").unwrap();
        }
        writeln!(output, "}}").unwrap();
    }
}

fn emit_c_boundary(program: &Program, output: &mut String) {
    let limits = [
        ("INT8_MIN", "INT8_MAX"),
        ("INT16_MIN", "INT16_MAX"),
        ("INT32_MIN", "INT32_MAX"),
        ("INT64_MIN", "INT64_MAX"),
        ("0", "UINT8_MAX"),
        ("0", "UINT16_MAX"),
        ("0", "UINT32_MAX"),
        ("0", "UINT64_MAX"),
        ("0", "0"), // CFloat32 has its own check.
        ("CHAR_MIN", "CHAR_MAX"),
    ];
    for (id, layout) in program.layouts.iter().enumerate() {
        let LayoutKind::Union(parts, _) = &layout.kind else {
            continue;
        };
        if parts.len() != 2 {
            continue;
        }
        let Some(error) = parts
            .iter()
            .position(|t| *t == crate::types::Type::named("Error"))
        else {
            continue;
        };
        let success = 1 - error;
        let crate::types::Type::Named(name, _) = &parts[success] else {
            continue;
        };
        if name == "Int" {
            writeln!(output, "static KRecord_{id} k_checked_cuint64ToInt_{id}(uint64_t value) {{ if (value > (uint64_t)INT64_MAX) return k_aggregate_{id}_{error}(k_runtime_error(0, K_TEXT(\"CUInt64 value is larger than Int\"))); return k_aggregate_{id}_{success}((int64_t)value); }}").unwrap();
            continue;
        }
        let Some(index) = crate::interop::C_TYPES.iter().position(|t| t == name) else {
            continue;
        };
        let c = c_type(Ty::C(index));
        if index == 8 {
            writeln!(output, "static KRecord_{id} k_cmake_{id}_{index}(double value) {{ if (isfinite(value) && fabs(value) > FLT_MAX) return k_aggregate_{id}_{error}(k_runtime_error(0, K_TEXT(\"Value does not fit CFloat32\"))); return k_aggregate_{id}_{success}((float)value); }}").unwrap();
        } else {
            let (low, high) = limits[index];
            // Unsigned limits compare after a sign check, so no conversion changes the value.
            let check = if (4..8).contains(&index) {
                format!("value < 0 || (uint64_t)value > (uint64_t){high}")
            } else {
                format!("value < {low} || value > {high}")
            };
            writeln!(output, "static KRecord_{id} k_cmake_{id}_{index}(int64_t value) {{ if ({check}) return k_aggregate_{id}_{error}(k_runtime_error(0, K_TEXT(\"Value does not fit {name}\"))); return k_aggregate_{id}_{success}(({c})value); }}").unwrap();
        }
    }
}

include!("native_tensor.rs");

include!("native_numerical.rs");
include!("native_variable.rs");
