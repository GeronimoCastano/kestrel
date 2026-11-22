use kestrel::paths::CanonicalPath;
use kestrel::{
    lexer::{TokenKind, lex},
    modules::{ProjectCheck, check_path, check_source, check_text},
    parse_file, parse_project,
    source::SourceFile,
};
use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::ExitCode,
};

fn main() -> ExitCode {
    // The Windows install ships OpenBLAS beside the compiler. Programs that kestrel runs
    // find it through KESTREL_LAPACK even when PATH does not name the install folder yet.
    if std::env::var_os("KESTREL_LAPACK").is_none()
        && let Some(library) = env::current_exe()
            .ok()
            .and_then(|path| Some(path.parent()?.join("libopenblas.dll")))
            .filter(|path| path.is_file())
    {
        // SAFETY: no other thread exists yet.
        unsafe { env::set_var("KESTREL_LAPACK", library) };
    }
    // Windows gives the main thread 1 MiB of stack, where macOS and Linux give 8 MiB. The
    // checker recurses on deep programs, so it runs on a thread with room to spare there.
    if cfg!(windows) {
        return std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(start)
            .map_or_else(
                |error| {
                    eprintln!("kestrel: cannot start the compiler thread: {error}");
                    ExitCode::from(2)
                },
                |thread| {
                    thread
                        .join()
                        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
                },
            );
    }
    start()
}

