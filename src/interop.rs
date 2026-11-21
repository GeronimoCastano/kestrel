//! C headers are read with Clang; their signatures map onto the std.c boundary types.
use crate::paths::CanonicalPath;
use crate::types::Type;
use std::{
    collections::BTreeMap,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

/// The std.c boundary types, in the order C programs usually list them.
pub const C_TYPES: [&str; 10] = [
    "CInt8", "CInt16", "CInt32", "CInt64", "CUInt8", "CUInt16", "CUInt32", "CUInt64", "CFloat32",
    "CChar",
];

#[derive(Clone, Debug)]
pub struct CFunction {
    pub name: String,
    pub parameters: Vec<Type>,
    pub result: Type,
}

/// The functions a header declares, and the names whose signatures the boundary cannot carry.
pub type Header = (Vec<CFunction>, BTreeMap<String, String>);

pub(crate) fn header_path(header: &str, directories: &[PathBuf]) -> Option<PathBuf> {
    directories
        .iter()
        .map(|directory| directory.join(header))
        .chain(std::iter::once(PathBuf::from(header)))
        .find(|path| path.is_file())
        .and_then(|path| path.canonical().ok())
}

pub fn read_header(header: &str, directories: &[PathBuf]) -> Result<Header, String> {
    if header.is_empty() || header.contains(['"', '\n', '\0']) {
        return Err(format!("`{header}` is not a header file name."));
    }
    let resolved = header_path(header, directories);
    let include = resolved
        .as_deref()
        .and_then(|path| path.to_str())
        .unwrap_or(header);
    let mut command = Command::new("clang");
    command.args([
        "-fsyntax-only",
        "-Xclang",
        "-ast-dump",
        "-fno-color-diagnostics",
        "-x",
        "c",
        "-",
    ]);
    for directory in directories {
        command.arg("-I").arg(directory);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot start Clang to read `{header}`: {e}."))?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(format!("#include \"{include}\"\n").as_bytes())
        .map_err(|e| format!("Cannot send `{header}` to Clang: {e}."))?;
    let output = child
        .wait_with_output()
        .map_err(|e| format!("Clang did not finish reading `{header}`: {e}."))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let first = detail.lines().find(|l| l.contains("error")).unwrap_or("");
        return Err(format!("Clang could not read `{header}`. {}", first.trim()));
    }
    Ok(parse_dump(&String::from_utf8_lossy(&output.stdout)))
}

/// The quoted type after a declaration's name, preferring the canonical spelling
/// that Clang prints as `'sugared':'canonical'`.
fn quoted_type(after: &str) -> Option<&str> {
    let end = after.find('\'')?;
    if let Some(canonical) = after[end + 1..].strip_prefix(":'")
        && let Some(close) = canonical.find('\'')
    {
        return Some(&canonical[..close]);
    }
    Some(&after[..end])
}

/// Reads top-level `FunctionDecl` lines of Clang's text AST dump. Typedef names in a
/// signature resolve through the dump's `TypedefDecl` lines.
fn parse_dump(dump: &str) -> Header {
    let mut functions = BTreeMap::new();
    let mut unsupported = BTreeMap::new();
    let mut typedefs = BTreeMap::new();
    for line in dump.lines() {
        let Some(rest) = line
            .strip_prefix("|-TypedefDecl ")
            .or_else(|| line.strip_prefix("`-TypedefDecl "))
        else {
            continue;
        };
        if let Some(quote) = rest.find('\'')
            && let Some(name) = rest[..quote].split_whitespace().last()
            && let Some(ty) = quoted_type(&rest[quote + 1..])
        {
            typedefs.insert(name.to_owned(), ty.to_owned());
        }
    }
    for line in dump.lines() {
        let Some(rest) = line
            .strip_prefix("|-FunctionDecl ")
            .or_else(|| line.strip_prefix("`-FunctionDecl "))
        else {
            continue;
        };
        if rest.contains(" implicit ") {
            continue;
        }
        let Some(quote) = rest.find('\'') else {
            continue;
        };
        let Some(name) = rest[..quote].split_whitespace().last() else {
            continue;
        };
        if name.starts_with('_') || functions.contains_key(name) || unsupported.contains_key(name) {
            continue;
        }
        let Some(signature) = quoted_type(&rest[quote + 1..]) else {
            continue;
        };
        match function(name, signature, &typedefs) {
            Ok(function) => {
                functions.insert(name.to_owned(), function);
            }
            Err(reason) => {
                unsupported.insert(name.to_owned(), reason);
            }
        }
    }
    (functions.into_values().collect(), unsupported)
}

/// Replaces a whole typedef name, keeping a leading `const`.
fn resolve(c: &str, typedefs: &BTreeMap<String, String>) -> String {
    if let Some(base) = c.trim().strip_suffix('*') {
        return format!("{} *", resolve(base, typedefs));
    }
    let mut c = c.trim().to_owned();
    for _ in 0..8 {
        let (constant, bare) = match c.strip_prefix("const ") {
            Some(bare) => ("const ", bare.trim()),
            None => ("", c.as_str()),
        };
        let Some(target) = typedefs.get(bare) else {
            break;
        };
        c = format!("{constant}{target}");
    }
    c
}

fn function(
    name: &str,
    signature: &str,
    typedefs: &BTreeMap<String, String>,
) -> Result<CFunction, String> {
    let Some(open) = signature.find('(') else {
        return Err(format!("its C type `{signature}` is not a plain function"));
    };
    let result = signature[..open].trim();
    let parameters = signature[open + 1..]
        .strip_suffix(')')
        .ok_or_else(|| format!("its C type `{signature}` is not a plain function"))?;
    if result.contains('(') || parameters.contains('(') {
        return Err("it takes or returns a function pointer".into());
    }
    if parameters.contains("...") {
        return Err("it takes a variable number of arguments".into());
    }
    let parameters = if parameters.trim().is_empty() || parameters.trim() == "void" {
        vec![]
    } else {
        parameters
            .split(',')
            .map(|p| {
                let p = resolve(p, typedefs);
                if text_parameter(&p) {
                    return Some(Type::union(
                        Type::named("String"),
                        Type::applied("CConstPointer", vec![Type::named("CChar")]),
                    ));
                }
                kestrel_type(&p).filter(|t| *t != Type::named("void"))
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| format!("a parameter in `{signature}` has no boundary type yet"))?
    };
    let result = kestrel_type(&resolve(result, typedefs))
        .ok_or_else(|| format!("its result type `{result}` has no boundary type yet"))?;
    Ok(CFunction {
        name: name.to_owned(),
        parameters,
        result,
    })
}

/// `const char *` parameters take a String copied with a final NUL for the call (Q51).
fn text_parameter(c: &str) -> bool {
    let c = c.trim();
    let c = c.strip_suffix("restrict").unwrap_or(c).trim();
    matches!(c, "const char *" | "char const *")
}

fn trailing_qualifier<'a>(c: &'a str, qualifier: &str) -> Option<&'a str> {
    let prefix = c.strip_suffix(qualifier)?;
    prefix
        .chars()
        .last()
        .is_some_and(|c| c.is_whitespace() || c == '*')
        .then_some(prefix.trim_end())
}

