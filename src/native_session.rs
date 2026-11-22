//! Long-lived native worker. Session entries share one runtime and stable global slots.
use super::{TemporaryDirectory, c_type, emit, python_flags, suffix};
use crate::core::{LayoutKind, Program, Ty};
#[cfg(unix)]
use std::os::unix::{
    fs::PermissionsExt,
    net::{UnixListener, UnixStream},
};
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

pub(super) fn entry_points(program: &Program, out: &mut String) {
    let count = program.globals.len();
    let types = program.layouts.len();
    writeln!(out,"K_EXPORT void k_session_step(void ***globals,size_t *count,int argc,char **argv) {{\n    size_t previous=*count;KSessionVersion *version=calloc(1,sizeof *version);if(!version) k_fatal(\"Out of memory\");\n    version->previous=atomic_load_explicit(&k_session_version,memory_order_acquire);\n    version->globals=calloc({},sizeof *version->globals);if(!version->globals) k_fatal(\"Out of memory\");\n    if(previous) memcpy(version->globals,*globals,previous*sizeof *version->globals);\n    k_session_globals=version->globals;*globals=version->globals;*count={count};",count.max(1)).unwrap();
    for (index, ty) in program.globals.iter().enumerate() {
        if *ty != Ty::Unit {
            writeln!(out,"    if(!k_session_globals[{index}]) {{k_session_globals[{index}]=calloc(1,sizeof({}));if(!k_session_globals[{index}]) k_fatal(\"Out of memory\");}}",c_type(*ty)).unwrap();
        }
    }
    writeln!(out,"    version->error=k_cell_runtime_error;version->dispatch=calloc({},sizeof *version->dispatch);if(!version->dispatch) k_fatal(\"Out of memory\");",types.max(1)).unwrap();
    for (id, layout) in program.layouts.iter().enumerate() {
        let LayoutKind::Trait(info) = &layout.kind else {
            continue;
        };
        writeln!(out,"    version->dispatch[{id}]=calloc({},sizeof(void *));if(!version->dispatch[{id}]) k_fatal(\"Out of memory\");",info.methods.len()+1).unwrap();
        for method in 0..info.methods.len() {
            writeln!(
                out,
                "    version->dispatch[{id}][{method}]=(void *)k_session_dispatch_{id}_{method};"
            )
            .unwrap();
        }
        writeln!(
            out,
            "    version->dispatch[{id}][{}]=(void *)k_session_dyn_drop_{id};",
            info.methods.len()
        )
        .unwrap();
    }
    writeln!(
        out,
        "    atomic_store_explicit(&k_session_version,version,memory_order_release);"
    )
    .unwrap();
    if let Some(slot) = program.process_args {
        let Ty::Record(id) = program.globals[slot.id] else {
            unreachable!()
        };
        writeln!(out,"    if(previous<={}) for(int i=0;i<argc;++i) {{KString arg=k_string_bytes(argv[i],strlen(argv[i]));if(!k_valid_utf8(arg)) k_fatal(\"A command-line argument is not valid UTF-8\");k_list_{id}_append(&g{},arg);}}",slot.id,slot.id).unwrap();
    }
    if program.statics.iter().any(|field| field.mutable) {
        out.push_str("    k_static_init_all();\n");
    }
    out.push_str("    (void)argc;(void)argv;k_fn_0();\n    if(fflush(stdout)==EOF) k_fatal(\"Cannot write standard output\");\n}\nK_EXPORT void k_session_drop(void ***globals) {\n    k_session_globals=*globals;\n");
    for (index, ty) in program.globals.iter().enumerate().rev() {
        if ty.owns() {
            writeln!(out, "    k_{}_drop(&g{index});", suffix(*ty)).unwrap();
        }
    }
    for index in 0..program.globals.len() {
        writeln!(out, "    free((*globals)[{index}]);").unwrap();
    }
    out.push_str("    *globals=NULL;\n}\nK_EXPORT void k_session_modules_drop(void) {\n");
    if !program.python_sites.is_empty() {
        writeln!(out,"    if(!Py_IsInitialized()) return;PyGILState_STATE state=k_python_enter();for(size_t i=0;i<{};++i) {{Py_XDECREF(k_python_modules[i]);k_python_modules[i]=NULL;}}k_python_leave(state);",program.python_modules.len()).unwrap();
    }
    out.push_str("}\n");
}