fn start() -> ExitCode {
    match run() {
        Ok(success) => {
            if success {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(message) => {
            eprintln!("kestrel: {message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<bool, String> {
    let mut args = env::args_os()
        .skip(1)
        .map(|arg| {
            arg.into_string()
                .map_err(|_| "A command-line argument is not valid UTF-8.".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter();
    let Some(command) = args.next() else {
        help();
        return Err("No command given. Run `kestrel --help` for the list.".into());
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        help();
        return Ok(true);
    }
    if matches!(command.as_str(), "--version" | "-V") {
        println!(
            "kestrel {} (compiler and native session, commit {})",
            env!("CARGO_PKG_VERSION"),
            env!("KESTREL_COMMIT")
        );
        return Ok(true);
    }
    if !matches!(
        command.as_str(),
        "parse"
            | "tokens"
            | "check"
            | "build"
            | "run"
            | "fetch"
            | "update"
            | "session"
            | "test"
            | "notebook"
            | "serve"
            | "goals"
            | "doctor"
            | "game"
    ) {
        return Err(format!(
            "Unknown command `{command}`. Use `kestrel --help`."
        ));
    }
    let mut project = false;
    let mut tree = false;
    let mut notebook = false;
    let mut trusted = false;
    let mut paths = vec![];
    let mut script_args = vec![];
    let mut output: Option<PathBuf> = None;
    let mut emit_c: Option<PathBuf> = None;
    let mut libraries = vec![];
    let mut filter = None;
    let mut jobs = None;
    let mut timeout = None;
    let mut timeout_given = false;
    let mut line = None;
    let mut column = None;
    let mut json = false;
    let mut port = None;
    let mut root = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut root_given = false;
    let mut stdin_path: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--filter" if command == "test" => {
                filter = Some(args.next().ok_or("--filter needs some text to match.")?)
            }
            "--jobs" if command == "test" => {
                let count = args.next().ok_or("--jobs needs a number.")?;
                jobs = Some(
                    count
                        .parse::<usize>()
                        .ok()
                        .filter(|n| *n > 0)
                        .ok_or(format!("`{count}` is not a positive number of jobs."))?,
                );
            }
            "--timeout" if command == "test" => {
                let value = args.next().ok_or("--timeout needs a number of seconds.")?;
                timeout_given = true;
                // `--timeout none` lets each test run as long as it needs.
                if value == "none" {
                    timeout = None;
                    continue;
                }
                timeout = Some(
                    value
                        .parse::<f64>()
                        .ok()
                        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
                        .and_then(|seconds| std::time::Duration::try_from_secs_f64(seconds).ok())
                        .filter(|duration| !duration.is_zero())
                        .ok_or_else(|| {
                            format!(
                                "`{value}` is not a positive, representable timeout in seconds."
                            )
                        })?,
                );
            }
            "--" if matches!(command.as_str(), "run" | "session") => {
                script_args.extend(args);
                break;
            }
            "--project" => project = true,
            "--tree" => tree = true,
            "--notebook" if command == "check" => notebook = true,
            "--trusted" if command == "check" => trusted = true,
            "--stdin-path" if command == "check" => {
                stdin_path = Some(args.next().ok_or("--stdin-path needs a file path.")?.into())
            }
            "--line" if command == "goals" => {
                line = Some(
                    args.next()
                        .ok_or("--line needs a one-based line number.")?
                        .parse::<usize>()
                        .map_err(|_| "--line needs a positive number.")?,
                )
            }
            "--column" if command == "goals" => {
                column = Some(
                    args.next()
                        .ok_or("--column needs a one-based column number.")?
                        .parse::<usize>()
                        .map_err(|_| "--column needs a positive number.")?,
                )
            }
            "--json" if matches!(command.as_str(), "goals" | "doctor") => json = true,
            "--port" if command == "game" => {
                port = Some(
                    args.next()
                        .ok_or("--port needs a port number.")?
                        .parse::<u16>()
                        .map_err(|_| "--port needs a number between 1 and 65535.")?,
                )
            }
            "--root" => {
                root = args.next().ok_or("--root needs a directory path.")?.into();
                root_given = true;
            }
            "-o" => output = Some(args.next().ok_or("-o needs an output path.")?.into()),
            "--emit-c" => {
                emit_c = Some(args.next().ok_or("--emit-c needs an output path.")?.into())
            }
            "--link" => {
                let name = args
                    .next()
                    .ok_or("--link needs a library name, such as z.")?;
                let valid = !name.is_empty()
                    && !name.starts_with('-')
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '+'));
                if !valid {
                    return Err(format!("`{name}` is not a library name."));
                }
                libraries.push(name)
            }
            "-" => paths.push(arg),
            _ if arg.starts_with('-') => return Err(format!("Unknown option `{arg}`.")),
            _ => paths.push(arg),
        }
    }
    if matches!(command.as_str(), "fetch" | "update") {
        if project
            || tree
            || notebook
            || output.is_some()
            || emit_c.is_some()
            || !libraries.is_empty()
            || !script_args.is_empty()
        {
            return Err(
                "fetch and update take only an optional --root; update also takes a package name."
                    .into(),
            );
        }
        if (command == "fetch" && !paths.is_empty()) || (command == "update" && paths.len() != 1) {
            return Err(if command == "fetch" {
                "fetch takes only an optional --root.".into()
            } else {
                "update takes one package name and an optional --root.".into()
            });
        }
        if command == "fetch" {
            let count = kestrel::modules::fetch_packages(&root)?;
            println!("Fetched and locked {count} Git package(s).");
        } else {
            let name = &paths[0];
            let count = kestrel::modules::update_package(&root, name)?;
            println!("Updated `{name}`; {count} Git package(s) are locked.");
        }
        return Ok(true);
    }
    if command == "test" {
        if project || tree || paths.len() > 1 || output.is_some() || emit_c.is_some() {
            return Err(
                "test takes an optional package folder, --filter, --jobs, --timeout and --link."
                    .into(),
            );
        }
        let folder = paths.first().map(PathBuf::from).unwrap_or(root);
        // A test that never ends would hang the run, so each test gets 60 seconds unless
        // --timeout says otherwise (REVIEW2 section 2).
        let timeout = if timeout_given {
            timeout
        } else {
            Some(std::time::Duration::from_secs(60))
        };
        let options = kestrel::testing::Options {
            timeout,
            filter,
            jobs: jobs
                .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get())),
            libraries,
        };
        return kestrel::testing::run(&folder, &options, &mut io::stdout());
    }
    if command == "notebook" {
        if project || tree || !paths.is_empty() || output.is_some() || emit_c.is_some() {
            return Err("notebook takes optional --root and --link.".into());
        }
        return kestrel::session::run_notebook(&root, libraries);
    }
    if command == "session" {
        if project || tree || !paths.is_empty() || output.is_some() || emit_c.is_some() {
            return Err("session takes optional --root, --link and -- ARG... .".into());
        }
        return kestrel::session::run(&root, libraries, &script_args);
    }
    if command == "game" {
        if project
            || tree
            || notebook
            || !paths.is_empty()
            || output.is_some()
            || emit_c.is_some()
            || !libraries.is_empty()
            || !script_args.is_empty()
        {
            return Err("game takes only an optional --port and --root.".into());
        }
        let games = if root_given {
            root
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("games")
        };
        return kestrel::game::run(port.unwrap_or(7878), &games).map(|()| true);
    }

    if command == "goals" {
        if project
            || tree
            || notebook
            || output.is_some()
            || emit_c.is_some()
            || !libraries.is_empty()
        {
            return Err("goals takes one file, optional --line, --column and --json.".into());
        }
        if paths.len() != 1 {
            return Err("goals takes exactly one file path.".into());
        }
        let path = &paths[0];
        let text = if path == "-" {
            let mut text = String::new();
            io::stdin()
                .read_to_string(&mut text)
                .map_err(|e| e.to_string())?;
            text
        } else {
            fs::read_to_string(path).map_err(|e| format!("Cannot read {path}: {e}"))?
        };
        let checked = check_text(path, &root, text).map_err(|e| e.to_string())?;
        let module = checked
            .modules
            .into_iter()
            .find(|module| module.parsed.source.name == "-")
            .ok_or("The checker did not return the edited file.")?;
        let line = line.unwrap_or(1);
        let column = column.unwrap_or(1);
        let offset = module.parsed.source.offset(line, column);
        let report = kestrel::proof::goals_at(&module.parsed, &module.checked, offset);
        if json {
            println!(
                "{}",
                serde_json::json!({
                    "theorem": report.theorem,
                    "goals": report.goals.iter().map(|goal| serde_json::json!({
                        "variables": goal.variables,
                        "hypotheses": goal.hypotheses,
                        "target": goal.target,
                    })).collect::<Vec<_>>(),
                    "cases": report.cases.iter().map(|case| serde_json::json!({
                        "label": case.label,
                        "variables": case.goal.variables,
                        "hypotheses": case.goal.hypotheses,
                        "target": case.goal.target,
                        "line": case.line,
                        "column": case.column,
                        "active": case.active,
                    })).collect::<Vec<_>>(),
                    "error": report.error,
                    "complete": report.complete,
                    "admitted": report.admitted,
                    "inTheorem": report.in_theorem,
                })
            );
        } else if report.theorem.is_empty() {
            println!("No theorem at {path}:{line}:{column}.");
        } else {
            println!("theorem {}", report.theorem);
            if let Some(error) = &report.error {
                println!("error: {error}");
            }
            if report.goals.is_empty() {
                println!("No goals. The proof is complete.");
            }
            for (index, goal) in report.goals.iter().enumerate() {
                println!();
                println!("goal {}/{}", index + 1, report.goals.len());
                for (name, ty) in &goal.variables {
                    println!("  {name} : {ty}");
                }
                for (name, prop) in &goal.hypotheses {
                    println!("  {name} : {prop}");
                }
                if !goal.variables.is_empty() || !goal.hypotheses.is_empty() {
                    println!("  --------------------");
                }
                println!("  ⊢ {}", goal.target);
            }
        }
        return Ok(report.error.is_none());
    }

    if command == "doctor" {
        if project
            || tree
            || notebook
            || !paths.is_empty()
            || output.is_some()
            || emit_c.is_some()
            || !libraries.is_empty()
            || !script_args.is_empty()
        {
            return Err("doctor takes optional --root and --json.".into());
        }
        let report = kestrel::doctor::run(&root)?;
        if json {
            println!("{}", report.json());
        } else {
            println!("{}", report.render());
        }
        return Ok(report.healthy());
    }

    if command == "serve" {
        if project
            || tree
            || notebook
            || !paths.is_empty()
            || output.is_some()
            || emit_c.is_some()
            || !libraries.is_empty()
            || !script_args.is_empty()
        {
            return Err("serve takes only an optional --root DIRECTORY.".into());
        }
        return kestrel::server::run(root).map(|()| true);
    }
    if paths.is_empty() {
        return Err("Supply a file path, or `-` to read standard input.".into());
    }
    if matches!(command.as_str(), "build" | "run") {
        if project || tree || paths.len() != 1 {
            return Err(
                "build and run take one file, with optional --root, --emit-c and --link.".into(),
            );
        }
        if command == "run" && output.is_some() {
            return Err("Use -o with build to save an executable.".into());
        }
        return native_command(
            &command,
            &paths[0],
            &root,
            output,
            emit_c,
            libraries,
            script_args,
        );
    }
    if output.is_some() || emit_c.is_some() || !libraries.is_empty() {
        return Err("Use -o with build; use --emit-c and --link with build or run.".into());
    }
    if command == "check" && (project || tree) {
        return Err("Use --tree and --project with the parse command.".into());
    }
    if notebook && (paths.len() != 1 || paths[0] != "-") {
        return Err(
            "--notebook checks one cell read from standard input; use `check --notebook -`.".into(),
        );
    }
    if paths.iter().filter(|path| path.as_str() == "-").count() > 1 {
        return Err("Standard input can be read only once.".into());
    }
    if stdin_path.is_some() {
        if command != "check" {
            return Err("--stdin-path is used with check.".into());
        }
        if notebook {
            return Err("--stdin-path cannot name a notebook cell.".into());
        }
        if paths.len() != 1 || paths[0] != "-" {
            return Err("--stdin-path checks one file read from standard input; use `check --stdin-path FILE -`.".into());
        }
    }
    let mut success = true;
    for path in paths {
        if command == "check" && path != "-" {
            let result =
                check_path(&path, &root).map_err(|e| format!("Cannot check {path}: {e}"))?;
            eprint!("{}", result.render_diagnostics());
            if trusted {
                print_trusted(&result);
            }
            if result.is_ok() {
                println!("{path}: checked names and supported types");
            }
            success &= result.is_ok();
            continue;
        }
        let text = if path == "-" {
            let mut text = String::new();
            io::stdin()
                .read_to_string(&mut text)
                .map_err(|e| e.to_string())?;
            text
        } else {
            fs::read_to_string(&path).map_err(|e| format!("Cannot read {path}: {e}"))?
        };
        if command == "check" {
            if let Some(source_path) = &stdin_path {
                let result = check_text(source_path, &root, text)
                    .map_err(|e| format!("Cannot check {}: {e}", source_path.display()))?;
                eprint!("{}", result.render_diagnostics());
                if trusted {
                    print_trusted(&result);
                }
                if result.is_ok() {
                    println!("-: checked names and supported types");
                }
                success &= result.is_ok();
                continue;
            }
            let name = if notebook { "<session>" } else { path.as_str() };
            let result = check_source(name, text);
            if trusted {
                print_module_trusted(&result);
            }
            for diagnostic in &result.checked.diagnostics {
                eprint!("{}", diagnostic.render(&result.parsed.source));
            }
            if result.checked.is_ok() {
                println!("{path}: checked names and supported types");
            }
            success &= result.checked.is_ok();
            continue;
        }
        if command == "tokens" {
            let source = SourceFile::new(&path, text);
            let lexed = lex(&source.text);
            for token in &lexed.tokens {
                if token.kind != TokenKind::Eof {
                    println!(
                        "{:?}@{}..{} {:?}",
                        token.kind,
                        token.span.start,
                        token.span.end,
                        source.slice(token.span)
                    );
                }
            }
            for diagnostic in &lexed.diagnostics {
                eprint!("{}", diagnostic.render(&source));
            }
            success &= lexed.diagnostics.is_empty();
            continue;
        }
        let parsed = if project {
            parse_project(&path, text)
        } else {
            parse_file(&path, text)
        };
        if tree {
            print!("{}", parsed.root.dump(&parsed.source));
        }
        for diagnostic in &parsed.diagnostics {
            eprint!("{}", diagnostic.render(&parsed.source));
        }
        if parsed.is_ok() && command == "parse" && !tree {
            println!("{path}: parses");
        }
        success &= parsed.is_ok();
    }
    Ok(success)
}

fn print_module_trusted(module: &kestrel::modules::ModuleCheck) {
    for theorem in kestrel::proof::trusted_theorems(&module.parsed, &module.checked) {
        let (line, column) = module.parsed.source.position(theorem.span.start);
        println!(
            "{}:{line}:{column}: trusted {}: {}",
            module.parsed.source.name,
            theorem.name,
            theorem.reasons.join(", ")
        );
    }
}
fn print_trusted(project: &ProjectCheck) {
    for module in &project.modules {
        print_module_trusted(module);
    }
}

fn native_command(
    command: &str,
    path: &str,
    root: &Path,
    output: Option<PathBuf>,
    emit_c: Option<PathBuf>,
    libraries: Vec<String>,
    arguments: Vec<String>,
) -> Result<bool, String> {
    let checked = if path == "-" {
        let mut text = String::new();
        io::stdin()
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        ProjectCheck {
            modules: vec![check_source("-", text)],
        }
    } else {
        check_path(path, root).map_err(|e| format!("Cannot read {path}: {e}"))?
    };
    let mut program = match kestrel::core::lower(&checked) {
        Ok(program) => program,
        Err(diagnostics) => {
            for located in diagnostics {
                eprint!(
                    "{}",
                    located
                        .diagnostic
                        .render(&checked.modules[located.module].parsed.source)
                );
            }
            return Ok(false);
        }
    };
    program.c_libraries = libraries;
    let destination = output.unwrap_or_else(|| {
        if path == "-" {
            "a.out".into()
        } else {
            Path::new(path).with_extension("")
        }
    });
    let resolved = |path: &Path| -> Result<PathBuf, String> {
        if path.exists() {
            return path.canonical().map_err(|e| e.to_string());
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        Ok(parent
            .canonical()
            .map_err(|e| e.to_string())?
            .join(path.file_name().ok_or("Output needs a file name.")?))
    };
    let sources = checked
        .modules
        .iter()
        .filter(|m| m.parsed.source.name != "-")
        .map(|m| PathBuf::from(&m.parsed.source.name))
        .collect::<Vec<_>>();
    let mut outputs = vec![];
    if command == "build" {
        outputs.push(resolved(&destination)?);
    }
    if let Some(path) = &emit_c {
        outputs.push(resolved(path)?);
    }
    for path in &outputs {
        if sources.contains(path) {
            return Err("An output path would overwrite an input source file.".into());
        }
    }
    if outputs.len() == 2 && outputs[0] == outputs[1] {
        return Err("The executable and C output need different paths.".into());
    }
    if let Some(path) = emit_c {
        kestrel::native::write_c(&program, &path)?;
    }
    if command == "build" {
        kestrel::native::build(&program, &destination)?;
        println!(
            "Built {}",
            kestrel::native::executable_path(&destination).display()
        );
        Ok(true)
    } else {
        Ok(kestrel::native::run_with_args(&program, &arguments)?.success())
    }
}

fn help() {
    println!(
        "Kestrel compiler (compiler and native session)\n\nUsage:\n  kestrel session [--root DIRECTORY] [--link LIBRARY] [-- ARG...]\n  kestrel notebook [--root DIRECTORY] [--link LIBRARY]\n  kestrel serve [--root DIRECTORY]
  kestrel goals [--root DIRECTORY] [--line N] [--column N] [--json] FILE
  kestrel doctor [--root DIRECTORY] [--json]\n  kestrel game [--port N] [--root DIRECTORY]\n  kestrel parse [--tree] [--project] FILE...\n  kestrel tokens FILE...\n  kestrel check [--root DIRECTORY] [--trusted] [--notebook] [--stdin-path FILE] FILE...\n  kestrel build [--root DIRECTORY] [-o EXECUTABLE] [--emit-c FILE] [--link LIBRARY] FILE\n  kestrel run [--root DIRECTORY] [--emit-c FILE] [--link LIBRARY] FILE [-- ARG...]\n  kestrel fetch [--root DIRECTORY]\n  kestrel update NAME [--root DIRECTORY]\n  kestrel test [FOLDER] [--filter TEXT] [--jobs N] [--timeout SECONDS|none] [--link LIBRARY]\n\nUse - as a file path to read standard input. --stdin-path FILE treats that
input as an unsaved version of FILE, so project-local imports resolve.\n\nparse checks grammar only. check resolves imports and checks supported\nnames and types, effects, mutation permissions, moves, borrows, contracts,\nfiles and tasks. build and run compile the supported native subset using\nClang. session keeps live values and runs each entry once; :quit ends it.\n:test [FILTER] runs saved package tests without clearing session values.\ntest runs the @test functions in a package's tests folder, each in its own\nprocess. doctor reports the tools, install layout, PATH, cache, lock and\neditor extension without changing anything. See README.md for current limits."
    );
}
