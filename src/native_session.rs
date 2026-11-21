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
    let output = command
        .output()
        .map_err(|e| format!("Cannot start Clang: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Session compilation failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}
/// Clang for one session module. On Windows every module links the DLL C runtime, so the
/// host, the runtime and each entry share one heap and one standard output buffer.
fn clang(shared: bool) -> Command {
    let mut command = Command::new("clang");
    command.args([
        "-std=c11",
        "-O2",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-Wno-unused-function",
        "-Wno-unused-variable",
        "-Wno-unused-parameter",
        "-Wno-unused-but-set-variable",
        "-Wno-unused-label",
    ]);
    command.args(super::thread_flags());
    if cfg!(target_os = "linux") {
        command.arg("-ldl");
    }
    if cfg!(windows) {
        command.arg("-fms-runtime-lib=dll");
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
    if ["KESTREL_LEAK_CHECK", "KESTREL_NO_POOL"]
        .iter()
        .any(|n| std::env::var_os(n).is_some())
    {
        command.arg("-DK_NO_POOL");
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
    c_sources: BTreeMap<PathBuf, PathBuf>,
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
            if cfg!(windows) {
                // A Windows DLL exports only the functions it marks, and project C files
                // do not mark theirs.
                return Err(format!(
                    "Sessions on Windows cannot load project C files yet ({}); use kestrel run or kestrel build.",
                    source.display()
                ));
            }
            let library = path.join(dylib(&format!("foreign{}", self.c_sources.len())));
            let mut command = Command::new("clang");
            command.args(["-std=c11", "-O2", "-pthread"]);
            command.args(shared_flags());
            if std::env::var_os("KESTREL_SANITIZE").is_some() {
                command.args([
                    "-fsanitize=address,undefined",
                    "-fno-sanitize-recover=undefined",
                ]);
            }
            command.arg(source);
            for folder in &program.c_include_dirs {
                command.arg("-I").arg(folder);
            }
            command
                .args(self.c_sources.values())
                .args(program.c_libraries.iter().map(|name| format!("-l{name}")))
                .arg("-lm")
                .arg("-o")
                .arg(&library);
            compile(&mut command)?;
            self.c_sources.insert(source.clone(), library);
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
                    .filter_map(|source| self.c_sources.get(source)),
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
    use super::{declares_function, runtime_parts};
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
}