/// Whether a top-level declaration without an initializer declares a function rather
/// than a variable. Attribute, `_Atomic` and `_Alignas` parentheses do not count, and
/// `(*name)` declares a function pointer variable.
fn declares_function(declaration: &str) -> bool {
    let bytes = declaration.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'(' {
            i += 1;
            continue;
        }
        let before = declaration[..i].trim_end();
        let word = before
            .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .next()
            .unwrap_or("");
        if matches!(
            word,
            "__attribute__" | "__declspec" | "_Atomic" | "_Alignas"
        ) {
            let mut depth = 0;
            while i < bytes.len() {
                match bytes[i] {
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        return !word.is_empty() && !declaration[i + 1..].trim_start().starts_with('*');
    }
    false
}

/// The text with its C comments removed, for deciding what a declaration declares.
fn without_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("/*") {
            rest = after.find("*/").map_or("", |end| &after[end + 2..]);
            out.push(' ');
        } else if let Some(after) = rest.strip_prefix("//") {
            rest = after.find('\n').map_or("", |end| &after[end..]);
        } else {
            let next = rest.chars().next().unwrap();
            out.push(next);
            rest = &rest[next.len_utf8()..];
        }
    }
    out
}

/// Split our C runtime's top-level declarations from its definitions. Bodies and
/// initializers stay in the one shared library; cells include only declarations.
/// A Windows DLL exports only what it marks, and code outside it reaches DLL data only
/// through `dllimport`, so `windows` marks functions and data on both sides. Thread-local
/// data cannot cross a DLL boundary there, so it stays private to the runtime.
fn runtime_parts(source: &str, windows: bool) -> (String, String) {
    let export = if windows {
        "__declspec(dllexport) "
    } else {
        ""
    };
    fn skip_quoted(bytes: &[u8], mut i: usize) -> usize {
        let quote = bytes[i];
        i += 1;
        while i < bytes.len() {
            if bytes[i] == b'\\' {
                i += 2;
            } else if bytes[i] == quote {
                return i + 1;
            } else {
                i += 1;
            }
        }
        i
    }
    fn skip_comment(bytes: &[u8], i: usize) -> Option<usize> {
        if bytes.get(i..i + 2) == Some(b"/*") {
            let mut j = i + 2;
            while j + 1 < bytes.len() && &bytes[j..j + 2] != b"*/" {
                j += 1;
            }
            return Some((j + 2).min(bytes.len()));
        }
        if bytes.get(i..i + 2) == Some(b"//") {
            let mut j = i + 2;
            while j < bytes.len() && bytes[j] != b'\n' {
                j += 1;
            }
            return Some(j);
        }
        None
    }
    let bytes = source.as_bytes();
    let (mut i, mut start, mut braces, mut parens) = (0, 0, 0, 0);
    let (mut header, mut body) = (String::new(), String::new());
    let mut initializer = None;
    while i < bytes.len() {
        if let Some(end) = skip_comment(bytes, i) {
            i = end;
            continue;
        }
        if matches!(bytes[i], b'"' | b'\'') {
            i = skip_quoted(bytes, i);
            continue;
        }
        if bytes[i] == b'#' && without_comments(&source[start..i]).trim().is_empty() {
            let mut end = i;
            loop {
                while end < bytes.len() && bytes[end] != b'\n' {
                    end += 1;
                }
                let continued = end > 0 && bytes[end - 1] == b'\\';
                if end < bytes.len() {
                    end += 1;
                }
                if !continued {
                    break;
                }
            }
            header.push_str(&source[start..end]);
            body.push_str(&source[start..end]);
            start = end;
            i = end;
            continue;
        }
        match bytes[i] {
            b'(' => parens += 1,
            b')' => parens -= 1,
            b'{' if braces == 0
                && parens == 0
                && source[start..i].trim_end().ends_with(')')
                && initializer.is_none() =>
            {
                let prefix = &source[start..i];
                let mut depth = 1;
                let mut end = i + 1;
                while end < bytes.len() && depth > 0 {
                    if let Some(next) = skip_comment(bytes, end) {
                        end = next;
                        continue;
                    }
                    if matches!(bytes[end], b'"' | b'\'') {
                        end = skip_quoted(bytes, end);
                        continue;
                    }
                    if bytes[end] == b'{' {
                        depth += 1;
                    }
                    if bytes[end] == b'}' {
                        depth -= 1;
                    }
                    end += 1;
                }
                header.push_str(&prefix.replace("static ", "extern ").replace("inline ", ""));
                header.push_str(";\n");
                body.push_str(export);
                body.push_str(&prefix.replace("static ", "").replace("inline ", ""));
                body.push_str(&source[i..end]);
                body.push('\n');
                start = end;
                i = end;
                continue;
            }
            b'=' if braces == 0 && parens == 0 => {
                initializer = Some(i);
            }
            b'{' => braces += 1,
            b'}' => braces -= 1,
            b';' if braces == 0 && parens == 0 => {
                let declaration = &source[start..=i];
                // A comment before the declaration may say "static"; only code counts.
                let code = without_comments(declaration);
                let is_static = code
                    .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .any(|word| word == "static");
                let local = code.contains("_Thread_local");
                let function = initializer.is_none()
                    && declares_function(&without_comments(&source[start..i]));
                if is_static && !local {
                    body.push_str(export);
                }
                body.push_str(&declaration.replace("static ", ""));
                if is_static && !(windows && local) {
                    let declaration = source[start..initializer.unwrap_or(i)].trim_end();
                    let external = if windows && !function {
                        "extern __declspec(dllimport) "
                    } else {
                        "extern "
                    };
                    header.push_str(&declaration.replace("static ", external));
                    header.push_str(";\n");
                } else if !is_static {
                    header.push_str(declaration);
                }
                start = i + 1;
                initializer = None;
            }
            _ => {}
        }
        i += 1;
    }
    header.push_str(&source[start..]);
    body.push_str(&source[start..]);
    (header, body)
}

