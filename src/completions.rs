//! Shell completion scripts, printed by `kestrel completions SHELL`.
//!
//! The command list is written out rather than derived at runtime, so a script
//! never depends on the binary it was generated from. Keep it in step with the
//! dispatch in `main.rs`.

/// The script for `shell`, or `None` when the name is not one of the three.
pub fn script(shell: &str) -> Option<&'static str> {
    match shell {
        "bash" => Some(BASH),
        "zsh" => Some(ZSH),
        "fish" => Some(FISH),
        _ => None,
    }
}

const BASH: &str = r#"# kestrel completions for bash
_kestrel() {
    local commands="parse tokens check build run fetch update new session test notebook serve goals game fmt doctor completions"
    if [ "$COMP_CWORD" -eq 1 ]; then
        COMPREPLY=( $(compgen -W "$commands" -- "${COMP_WORDS[1]}") )
        return
    fi
    COMPREPLY=( $(compgen -f -- "${COMP_WORDS[COMP_CWORD]}") )
}
complete -o filenames -F _kestrel kestrel
"#;

const ZSH: &str = r#"#compdef kestrel

_kestrel() {
    local -a commands
    commands=(
        'parse:check grammar only'
        'tokens:print the token stream'
        'check:check names, types, effects and contracts'
        'build:compile to a standalone executable'
        'run:compile and run'
        'fetch:resolve dependencies into kestrel.lock'
        'update:resolve one dependency again'
        'new:scaffold a package'
        'session:keep values alive between entries'
        'test:run @test functions'
        'notebook:serve one session for a notebook'
        'serve:answer editor requests'
        'goals:print proof goals at a position'
        'game:start the proof game'
        'fmt:format source in place'
        'doctor:diagnose the installation'
        'completions:print a completion script'
    )
    if (( CURRENT == 2 )); then
        _describe 'command' commands
    else
        _files
    fi
}

_kestrel "$@"
"#;

const FISH: &str = r#"# kestrel completions for fish
complete -c kestrel -f
complete -c kestrel -n "__fish_use_subcommand" -a "parse tokens check build run fetch update new session test notebook serve goals game fmt doctor completions"
"#;
