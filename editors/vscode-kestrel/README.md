# Kestrel for VS Code

**Kestrel** support for `.kes` files: highlighting, completions, hover, go to
definition, inlay hints, live proof goals, quick fixes, and the compiler's errors
as you type.

The extension is lightweight and syntactic. The Kestrel compiler stays the
source of truth for types, effects and ownership, so every error you see comes
from `kestrel check`.

## Features

**Writing code**
- Highlighting for declarations, keywords, types, constants, strings and
  f-strings, numbers, comments and operators.
- Completions for functions, classes, traits, enums, constants, keywords,
  built-in types and nearby names. Inside a proof they also include the theorems
  declared above the cursor, the theorem's parameters, and the names `intro`,
  `have` and `byContra` introduce.
- Member completion after a dot, filtered by the receiver's inferred type:
  `list.` on a `List[Int]` offers List methods, and `this.` offers the current
  class's members, including private fields declared in its constructor.
- Snippets for `function`, `class`, `test`, `match` and `if`.
- Quick fixes from the light bulb for code that is still missing: the trait
  methods a class owes, the `match` arms it leaves uncovered, and the missing
  cases in an explicit proof or a `cases`/`induction` tactic. Method stubs carry
  a `TODO` comment; match and proof arms are `{ }` so the checker keeps telling
  you what is left. For a partly covered case, the quick fix places a
  top-level arm after the existing arms, so it catches exactly the remaining
  values.

**Proving**
- A **Kestrel Proof State** view in the activity bar follows the cursor
  inside a `by { ... }` block. It lists the variables, hypotheses and target of
  every open goal, updates after typing, and turns green when the kernel accepts
  the proof. Goals keep the notation you wrote, such as `A ⊆ B`; a tactic
  unfolds a definition only when it needs to. A proof that still uses `sorry` shows an amber `sorry` badge.
  A proof relying on an unchecked solver result shows an amber `trusted SMT`
  badge. Linear certificates checked by the kernel keep the normal proved
  state. `kestrel check --trusted` lists the remaining trust.
  Tactic completion offers the shape of each tactic, and the proof constructors
  `And`, `OrLeft`, `OrRight`, `Iff`, `Exists` and `True` complete inside a proof.
  A theorem call shows its parameters in signature help, and a theorem name in a
  proof shows its statement on hover. `exact` reports a value in proof position as a type error. The compiler answers with structured goal data, so the view never
  parses the checker's printed output.
- Set goals support `∪`, `∩`, `ᶜ`, `∉` and `↔`. `ext` proves a set equality
  through its two subsets, and `byContra h` is the classical contradiction
  rule. `pushNeg` rewrites negations inward, and `have h := proof` infers the
  statement of the intermediate fact. The backslash spellings `\cup`, `\cap`, `\compl`, `\notin`, `\iff`, `\larr` and `\ot`
  expand as you type.
- Run **Kestrel: Show Proof State** from the command palette or click the
  Kestrel icon in the activity bar. **Kestrel: Refresh Proof State** re-requests
  the current goals.

**Reading code**
- Go to definition with F12 or the editor's definition click (Ctrl-click, or
  Command-click on macOS) for functions, classes, traits, enums and their
  cases, constants, fields and methods. It follows a qualified name such as
  `Mode.Fast` or `state.at(...)`, a bare enum case inside a `match`, and a
  declaration imported from another file in the project.
- `///` documentation comments above a declaration appear under its signature
  in hover and signature help, and in completion. The block is Markdown, so
  paragraphs, lists, code fences and links work; `//` stays an implementation
  note. A selective import carries the documentation with the declaration.
  Each declaration keeps its own block, so a class shows its documentation
  above its constructor's, and a documented `new` cannot hide the class's
  equations.
  Inline `$...$` and display `$$...$$` equations render with bundled MathJax
  in hover, completion and signature documentation, including hovers directly
  over a `///` block. Inline equations stay complete, align with the text
  baseline and scale with the editor font and window zoom. Rendering works
  offline in light, dark and high-contrast themes and needs no TeX installation. Source comments keep their editable markup;
  code spans, fenced code and escaped dollars stay literal. Invalid or
  unsupported TeX stays readable while you edit it.
- Hover for functions, records, classes, constants, locals, parameters and the
  standard library.