fn compile(command: &mut Command) -> Result<(), String> {
    let output = command.output().map_err(|e| super::missing_clang(&e))?;
    if !output.status.success() {
        return Err(format!(
            "Session compilation failed:\n{}",
            super::compiler_messages(&output)
        ));
    }
    Ok(())
}
/// Clang for one session module that Kestrel generates, under the generated code's
/// warning policy.
fn clang(shared: bool) -> Command {
    let mut command = toolchain(shared);
    command.args([
        "-Wall",
        "-Wextra",
        "-Werror",
        "-Wno-unused-function",
        "-Wno-unused-variable",
        "-Wno-unused-parameter",
        "-Wno-unused-but-set-variable",
        "-Wno-unused-label",
    ]);
    if ["KESTREL_LEAK_CHECK", "KESTREL_NO_POOL"]
        .iter()
        .any(|n| std::env::var_os(n).is_some())
    {
        command.arg("-DK_NO_POOL");
    }
    command
}
/// Clang with the flags every module in one session shares, project C files included. On
/// Windows every module links the DLL C runtime, so the host, the runtime, project C code
/// and each entry share one heap and one standard output buffer.
fn toolchain(shared: bool) -> Command {
    let mut command = super::clang_command();
    command.args(["-std=c11", "-O2"]);
    command.args(super::thread_flags());
    if cfg!(target_os = "linux") {
        command.arg("-ldl");
    }
    if cfg!(windows) && !super::mingw() {
        // Clang's driver links the static C runtime by default even with the DLL runtime
        // selected; two runtimes in one module leave the DLL imports unresolved. MinGW
        // links the system's DLL C runtime already.
        command.args(["-fms-runtime-lib=dll", "-lkernel32"]);
        for library in ["libcmt", "libucrt", "libvcruntime"] {
            command
                .arg("-Xlinker")
                .arg(format!("/NODEFAULTLIB:{library}"));
        }
    }
    if shared {
        command.args(shared_flags());
    }
    if std::env::var_os("KESTREL_SANITIZE").is_some() {
        command.args([
            "-fsanitize=address,undefined",
            "-fno-sanitize-recover=undefined",
        ]);
    }
    command
}
fn shared_flags() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &["-dynamiclib"]
    } else if cfg!(windows) {
        &["-shared"]
    } else {
        &["-shared", "-fPIC"]
    }
}
/// The file name of a shared library that `-l{name}` finds. On Windows the linker reads
/// the import library Clang writes beside `{name}.dll`.
fn dylib(name: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("lib{name}.dylib")
    } else if cfg!(windows) {
        format!("{name}.dll")
    } else {
        format!("lib{name}.so")
    }
}
/// Links a module against the shared runtime in `folder`. Unix records the folder as a
/// search path; Windows finds the DLL beside the host program.
fn link_runtime(command: &mut Command, folder: &Path) {
    command.arg("-L").arg(folder).arg("-lkestrel_runtime");
    if !cfg!(windows) {
        command
            .arg(format!("-Wl,-rpath,{}", folder.display()))
            .arg("-lm");
    }
}

