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
    writeln!(out,"void k_session_step(void ***globals,size_t *count,int argc,char **argv) {{\n    size_t previous=*count;KSessionVersion *version=calloc(1,sizeof *version);if(!version) k_fatal(\"Out of memory\");\n    version->previous=atomic_load_explicit(&k_session_version,memory_order_acquire);\n    version->globals=calloc({},sizeof *version->globals);if(!version->globals) k_fatal(\"Out of memory\");\n    if(previous) memcpy(version->globals,*globals,previous*sizeof *version->globals);\n    k_session_globals=version->globals;*globals=version->globals;*count={count};",count.max(1)).unwrap();
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
    out.push_str("    (void)argc;(void)argv;k_fn_0();\n    if(fflush(stdout)==EOF) k_fatal(\"Cannot write standard output\");\n}\nvoid k_session_drop(void ***globals) {\n    k_session_globals=*globals;\n");
    for (index, ty) in program.globals.iter().enumerate().rev() {
        if ty.owns() {
            writeln!(out, "    k_{}_drop(&g{index});", suffix(*ty)).unwrap();
        }
    }
    for index in 0..program.globals.len() {
        writeln!(out, "    free((*globals)[{index}]);").unwrap();
    }
    out.push_str("    *globals=NULL;\n}\nvoid k_session_modules_drop(void) {\n");
    if !program.python_sites.is_empty() {
        writeln!(out,"    if(!Py_IsInitialized()) return;PyGILState_STATE state=k_python_enter();for(size_t i=0;i<{};++i) {{Py_XDECREF(k_python_modules[i]);k_python_modules[i]=NULL;}}k_python_leave(state);",program.python_modules.len()).unwrap();
    }
    out.push_str("}\n");
}

/// Split our C runtime's top-level declarations from its definitions. Bodies and
/// initializers stay in the one shared library; cells include only declarations.
fn runtime_parts(source: &str) -> (String, String) {
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
        if bytes[i] == b'#' && source[start..i].trim().is_empty() {
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
                body.push_str(&declaration.replace("static ", ""));
                if declaration.contains("static ") {
                    let declaration = source[start..initializer.unwrap_or(i)].trim_end();
                    header.push_str(&declaration.replace("static ", "extern "));
                    header.push_str(";\n");
                } else {
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
fn clang(shared: bool) -> Command {
    let mut command = Command::new("clang");
    command.args([
        "-std=c11",
        "-O2",
        "-pthread",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-Wno-unused-function",
        "-Wno-unused-variable",
        "-Wno-unused-parameter",
        "-Wno-unused-but-set-variable",
        "-Wno-unused-label",
    ]);
    if cfg!(target_os = "linux") {
        command.arg("-ldl");
    }
    if shared {
        if cfg!(target_os = "macos") {
            command.arg("-dynamiclib");
        } else {
            command.args(["-shared", "-fPIC"]);
        }
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
fn dylib(name: &str) -> String {
    format!(
        "lib{name}.{}",
        if cfg!(target_os = "macos") {
            "dylib"
        } else {
            "so"
        }
    )
}

#[cfg(unix)]
pub struct Runtime {
    directory: TemporaryDirectory,
    worker: Child,
    control: BufReader<UnixStream>,
    python: bool,
    entries: usize,
    c_sources: BTreeMap<PathBuf, PathBuf>,
}
#[cfg(unix)]
impl Runtime {
    pub fn new(arguments: &[String], null_input: bool, marked: bool) -> Result<Self, String> {
        let directory = TemporaryDirectory::new(Path::new("/tmp")).map_err(|e| e.to_string())?;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        let path = directory.path();
        let source = [
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
        let (header, body) = runtime_parts(&source);
        fs::write(path.join("runtime.h"), format!("#pragma once\n{header}"))
            .map_err(|e| e.to_string())?;
        fs::write(path.join("runtime.c"), body).map_err(|e| e.to_string())?;
        compile(
            clang(true)
                .arg("-DK_SESSION_RUNTIME")
                .arg(path.join("runtime.c"))
                .args(["-lm", "-o"])
                .arg(path.join(dylib("kestrel_runtime"))),
        )?;
        fs::write(path.join("host.c"), include_str!("session_host.c"))
            .map_err(|e| e.to_string())?;
        compile(
            clang(false)
                .arg(path.join("host.c"))
                .arg("-I")
                .arg(path)
                .arg("-L")
                .arg(path)
                .args(["-lkestrel_runtime", "-lm"])
                .arg(format!("-Wl,-rpath,{}", path.display()))
                .args(if cfg!(target_os = "macos") {
                    vec![]
                } else {
                    vec!["-ldl"]
                })
                .arg("-o")
                .arg(path.join("host")),
        )?;
        let socket = path.join("control");
        let listener = UnixListener::bind(&socket)
            .map_err(|e| format!("Cannot open session control socket: {e}"))?;
        let mut worker = Command::new(path.join("host"))
            .arg(&socket)
            .args(arguments)
            .env("KESTREL_SESSION_MARKED", if marked { "1" } else { "0" })
            .stdin(if null_input {
                Stdio::null()
            } else {
                Stdio::inherit()
            })
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
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
        Ok(Self {
            directory,
            worker,
            control: BufReader::new(control),
            python: false,
            entries: 0,
            c_sources: BTreeMap::new(),
        })
    }
    /// Runs one entry. The result says whether the entry stopped early on a propagated Error.
    pub fn execute(&mut self, program: &Program) -> Result<bool, String> {
        let path = self.directory.path();
        if !program.python_sites.is_empty() && !self.python {
            let (header, body) = runtime_parts(include_str!("python_runtime.c"));
            fs::write(
                path.join("kestrel_python.h"),
                format!("#pragma once\n{header}"),
            )
            .map_err(|e| e.to_string())?;
            fs::write(path.join("python.c"),format!("#define PY_SSIZE_T_CLEAN\n#include <Python.h>\n#define K_SESSION_RUNTIME\n#include \"runtime.h\"\n{body}")).map_err(|e|e.to_string())?;
            compile(
                clang(true)
                    .arg(path.join("python.c"))
                    .args(python_flags(program)?)
                    .arg("-L")
                    .arg(path)
                    .arg("-lkestrel_runtime")
                    .arg(format!("-Wl,-rpath,{}", path.display()))
                    .arg("-o")
                    .arg(path.join(dylib("kestrel_python"))),
            )?;
            self.python = true;
        }
        for source in &program.c_sources {
            if self.c_sources.contains_key(source) {
                continue;
            }
            let library = path.join(dylib(&format!("foreign{}", self.c_sources.len())));
            let mut command = Command::new("clang");
            command.args(["-std=c11", "-O2", "-pthread"]);
            command.args(if cfg!(target_os = "macos") {
                vec!["-dynamiclib"]
            } else {
                vec!["-shared", "-fPIC"]
            });
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
            .arg(path)
            .arg("-L")
            .arg(path)
            .arg("-lkestrel_runtime")
            .arg(format!("-Wl,-rpath,{}", path.display()))
            .arg("-lm");
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
#[cfg(unix)]
impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::runtime_parts;
    #[test]
    fn shared_runtime_header_keeps_types_and_erases_bodies_and_initializers() {
        let source = "typedef struct { int n; } Value;\nstatic const Value empty = {0};\nstatic int value(int n) { static int once; return n + once++; }\n";
        let (header, body) = runtime_parts(source);
        assert!(header.contains("extern const Value empty;"));
        assert!(header.contains("extern int value(int n) ;"));
        assert!(!header.contains("return"));
        assert!(body.contains("int value(int n) { static int once;"));
    }
}