/// LP64 widths: `long` is 64 bits on the supported macOS and Linux targets.
/// Plain `char` keeps the target's signedness; opaque pointers preserve constness.
fn kestrel_type(c: &str) -> Option<Type> {
    let c = c.trim();
    let c = trailing_qualifier(c, "restrict").unwrap_or(c);
    let c = if c.contains('*') {
        trailing_qualifier(c, "const").unwrap_or(c)
    } else {
        c
    };
    if let Some(base) = c.strip_suffix('*') {
        let base = base.trim();
        let qualifier = trailing_qualifier(base, "const");
        let constant = qualifier.is_some() || (!base.ends_with('*') && base.starts_with("const "));
        let base = qualifier.unwrap_or(base);
        let target = if base.trim_start_matches("const ").trim() == "void" {
            Type::named("CVoid")
        } else {
            let bare = base.trim_start_matches("const ").trim();
            let opaque = ["struct", "union", "enum"].into_iter().find_map(|kind| {
                let tag = bare.strip_prefix(&format!("{kind} "))?;
                (!tag.is_empty() && tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
                    .then(|| Type::named(&format!("c.{kind}.{tag}")))
            });
            opaque.or_else(|| kestrel_type(base))?
        };
        return Some(Type::applied(
            if constant {
                "CConstPointer"
            } else {
                "CPointer"
            },
            vec![target],
        ));
    }
    let c = c.strip_prefix("const ").unwrap_or(c).trim();
    let name = match c {
        "double" => "double",
        "float" => "CFloat32",
        "_Bool" | "bool" => "Bool",
        "void" => "void",
        "char" => "CChar",
        "signed char" => "CInt8",
        "unsigned char" => "CUInt8",
        "short" => "CInt16",
        "unsigned short" => "CUInt16",
        "int" => "CInt32",
        "unsigned int" => "CUInt32",
        "long" | "long long" => "CInt64",
        "unsigned long" | "unsigned long long" => "CUInt64",
        _ => return None,
    };
    Some(Type::named(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_targets_keep_each_level_of_const_and_structs_stay_opaque() {
        let int = Type::named("CInt32");
        let pointer = |t| Type::applied("CPointer", vec![t]);
        let constant = |t| Type::applied("CConstPointer", vec![t]);
        assert_eq!(
            kestrel_type("const int **"),
            Some(pointer(constant(int.clone())))
        );
        assert_eq!(
            kestrel_type("int *const *"),
            Some(constant(pointer(int.clone())))
        );
        assert_eq!(kestrel_type("int * const"), Some(pointer(int)));
        assert_eq!(
            kestrel_type("struct Inconst *"),
            Some(pointer(Type::named("c.struct.Inconst")))
        );
        assert_eq!(kestrel_type("struct Handle"), None);
        assert_eq!(
            resolve(
                "Word *",
                &BTreeMap::from([("Word".into(), "unsigned int".into())])
            ),
            "unsigned int *"
        );
    }

    #[test]
    fn dump_lines_become_boundary_signatures() {
        let dump = "|-FunctionDecl 0x1 <line:1:1> col:15 implicit sqrt 'double (double)' extern\n|-FunctionDecl 0x2 prev 0x1 <col:1, col:26> col:15 sqrt 'double (double)' extern\n|-FunctionDecl 0x3 <col:1> col:5 abs 'int (int)' extern\n|-FunctionDecl 0x4 <col:1> col:5 printf 'int (const char *, ...)' extern\n|-FunctionDecl 0x6 <col:1> col:8 strlen 'unsigned long (const char *)' extern\n|-FunctionDecl 0x7 <col:1> col:7 getenv 'char *(const char *)' extern\n|-TypedefDecl 0x8 <col:1> col:24 referenced uLong 'unsigned long'\n|-FunctionDecl 0x9 <col:1> col:23 bound 'uLong (uLong)' extern\n`-FunctionDecl 0x5 <col:1> col:5 count 'size_t (void)':'unsigned long (void)' extern\n";
        let (functions, unsupported) = parse_dump(dump);
        let names = functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["abs", "bound", "count", "getenv", "sqrt", "strlen"]);
        assert_eq!(functions[1].parameters, [Type::named("CUInt64")]);
        assert_eq!(functions[2].result, Type::named("CUInt64"));
        assert_eq!(
            functions[5].parameters,
            [Type::union(
                Type::named("String"),
                Type::applied("CConstPointer", vec![Type::named("CChar")])
            )]
        );
        assert_eq!(
            functions[3].result,
            Type::applied("CPointer", vec![Type::named("CChar")])
        );
        assert_eq!(functions[0].parameters, [Type::named("CInt32")]);
        assert!(unsupported["printf"].contains("variable number"));
    }
}