/// Builds one project C file into a shared library, so its state lives once and every
/// later entry calls into the same copy. Returns the file those entries link against,
/// or `None` when the file exports nothing.
///
/// A Windows DLL exports only the symbols it marks, and project C code marks none. The
/// MinGW linker exports every external symbol on request; the MSVC linker exports what a
/// module-definition file lists, so the session lists the object's external symbols.
fn project_library<'a>(
    folder: &Path,
    index: usize,
    source: &Path,
    program: &Program,
    earlier: impl Iterator<Item = &'a PathBuf>,
) -> Result<Option<PathBuf>, String> {
    let name = format!("foreign{index}");
    let library = folder.join(dylib(&name));
    let includes = || {
        program
            .c_include_dirs
            .iter()
            .flat_map(|d| [std::ffi::OsStr::new("-I"), d.as_os_str()])
    };
    let mut command = toolchain(true);
    let link = if !cfg!(windows) {
        command.arg(source);
        library.clone()
    } else if super::mingw() {
        let implib = folder.join(format!("lib{name}.dll.a"));
        let mut flag = std::ffi::OsString::from("-Wl,--out-implib,");
        flag.push(&implib);
        command
            .arg(source)
            .arg("-Wl,--export-all-symbols")
            .arg(flag);
        implib
    } else {
        let object = folder.join(format!("{name}.o"));
        compile(
            toolchain(false)
                .arg("-c")
                .arg(source)
                .args(includes())
                .arg("-o")
                .arg(&object),
        )?;
        let definition = folder.join(format!("{name}.def"));
        fs::write(
            &definition,
            module_definition(&dylib(&name), &external_symbols(&object)?),
        )
        .map_err(|e| e.to_string())?;
        command
            .arg(&object)
            .arg("-Xlinker")
            .arg(format!("/DEF:{}", definition.display()));
        folder.join(format!("{name}.lib"))
    };
    command
        .args(includes())
        .args(earlier)
        .args(program.c_libraries.iter().map(|name| format!("-l{name}")))
        .args(super::system_libraries())
        .arg("-o")
        .arg(&library);
    compile(&mut command)?;
    // The MSVC linker writes no import library for a DLL without exports.
    Ok(link.exists().then_some(link))
}

/// The external symbols an object file defines, as `llvm-nm` reports them: each name with
/// whether it names data rather than code.
fn external_symbols(object: &Path) -> Result<Vec<(String, bool)>, String> {
    let output = super::clang_command()
        .arg("-print-prog-name=llvm-nm")
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|tool| !tool.is_empty())
        .and_then(|tool| {
            Command::new(tool)
                .args(["--extern-only", "--defined-only"])
                .arg(object)
                .output()
                .ok()
        })
        .filter(|output| output.status.success())
        .ok_or_else(|| {
            format!(
                "Cannot find llvm-nm beside clang ({}); a session needs it to export project C functions under MSVC. Install the full LLVM package or set KESTREL_CC to the bundled Clang.",
                super::c_compiler().display()
            )
        })?;
    Ok(parse_symbols(&String::from_utf8_lossy(&output.stdout)))
}

/// Reads `llvm-nm` lines such as `0000000000000000 T next`. Only names a C declaration can
/// spell are kept; the compiler's own constants, such as `__real@...`, stay private.
fn parse_symbols(listing: &str) -> Vec<(String, bool)> {
    listing
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace().rev();
            let name = words.next()?;
            let kind = words.next()?;
            let spelled = name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            spelled.then(|| (name.to_owned(), !kind.eq_ignore_ascii_case("t")))
        })
        .collect()
}