- Signature help and gray parameter names for record constructors, which take
  their fields as parameters: `Problem(` shows `variables`, `initial`, `goal`
  and `actions`.
- Selective imports bring the imported declarations with them, so hover,
  signature help, parameter hints and member completion work on a record or a
  function that lives in another file of the project.
- Hover for every tactic, with its shape, what it does to the goal, and an
  example.
- Inlay hints for inferred `let` and `var` types, such as `tree : Node[Int]`.
- Gray parameter names at call arguments, such as `value = 3`, with options to
  hide them or show them only on the cursor's line.
- Partial calls keep their function type, so `add(1.0, _)` shows
  `(b : double) -> double` rather than the call's result.
- An outline of classes, functions and constants.

**Checking code**
- The compiler checks your file as you type, and reports its errors with their
  hints. It runs as one long-lived `kestrel serve` process per package, and a
  rebuilt compiler starts a new one, so the editor never keeps serving rules
  from a binary you replaced. The standard library is checked once, and later
  keystrokes cost only the open file. On a warm process an edit is checked in a
  few milliseconds; an older compiler without `serve` falls back to one process
  per check.
- Project-local imports resolve from the file's path even before you save, so
  an unsaved buffer does not report a missing-import error. The editor resolves
  them the way `kestrel check` does: from the workspace folder when it holds a
  `kestrel.project` file, otherwise from the nearest package above the file. A
  workspace can hold several packages, in its top folder or in subfolders.
- A notebook cell is read behind the code cells above it, in document order,
  because the kernel runs a notebook as one session: a name, import or type that
  an earlier cell bound is defined, so completion, hover and signature help use
  it, and only the edited cell's diagnostics appear. A top-level `?` is valid in
  a cell; a plain `.kes` file still requires an enclosing fallible function for
  `?`, matching the compiler.

The editor also caches the parse, the inferred bindings and the declaration
lookup tables for the current text, so completion, hover, signature help, inlay
hints and coloring stay fast as a file grows. Chains keep their type:
`sales.sortValues(...)?.head(2).` offers the same DataFrame members as
`sales.`. When a chain reaches `T or Error`, the dot completion offers `?` so
the success type's members become available instead of an empty list.

## Imports and suggestions

A module import has three forms:

```kestrel
import std.linalg                 // bind the namespace: linalg.solve(A, b)
import std.linalg as la           // rename the namespace: la.solve(A, b)
import std.linalg.{solve, lu}     // bind names directly: solve(A, b), lu(A)
```

The editor completes standard module paths after `import ` and `import std.`,
and it completes exported names inside `std.linalg.{...}`, `std.stats.{...}`,
`std.math.{...}`, `std.random.{...}`, `std.prob.{...}` and the other standard
modules. Names imported selectively appear as ordinary suggestions afterwards.
A namespace import needs the qualifier for free functions and constants: `import
std.linalg` gives you `linalg.solve`, not a bare `solve`. The module's type
names are also available unqualified, so `import std.kesdas as kd` lets a
signature say both `: DataFrame` and `: kd.DataFrame`. Typing a dot after a complete module path,
as in `import std.linalg.`, offers that module's names and inserts the chosen
one as a selective import, `import std.linalg.{inverse}`.

Suggestions for numerical types follow the same rule. `import std.random.{Rng}`
makes `Rng.seeded(42)` available; `import std.prob.{Normal, Binomial, Poisson}`
brings in the distribution types and their static methods; `import
std.complex.{Complex}` brings in the complex class, including `Complex.i()`, its
fields and its methods. `Normal.` offers `create`, `pdf`, `logPdf`, `cdf` and
`sample`; `Normal.sample(mean = 0.0, sigma = 1.0, shape = (2, 3))` is inferred as
a static tensor. Tensors complete `negate()` and `power(...)`, and `A ** 2`,
`-A`, `z ** 2` and `-z` keep their result types. `A.T` is a read-only
transpose view; the editor types `A.T`, `A.T.T` and `A.T[i, j]`. Integer
literals infer as double in double contexts, including `a + 1` for a tensor
`a` and `math.exp(1)`. `import std.ml.{StandardScaler, LinearRegression}`
offers fitted-model completion and inference for `fit`, `transform`, `predict`
and the regression/classification metrics. The editor completes and infers the same API for all 23
distributions. The existing
implicit math names, such as `sqrt` and `abs`, stay available without an import;
the newer names, such as `exp` and `log`, need `import std.math` and are then
used as `math.exp(x)`.

