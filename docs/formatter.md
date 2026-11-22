# Formatter

`kestrel fmt` rewrites a `.kes` file in one style without changing what the
program means. It works from the token stream, so comments, strings, numbers and
the written form of `x++`, `x += 1` and `(a b : Nat)` survive. A formatted file
lexes to the same tokens, apart from trailing commas in comma-separated groups
and the order of import declarations.

Formatting is idempotent: a second run produces the same bytes. The syntax tree
and the diagnostics of a file are the same before and after.

## Use it

```sh
kestrel fmt file.kes                 # rewrite in place
kestrel fmt --check file.kes         # write nothing; exit 1 if a file would change
kestrel fmt --diff file.kes          # print a unified diff; write nothing
kestrel fmt --stdin < file.kes       # format standard input to standard output
kestrel fmt --check --diff file.kes  # print the diff and exit 1 on a change
```

`kestrel fmt` accepts several files. Use `-` as a file path to read standard
input. The compiler refuses a file with a lexical error, writes nothing and exits 2.

Exit codes:

| Code | Meaning |
|---|---|
| 0 | The command finished. Every file was already formatted, or was written. |
| 1 | `--check` found a file that would change. `--diff` alone exits 0. |
| 2 | Usage, read or lexical error. |

## Style

Four spaces per level and a 100-column limit. A construct stays on one line when
it fits. When it does not fit, the formatter breaks it in a way the parser
accepts.

```kestrel
public function summarize(
    values : List[double],
    label : String = "total",
) : Summary {
    let total := first + second + third +
        fourth + fifth
    let report := summarize(
        daily,
        label = "quarterly totals",
    )
    return if values.isEmpty() then Summary(0.0) else Summary(total)
}
```

The rules:

- Calls, lists, records and parameter lists flatten when they fit. Otherwise
  they put one item per line, indent four columns and keep a trailing comma.
- A function, theorem or record signature breaks its parameter list before it
  breaks the return type.
- Binary expressions break after an operator, never before one. The loosest
  operator in the expression breaks first, and tighter operators break only when
  their own part still does not fit. Kestrel stops an expression at a newline,
  so a break before an operator would change the parse.
- Match arms and induction cases stay on one line when the whole construct
  fits. Otherwise they go one per line.
- An inline `if ... then ... else if ...` chain breaks before an `else` when the
  line is full, so the branches keep the same indent.
- A member chain stays on one line. The grammar does not allow a newline before
  or after `.`, so a long chain stays long.
- A single long token, a long string or a member chain can exceed 100 columns.
  The formatter leaves it alone when there is no safe place to break.
- Blank lines: one between top-level declarations, at most one inside a block,
  none after `{` or before `}`. The file ends with one newline.
- Imports sort alphabetically inside a contiguous run, and the names inside
  `{...}` sort too. A blank line or a standalone comment starts a new run, so a
  comment never moves to a different import.
- `///` doc comments stay attached to their declaration. A blank line between a
  doc block and a declaration stays, because removing it would turn ordinary
  comments into documentation.
- Attributes go on their own line before the declaration.
- A trailing `//` comment stays on its line with one space before it.

## What it does not touch

- Literals keep their spelling: `1_000_000`, `0xFF`, `1E3` and `1.5e-3` stay as
  written.
- Nothing inside a string changes, including the expressions inside
  `f"{...}"`. The formatter treats a format string as one token.
- The formatter never rewords, joins or wraps a comment.

## Editor

The VS Code extension registers a formatting provider that runs
`kestrel fmt --stdin`, so `editor.formatOnSave` formats an unsaved buffer. No
Kestrel setting is needed; the extension uses `kestrel.compilerPath` when it is
set.

## Stability

Formatting output is stable within a minor release. Patch releases do not change
it. A minor release may change layout, with an entry in the changelog and a new
formatter revision. These stay true across releases:

- The same input produces the same bytes on every platform.
- A second run produces the same bytes as the first.
- Every comment survives.
- The syntax tree and the diagnostics do not change.

`kestrel fmt --version` reports the revision:

```text
kestrel formatter revision 1 (compiler 0.2.0)
```

CI runs the formatter in check mode over every tracked `.kes` file:

```sh
git ls-files -z '*.kes' | xargs -0 kestrel fmt --check
```