/// A module-definition file exporting `symbols` from `library`. Data goes out as `DATA`,
/// so code that reaches it without `dllimport` fails to link instead of reading a thunk.
fn module_definition(library: &str, symbols: &[(String, bool)]) -> String {
    let mut text = format!("LIBRARY {library}\nEXPORTS\n");
    for (name, data) in symbols {
        writeln!(text, "    {name}{}", if *data { " DATA" } else { "" }).unwrap();
    }
    text
}

#[cfg(unix)]
type Control = UnixStream;
#[cfg(windows)]
type Control = std::net::TcpStream;

/// Opens the control channel: a Unix socket in the private session folder, or on Windows
/// a loopback port that only a worker holding the random token can claim. Returns the
/// listener and the address the worker takes as its first argument.
#[cfg(unix)]
fn control_listener(folder: &Path) -> Result<(UnixListener, String, Option<String>), String> {
    let socket = folder.join("control");
    let listener = UnixListener::bind(&socket)
        .map_err(|e| format!("Cannot open session control socket: {e}"))?;
    Ok((listener, socket.display().to_string(), None))
}
#[cfg(windows)]
fn control_listener(
    _folder: &Path,
) -> Result<(std::net::TcpListener, String, Option<String>), String> {
    use std::hash::{BuildHasher, Hasher};
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| format!("Cannot open session control socket: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    // Each RandomState carries fresh keys seeded from the operating system.
    let mut token = String::new();
    for round in 0..2u64 {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u64(round);
        write!(token, "{:016x}", hasher.finish()).unwrap();
    }
    Ok((listener, port.to_string(), Some(token)))
}

pub struct Runtime {
    directory: TemporaryDirectory,
    worker: Child,
    control: BufReader<Control>,
    python: bool,
    entries: usize,
    /// Each project C file loaded so far, with the file later entries link to reach it.
    /// A file that exports nothing has none.
    c_sources: BTreeMap<PathBuf, Option<PathBuf>>,
}
impl Runtime {
    pub fn new(arguments: &[String], null_input: bool, marked: bool) -> Result<Self, String> {
        // A Unix socket path has a short length limit, so Unix sessions live in /tmp.
        #[cfg(unix)]
        let directory = {
            let directory =
                TemporaryDirectory::new(Path::new("/tmp")).map_err(|e| e.to_string())?;
            fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
            directory
        };
        #[cfg(windows)]
        let directory =
            TemporaryDirectory::new(&std::env::temp_dir()).map_err(|e| e.to_string())?;
        let path = directory.path();
        let source = [
            include_str!("platform_runtime.c"),
            include_str!("runtime.c"),
            include_str!("blas_runtime.c"),
            include_str!("bigint_runtime.c"),
            include_str!("tensor_runtime.c"),
            include_str!("numerical_runtime.c"),
            include_str!("probability_runtime.c"),
            include_str!("unicode_data.c"),
            include_str!("unicode_runtime.c"),
            include_str!("sort_runtime.c"),
            include_str!("system_runtime.c"),
            include_str!("regex_runtime.c"),
        ]
        .join("\n");
        let (header, body) = runtime_parts(&source, cfg!(windows));
        fs::write(path.join("runtime.h"), format!("#pragma once\n{header}"))
            .map_err(|e| e.to_string())?;
        fs::write(path.join("runtime.c"), body).map_err(|e| e.to_string())?;
        let mut runtime = clang(true);
        runtime
            .arg("-DK_SESSION_RUNTIME")
            .arg(path.join("runtime.c"))
            .args(if cfg!(windows) { None } else { Some("-lm") })
            .arg("-o")
            .arg(path.join(dylib("kestrel_runtime")));
        if super::mingw() {
            // The MinGW linker reads an import library, not the DLL, so write one where
            // -lkestrel_runtime finds it.
            let mut implib = std::ffi::OsString::from("-Wl,--out-implib,");
            implib.push(path.join("libkestrel_runtime.dll.a"));
            runtime.arg(implib);
        }
        compile(&mut runtime)?;
        fs::write(path.join("host.c"), include_str!("session_host.c"))
            .map_err(|e| e.to_string())?;
        let host = super::executable_path(&path.join("host"));
        let mut command = clang(false);
        command.arg(path.join("host.c")).arg("-I").arg(path);
        link_runtime(&mut command, path);
        if cfg!(windows) {
            command
                .args(super::platform_link_flags(path)?)
                .arg("-lws2_32");
        } else if !cfg!(target_os = "macos") {
            command.arg("-ldl");
        }
        compile(command.arg("-o").arg(&host))?;
        let (listener, address, token) = control_listener(path)?;
        let mut worker = Command::new(&host);
        worker
            .arg(&address)
            .args(arguments)
            .env("KESTREL_SESSION_MARKED", if marked { "1" } else { "0" })
            .stdin(if null_input {
                Stdio::null()
            } else {
                Stdio::inherit()
            })
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        if let Some(token) = &token {
            worker.env("KESTREL_SESSION_TOKEN", token);
        }
        if std::env::var_os("KESTREL_SANITIZE").is_some() {
            // Windows loads the sanitizer runtime DLL from PATH.
            worker.envs(super::sanitizer_runtime_path().map(|path| ("PATH", path)));
        }
        let mut worker = worker
            .spawn()
            .map_err(|e| format!("Cannot start native session: {e}"))?;
        // The worker connects before it loads user code. A short timeout catches startup failure.
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let control = loop {
            match listener.accept() {
                Ok((control, _)) => break control,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if let Some(status) = worker.try_wait().map_err(|e| e.to_string())? {
                        return Err(format!(
                            "The native session stopped during startup ({status})."
                        ));
                    }
                    if std::time::Instant::now() >= deadline {
                        let _ = worker.kill();
                        let _ = worker.wait();
                        return Err("The native session did not start.".into());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(e) => return Err(e.to_string()),
            }
        };
        control.set_nonblocking(false).map_err(|e| e.to_string())?;
        let mut control = BufReader::new(control);
        if let Some(token) = token {
            // A loopback port is open to every local program; only our worker knows the token.
            let mut claimed = String::new();
            control
                .get_ref()
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .map_err(|e| e.to_string())?;
            let read = control.read_line(&mut claimed);
            control
                .get_ref()
                .set_read_timeout(None)
                .map_err(|e| e.to_string())?;
            if read.is_err() || claimed.trim_end() != token {
                let _ = worker.kill();
                let _ = worker.wait();
                return Err("The native session control channel was not the worker's.".into());
            }
        }
        Ok(Self {
            directory,
            worker,
            control,
            python: false,
            entries: 0,
            c_sources: BTreeMap::new(),
        })
    }
    /// Runs one entry. The result says whether the entry stopped early on a propagated Error.
    pub fn execute(&mut self, program: &Program) -> Result<bool, String> {
        let path = self.directory.path();
        if !program.python_sites.is_empty() && !self.python {
            let (header, body) = runtime_parts(include_str!("python_runtime.c"), cfg!(windows));
            fs::write(
                path.join("kestrel_python.h"),
                format!("#pragma once\n{header}"),
            )
            .map_err(|e| e.to_string())?;
            fs::write(path.join("python.c"),format!("#define PY_SSIZE_T_CLEAN\n#include <Python.h>\n#define K_SESSION_RUNTIME\n#include \"runtime.h\"\n{body}")).map_err(|e|e.to_string())?;
            let mut command = clang(true);
            command
                .arg(path.join("python.c"))
                .args(python_flags(program)?);
            link_runtime(&mut command, path);
            compile(command.arg("-o").arg(path.join(dylib("kestrel_python"))))?;
            self.python = true;
        }
        for source in &program.c_sources {
            if self.c_sources.contains_key(source) {
                continue;
            }
            let link = project_library(
                path,
                self.c_sources.len(),
                source,
                program,
                self.c_sources.values().flatten(),
            )?;
            self.c_sources.insert(source.clone(), link);
        }
        let source = path.join(format!("cell{}.c", self.entries));
        let library = path.join(format!(
            "cell{}.{}",
            self.entries,
            if cfg!(target_os = "macos") {
                "dylib"
            } else if cfg!(windows) {
                "dll"
            } else {
                "so"
            }
        ));
        fs::write(&source, emit(program, true)?).map_err(|e| e.to_string())?;
        let mut command = clang(true);
        command
            .arg(&source)
            .args(
                program
                    .c_sources
                    .iter()
                    .filter_map(|source| self.c_sources.get(source).and_then(Option::as_ref)),
            )
            .arg("-I")
            .arg(path);
        link_runtime(&mut command, path);
        for folder in &program.c_include_dirs {
            command.arg("-I").arg(folder);
        }
        if !program.python_sites.is_empty() {
            command.arg("-lkestrel_python").args(python_flags(program)?);
        }
        command
            .args(program.c_libraries.iter().map(|name| format!("-l{name}")))
            .arg("-o")
            .arg(&library);
        compile(&mut command)?;
        writeln!(self.control.get_mut(), "{}", library.display())
            .and_then(|_| self.control.get_mut().flush())
            .map_err(|e| format!("Cannot send code to the native session: {e}"))?;
        let mut response = String::new();
        if self
            .control
            .read_line(&mut response)
            .map_err(|e| e.to_string())?
            == 0
        {
            let status = self.worker.wait().map_err(|e| e.to_string())?;
            return Err(format!("The native session stopped ({status})."));
        }
        if response.trim() != "ok" && response.trim() != "abort" {
            return Err(response.trim().to_owned());
        }
        self.entries += 1;
        Ok(response.trim() == "abort")
    }
    pub fn is_running(&mut self) -> Result<bool, String> {
        self.worker
            .try_wait()
            .map(|status| status.is_none())
            .map_err(|e| e.to_string())
    }
    pub fn finish(&mut self) -> Result<(), String> {
        if self.worker.try_wait().map_err(|e| e.to_string())?.is_some() {
            return Ok(());
        }
        self.control
            .get_mut()
            .shutdown(std::net::Shutdown::Write)
            .map_err(|e| e.to_string())?;
        let status = self.worker.wait().map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("The native session stopped ({status})."))
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::{declares_function, module_definition, parse_symbols, runtime_parts};
    #[test]
    fn shared_runtime_header_keeps_types_and_erases_bodies_and_initializers() {
        let source = "typedef struct { int n; } Value;\nstatic const Value empty = {0};\nstatic int value(int n) { static int once; return n + once++; }\n";
        let (header, body) = runtime_parts(source, false);
        assert!(header.contains("extern const Value empty;"));
        assert!(header.contains("extern int value(int n) ;"));
        assert!(!header.contains("return"));
        assert!(body.contains("int value(int n) { static int once;"));
    }
    #[test]
    fn windows_runtime_header_imports_data_and_keeps_thread_locals_private() {
        let source = "/* a static var */\ntypedef struct { int n; } Cell;\nstatic int counter;\nstatic _Thread_local int depth;\nstatic void (*hook)(int);\nstatic _Atomic(int) flag;\nstatic void *alloc(size_t n) __attribute__((unused));\nstatic void *alloc(size_t n) { return malloc(n); }\n";
        let (header, body) = runtime_parts(source, true);
        assert!(header.contains("extern __declspec(dllimport) int counter;"));
        assert!(header.contains("extern __declspec(dllimport) void (*hook)(int);"));
        assert!(header.contains("extern __declspec(dllimport) _Atomic(int) flag;"));
        assert!(header.contains("extern void *alloc(size_t n) __attribute__((unused));"));
        assert!(!header.contains("depth"));
        assert!(!body.contains("dllexport) /* a"));
        assert!(header.contains("typedef struct { int n; } Cell;"));
        assert!(body.contains("__declspec(dllexport) \nint counter;"));
        assert!(body.contains("\n_Thread_local int depth;"));
        assert!(
            body.contains("__declspec(dllexport) \nvoid *alloc(size_t n) { return malloc(n); }")
        );
        assert!(declares_function(
            "static KString k_text(KString value) __attribute__((unused))"
        ));
        assert!(!declares_function(
            "static pthread_t k_thread __attribute__((unused))"
        ));
    }
    #[test]
    fn msvc_exports_list_c_names_and_mark_data() {
        let listing = "0000000000000000 T next\n0000000000000008 B count\n0000000000000000 R __real@4010000000000000\n0000000000000010 D table_2\n";
        let symbols = parse_symbols(listing);
        assert_eq!(
            symbols,
            [
                ("next".to_owned(), false),
                ("count".to_owned(), true),
                ("table_2".to_owned(), true)
            ]
        );
        assert_eq!(
            module_definition("foreign0.dll", &symbols),
            "LIBRARY foreign0.dll\nEXPORTS\n    next\n    count DATA\n    table_2 DATA\n"
        );
    }
}