## Settings

Open **Settings** and search for `kestrel`.

| Setting | Default | What it does |
|---|---|---|
| `kestrel.compilerPath` | `kestrel` on `PATH` | The compiler used for checking. In a Cargo checkout the release binary is preferred over the debug binary when it is at least as new. |
| `kestrel.checkOnType.enabled` | `true` | Check as you type. When off, a file is checked on save. |
| `kestrel.checkOnType.pauseMs` | `60` | Wait this long after your last keystroke before checking. |
| `kestrel.useServer` | `true` | Keep one warm `kestrel serve` process per package folder. Turn off for one process per check. |
| `kestrel.proofState.enabled` | `true` | Follow the cursor with the proof-state view. |
| `kestrel.inlayHints.parameterNames` | `always` | `always`, `cursorLine` (only on the cursor's line), or `off`. |
| `kestrel.inlayHints.parameterNameStyle` | `equals` | `value = 3` or `value: 3`. |
| `kestrel.inlayHints.skipMatchingNames` | `true` | Hide a name when the argument already has that name. |
| `kestrel.inlayHints.minimumArguments` | `1` | Show names only for calls with at least this many arguments. |
| `kestrel.inlayHints.types` | `true` | Show inferred types after `let` and `var` names. |

To switch between the parameter-name modes quickly, run **Kestrel: Cycle
Parameter Name Hints** from the Command Palette.

## Troubleshooting a stale compiler

The extension never checks syntax itself; it runs the binary named by
`kestrel.compilerPath`, or `kestrel` on `PATH`. A compiler built before the
tactic language reports `Expected '{', found 'by'` even though the extension is
current. Point the setting at the workspace release build, for example
`target/release/kestrel`, then reload the window so the warm `kestrel serve`
process starts the new binary. The proof-state view shows a checker error until
that process restarts.

## Install

Build an optimized compiler first. A debug build works, but its first
standard-library check is several times slower:

```sh
cargo build --release
./editors/install-vscode-extension.sh
```

If `kestrel` on `PATH` points into a Cargo `target/debug` folder, the extension
uses the newer release binary beside it automatically. You can also set
`kestrel.compilerPath` explicitly. Re-run the install script after changing the
extension's `out/*.js` files. Reload VS Code after installing (Command Palette:
**Developer: Reload Window**). Open a `.kes` file and check that the language
mode in the bottom-right corner says **Kestrel**.

## Limits

- Quick fixes need the warm checker (`kestrel.useServer`). An older compiler
  without the `actions` operation reports errors normally but offers no light
  bulb for them.
- Hover, completion, signature help and parameter hints read the open file, the
  declarations its selective imports name (`import facts.{Fact}`), and the
  standard module metadata in `out/`. A namespace import (`import facts`) adds
  the qualifier but not its members yet, and imports of imports are not followed.
  The compiler's `check` output remains the authority.
- Import-path completion offers the standard modules listed in `out/modules.js`;
  project-local module paths are not suggested after `import`.
- Parameter-name hints follow the cursor's line. VS Code does not tell
  extensions where the mouse is, so a hint cannot appear on hover alone.

## Documentation and feature checks

Trait default methods participate in member completion, return-type inference,
hover and signature help, including generic and associated types, inherited
traits and dynamic views. Theorems keep their parameter types in proofs and
appear in executable proof-argument positions. Direct recursive fields have
separate induction-hypothesis suggestions and semantic tokens. Imported
class and theorem documentation follows selective imports, including Git
sources selected by `kestrel.lock`. The compiler verifies cached source content
when checking; editor lookup does not fetch or update dependencies.

For extension development, run `npm ci`, `npm run build` and `npm test` here.
`npm run test:vscode` opens a separate installed VS Code instance with an isolated
profile, checks the actual providers and verifies that the math images load in
light and dark themes. Set `VSCODE_EXECUTABLE` to use another executable.
The shipped bundle includes the renderer and glyphs; development dependencies
and test tools are excluded from the VSIX.
