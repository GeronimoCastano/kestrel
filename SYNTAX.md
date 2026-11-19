# Kestrel syntax

This page shows how Kestrel code looks. Each section has an example, followed by
the rules the compiler enforces.

## A first look

```kestrel
import std.collections.{List}

trait Shape {
    function area() : double
}

public class Square {
    has Shape

    public final side : double

    new(side : double) {
        this.side = side
    }

    public function area() : double = this.side * this.side
}

function biggest[S : Shape](a : S, b : S) : S =
    if a.area() >= b.area() then a else b
```

## Files and names

- A file is a module. Its path is its name, so `shop/prices.kes` is the module `shop.prices`. The file has no header line.
- Imports come first, then the declarations.
- Comments start with `//`.
- A package is a folder of files. A project file lists its name and its dependencies:

```kestrel
package geometry
depends matrix
depends vectors from "https://example.org/vectors.git" at "v1.2.0"
```
- The project file is named `kestrel.project`. `depends matrix` finds the folder `matrix` beside the package's folder, and its modules are imported with the package name first: `import matrix.ops.{scale}`.
- A Git dependency writes `from "url" at "ref"`, where the ref is a tag, branch or commit. `kestrel fetch` resolves new sources into `kestrel.lock`, recording each source URL, ref, exact commit and SHA-256 content hash. Commit the lockfile with the project.
- Builds use only the lockfile and verified per-user cache, so they work offline. `kestrel update vectors` resolves that dependency's configured ref again while preserving other pins. `fetch` preserves existing pins and repairs changed cache contents. Two different pins for one package name report `E_PACKAGE_VERSION` with both dependency paths. Fetching never compiles or runs package code; a registry remains deferred.
- Standard modules drop the `std.` prefix: `import math`, `import kesdas.{DataFrame}`, `import kesnet.optim.{Adam}`. A local module with the same path wins, so `import math` reads a local `math.kes` when one exists; `import std.math` selects the standard one. `import c` and `import python` stay reserved for foreign imports.
- Functions and variables use `camelCase`. Types and classes use `PascalCase`. Constants use `SCREAMING_SNAKE_CASE`.

## Programs

In a session, :test runs the package's saved tests; :test TEXT filters their names and paths. Your session bindings stay available afterward.

```kestrel
// report.kes: a script, so it runs top to bottom
let mut total := 0
for (let i : Int = 1; i <= 3; i = i + 1) {
    total = total + i
}
f"total is {total}"
```

- A file with top-level statements is a script. It runs from top to bottom and cannot be imported.
- A file with only declarations is a module. Importing it runs nothing.
- In a session, each line runs as it is typed, and names stay defined for later lines.
- A script gets its command-line arguments with `import std.process.{args}`.
- In a session, typing a function's name shows its signature and source.

## Printing

```kestrel
function report(total : Int) : void with Effects {
    print(f"total is {total}")
}
```

- `print` writes a value as text.
- A function that prints, or does any other outside work, is marked `with Effects`. The compiler rejects outside work without that mark. Mutation uses separate mut permissions.

## Reading files

```kestrel
function firstLine(path : String) : String or Error with Effects {
    using f := open(path)? {
        f.readLine()?     // f closes when the block ends
    }
}
```

- A function that reads a file is marked `with Effects`.
- The `using` block closes the file when it ends, even if the function returns early.

## Writing files and input

```kestrel
function saveLines(path : String, lines : List[String]) : void or Error with Effects {
    using w := create(path)? {
        for (let line : String in lines) {
            w.writeLine(line)?   // w closes when the block ends
        }
    }
}

function askAge() : Int or Error with Effects {
    let line := readLine()?    // text from the terminal
    parseInt(line)?            // a non-number becomes an Error
}
```

- Files are written with a `using` block, the same as reading.
- Typed input is read as text first, then parsed. A bad number becomes an `Error`.

## Named and default arguments

```kestrel
function scaled(x : double, scale : double = 1.0) : double = x * scale

scaled(2.0)                  // 2.0, the default scale is used
scaled(2.0, scale = 3.0)     // 6.0, named argument
scaled(scale = 3.0, 2.0)     // error: positional arguments come first
```

- A call names an argument with `=`. A default value sits in the parameter list.

## Numbers

```kestrel
let million := 1_000_000
let mask := 0xFF                       // hexadecimal
let big : Int = 9223372036854775807
let huge : BigInt = 1_000_000_000_000_000_000_000

big + 1                                // stops: integer overflow
let sum := big.checkedAdd(1)           // Int or Error, for when you want to handle it
7 / 0                                  // stops: division by zero
let quotient := 7.checkedDivide(0)     // Int or Error
```

- `Int` is 64-bit and fast. `BigInt` holds numbers of any size.
- Overflow and division by zero stop the program. The `checked` versions return `Int or Error`.

## Converting values to text

```kestrel
public class Square {
    has Text
    public final side : double
    new(side : double) { this.side = side }
    public function text() : String = f"square of side {this.side}"
}

f"{Square(2.0)}"                              // square of side 2.0
f"a set is {{1, 2}} and the total is {3}"     // prints: a set is {1, 2} and the total is 3
```

- A class that can become text declares `has Text` and writes `text()` itself.
- `debug()` shows a value's structure for developers, used with `f"{p:?}"`. If a class writes only `text()`, `debug()` gives the same result.

## Calling C and Python

```kestrel
import c "math.h"                  // the compiler reads the signatures from the header
let r := unsafe { sqrt(2.0) }      // unchecked call, marked by the unsafe block

import python "numpy" as np
let a := np.array([1.0, 2.0])      // Python values are checked when they run
```

- C calls go inside an `unsafe` block, so the risk is visible where it happens.
- Python values are checked when the code runs, so a type mistake shows up late.
- Python worker callbacks may call Python when the bridge is idle. If another thread owns it, the nested call returns RuntimeError.Python instead of waiting. Same-thread recursion still works, and pure callbacks can copy or release PythonValue handles. Tasks spawned inside a callback follow the same rule.

## Values

```kestrel
let total := 3.5               // the type is worked out automatically
let count : Int = 4            // the type is written out
let mut sum := 0.0             // can change later
sum = sum + total              // changes an existing "let mut" value
const LIMIT : Int = 10
```

- `:=` creates a name with an automatic type.
- `let name : Type =` creates a name with a written type.
- A plain `=` only changes a value declared with `let mut`.
- The basic types are `Int`, `double`, `Bool`, `String`, and `void`.
- A constant writes its type (`const LIMIT : Int = 10`) or infers it with `:=` (`const LIMIT := 10`). It has an immutable type and reads only stable values: immutable bindings, other constants, literals and pure calls. Reaching a mutable container or a mutable binding reports E_CONST_TYPE or E_CONST_DEPENDENCY.

## Functions

```kestrel
function add(a : double, b : double) : double = a + b

function describe(n : Int) : String {
    if n > 0 then "positive" else "not positive"
}
```

- `= expression` is for a one-line body. `{ ... }` is for a longer body. The last line of a block is its result.
- Names before one annotation share it: `function pow(base exp : double) : double` means `base : double, exp : double`. The same form works in theorem parameters and lambdas. A grouped default is rejected.
- A `_` marks a slot that is left open. `add(1.0, _)` is a new function that waits for the second number.
- The pipe `|>` passes a value along from left to right: `3.0 |> add(_, 1.0)` means `add(3.0, 1.0)`.

## Small inline functions (lambdas)

```kestrel
let twice := x => x * 2.0
let area := (w : double, h : double) => w * h
let scale := (x : Int) : Int => x * 1000
```

- Types are optional. Write them when they help.
- A lambda borrows the outside values it uses. Write `move` in front of it when it must keep them after it is created.

## Decisions and loops

```kestrel
let sign := if n > 0 then 1 else -1

if n > 0 {
    total = total + 1.0
}

for (let name : String in names) {
    count = count + 1
}

for (let i : Int = 0; i < 10; i = i + 1) {
    if i == 5 { break }
}

while total < 100.0 {
    total = total * 2.0
}

function firstNegative(xs : List[double]) : double {
    for (let x : double in xs) {
        if x < 0.0 { return x }
    }
    0.0
}
```

- `if` has two forms: `if c then a else b` for one line, and `if c { ... } else { ... }` for blocks.
- `for`, `while`, `break`, `continue`, and `return` are statements.

## Tuples

```kestrel
let person : (Int, String) = (30, "Ana")
let (age, name) := person              // destructuring gives both parts names

let scores := xs.filter(x => x > 0.0).map(x => x * 2.0)   // list steps are methods
```

- A tuple type is written `(Int, String)`.
- A tuple's parts are read by destructuring, so each part gets a name at once.
- A `let` whose pattern can fail takes an `otherwise` block, which runs when the pattern does not match:

```kestrel
let Some(v) := m.get("a") otherwise { return 0 }
v * 2
```

## Lists

```kestrel
let sides : List[double] = [1.0, 2.0, 3.0]
let doubled := [x * 2.0 for x in sides if x > 1.0]
```

- A list is persistent. Its contents cannot change in place.
- The square brackets around `x * 2.0` build a new list from an existing one. This is a comprehension.
- `xs.at(i)` reads one item from a List of Int, Nat, double, Bool or String. It runs in constant time,
  copies that one item and stops the program when `i` is outside the list. Use it for hot loops that
  would otherwise call `toArray()` and copy the whole list; `get(i)` would return an Option and needs a match.

## Maps

Map and Set iteration order is unspecified. SortedSet follows Comparable order.

```kestrel
let m := Map("a" => 1)
let n := m.put("b", 2)          // a new map; m still has only "a"

match m.get("b") {
    Some(v) => v,
    None    => 0,
}

let fast := MutMap()           // mutable, for hot loops
fast.put("a", 1)               // changes fast in place
```

- A `Map` never changes. `put` returns a new map.
- `MutMap` changes in place, for loops where speed matters.
- A lookup returns an `Option`, so a missing key is handled, not treated as a failure.

## Sets

```kestrel
let s := Set(1, 2, 3)
let t := s.add(4)              // a new set; s is unchanged
let inside := s.contains(2)    // true

let fast := MutSet()           // mutable, for hot loops
fast.add(1)                    // changes fast in place

let ordered := SortedSet(3, 1, 2)   // always 1, 2, 3
```

- A `Set` never changes. `add` returns a new set.
- `Set` makes no promise about order. `SortedSet` always keeps its elements in order.

## Concurrency

```kestrel
// run tasks in parallel; the block waits for all of them
scope {
    spawn save(report)
    spawn save(backup)
}?                      // ? passes on the first task's Error

// change shared state under a lock
let counter := Mutex(0)
function increment(counter : Mutex[Int]) : void or Error with Effects {
    using g := counter.lock()? {
        g.value = g.value + 1
    }
}
increment(counter)?

// send and receive messages
let c := Channel[Int]()
c.send(5)?
match c.receive() {
    Some(n) => n,
    None    => 0,           // closed and empty
}
```

- Tasks started in a `scope` finish before the scope ends.
- Shared data that changes is behind a `Mutex`, changed in a `using` block.
- Shared data that never changes, such as a `Map`, needs no lock.

## Classes

```kestrel
public class Account {
    private var balance : double

    new(start : double) {
        this.balance = start
    }

    public mut function deposit(amount : double) : void or Error requires amount > 0.0 {
        this.balance = this.balance + amount
    }

    public function current() : double = this.balance
}
```

- `new(...)` builds the object. `this` means the current object.
- `final` fields are set once. `var` fields can change.
- A private field may be declared inside the constructor with `final this.name : Type := value` or `var this.name : Type := value`, as in `new(start : Int) { final this.start : Int := start }`. The declaration is a direct statement of `new`, needs a written type, and is private. Public and package fields stay in the class body. A class with constructor-declared fields cannot be extended, because constructors do not chain and a subclass could not initialize those private fields.
- `requires` states a condition the input must meet.
- `public`, `private`, `package`, and `static` set who can see a member. `static` also marks static methods and static final fields; see Static fields at the end.
- A class can name traits in its body with `has`. A trait method may declare a signature or supply a default body. A class may omit a method with a default; replacing it requires `@override`. Distinct defaults for one signature require an explicit class method and otherwise report `E_TRAIT_CONFLICT`. Traits declare methods and associated types, not fields; a property requirement is a method such as `name()`.
- `public mut function ...` may change `this`; `mut other : Other` may change that borrowed argument. An ordinary parameter borrows read-only.
- Mutation alone needs no `with Effects`. A method that also prints or performs other outside work writes both modifiers.
- `class Square extends Figure` inherits Figure's fields and methods. `describe(Square(3.0))` works for `describe(f : Figure)`, and calls through `f` run Square's overrides. A place that owns a value keeps its exact class, so `let f : Figure = Square(3.0)` is refused; leave the type off, or use a trait and `List[Box[dyn Shape]]` to mix classes.

## Failures

`Error` is a standard root type. A named enum extends it with its own cases.
Returning that enum exposes a closed set of errors; returning Error allows any
extension and needs a fallback when matched.

```kestrel
enum ParseError extends Error {
    Parse(String)
    NotFound(String)
}

function parse(s : String) : Int or ParseError {
    if s == "" { return Parse(s) }
    42
}

// pass the error up: ? returns it from this function if there is one
function total(a : String, b : String) : Int or Error {
    let x := parse(a)?
    let y := parse(b)?
    x + y
}

// handle it here: match decides what to do
function totalOrZero(a : String) : Int = match parse(a) {
    Parse(msg) => 0,
    NotFound(msg) => 0,
    n          => n,
}
```

- `?` after a call means: if this gave an error, return that error from the current function now.
- The current function's return type must include `Error`, or the compiler refuses the `?`. At the top level of a script, `?` prints the Error on standard error and ends the program with exit status 1.
- A result that can be an Error cannot be thrown away. A call such as `save()` on its own line, when `save` returns `void or Error`, reports E_UNUSED_ERROR. Pass it on with `?`, handle it with `match`, or discard it on purpose with `let _ := save()`.
- `?` after an `Option` gives the value inside `Some`. If the value is `None`, the current function returns `None` at once, so it must return `Option`. Inside a class method, `this.left?` reads the field as a borrow, the same as a match.
- `if let Some(child) := this.left { child.size() } else { 0 }` is an expression. It works like a `match` with two arms, and `child` exists only in the first branch. `else` is required.
- `while h : s.size() > 0 { print(s.pop(h)) }` binds `h` as a proof that the stack is not empty. `pop(proof h : this.size() > 0)` takes that proof and returns the value, with no `Option`. `pop()` without a proof still returns `Option`, so the two overloads sit side by side. A proof parameter takes no argument at run time.
- The claim of a proof parameter must follow from the facts on the caller's path. A `while` loop's condition is a fact in its body until a call changes the stack, so a second `pop(h)` in the same body is refused.
- `match` handles both cases. A name pattern catches the value case.
- Enum cases are written bare when the type is clear from context. The type name is added only when two enums share a case name.

## Equality

```kestrel
let same := square1 == square2     // calls your equals method
let plain := add(n, Zero) = n      // in a theorem: a statement to prove
// 1 == 1.0 is true: the integer literal adopts double exactly
```

- `==` calls the `equals` method you write for the `Equals` trait.
- In a theorem, `=` states an equality that must be proved.
- `==` needs both sides to have the same type.

## Matching

```kestrel
enum Shape {
    Circle(double)
    Square(double)
}

function describe(s : Shape) : String = match s {
    Circle(r) if r > 10.0 => "big circle",
    Circle(_)             => "circle",
    Square(_)             => "square",
}
```

- `match` checks a value against each pattern in order, and runs the first one that fits.
- The compiler checks that every case is covered.

## Recursive types

```kestrel
enum Expr {
    Lit(Int)
    Add(Expr, Expr)
    Neg(Expr)
}

function eval(e : Expr) : Int = match e {
    Lit(n)    => n,
    Add(a, b) => eval(a) + eval(b),
    Neg(a)    => -eval(a),
}
```

- A case can contain its own type. The compiler stores the inner value on the heap for you.
- Generic types take their parameter in brackets: `enum Option[T] { Some(T), None }`.

## Trait objects

Sometimes a list holds different kinds of values that share a trait. You write `&dyn Shape` for "any shape, looked at without taking it":

```kestrel
function totalArea(shapes : List[&dyn Shape]) : double {
    let mut sum := 0.0
    for (let s : &dyn Shape in shapes) {
        sum = sum + s.area()
    }
    sum
}
```

- A `Box[...]` holds a value on the heap, and a `box(...)` call makes one.
- Where the type already says `Box[...]`, a plain value goes into a new box by itself: `let figures : List[Box[dyn Shape]] = [Figure(), Square(3.0), Circle(1.0)]`. Each item still gets its own heap box, and the value is handed over to it like `box(x)`. An item that is already a box goes in as it is.
- A long type can get a short name with `alias`: `alias ShapeRef = &dyn Shape`, then `List[ShapeRef]`.

## Text

```kestrel
let total := 3.5
let message := f"total is {total}"
let price := f"price is {total:.2f}"   // a colon sets the format
let plain := "braces stay {plain}"    // no f, so nothing is filled in
```

## Theorems

```kestrel
theorem addZeroR(n : Nat) : add(n, Zero) = n {
    match n {
        Zero    => rfl,
        Succ(k) => congrArg(Succ, addZeroR(k)),
    }
}
```

- A theorem states a fact, and its proof shows the fact is true. The proof is removed before the program runs, so it costs nothing at runtime.
- `example` is an unnamed theorem. It is checked, then discarded; it cannot be cited and adds no name. `axiom` is a named statement accepted without proof. The checker reports `W_PROOF_AXIOM`, and a function cannot require an axiom.
- `cases` and `induction` branches report one case goal per branch. The editor lists all branches and highlights the one under the cursor; clicking a branch moves the cursor into it.
- In a theorem, `=` means "equal, and this must be proved".
- An assumption is written with `requires`, and it gets a name: `requires h : n > 0`.
- Facts combine with the words `and`, `or`, and `not`.
- A proof builds `P and Q` as a tuple `(p, q)` or `And(p, q)`, takes one side with `h.left` or `h.right`, builds `P or Q` with `Left(p)`, `Right(q)`, `OrLeft(p)` or `OrRight(q)`, and builds `not P` as a lambda `h => ...` that reaches a contradiction. `Iff(f, g)`, `Exists(w, p)` and `True` build an iff, an existential and `true`.
- The named constructors are proof terms, not tactics. Write `exact(And(h1, h2))` in a script, or `have h := And(h1, h2)` when the statement should be inferred. A disjunction or existential needs a stated `have` type because its arguments do not determine the whole proposition.
- Either side of an `and` is a proof term: `h.left` and `h.right`, chained as in `h.right.left`. The base unfolds one definitional step, so `h.left` on `h : x ∈ A ∩ B` proves `x ∈ A`. An `or` has no side to project, because the statement does not say which side holds; `cases` remains its eliminator. A `cases` split names its new hypotheses after the fact: `cases h3` gives `h3l` and `h3r` (`h3mp`/`h3mpr` for an iff, `h3w` for an exists), so it cannot overwrite another fact's parts.
- `rfl`, `symm`, `trans`, `congrArg`, `absurd` and `decide` are the basic proof rules. A theorem cites the theorems above it, and itself only on a smaller value.
- Every proof must establish that its checked arithmetic can succeed. Bounds come before assumptions that evaluate arithmetic; unfolding preserves the work of unused let bindings. The checker reports E_PROOF_UNDEFINED when it cannot establish safety.
- Proofs may use visible imported pure functions through selective or namespace imports. Private helpers can unfold through those functions. Effects, `mut` parameters, trait-bounded generic functions and unsupported bodies remain opaque. A call to a function with a run-time `requires` needs the condition established, and stands for its success value. A function whose only preconditions are theorem requirements unfolds where the theorems' assumptions hold. Calls in proofs supply every argument positionally.
- A `public` theorem can be imported and cited: `import facts.{addZeroR}`, or `import facts as f` and `f.addZeroR(n)`. `simp [f.addZeroR]` names an imported rule.
- `induction n` works while an assumption mentions `n`; each use of `ih` must establish the assumptions at the smaller value from known facts. An assumption also says its own arithmetic succeeds, and each citation must establish that.
- Theorems use implicit type variables like functions: `theorem appendEnd(xs : Chain[T]) : append(xs, End) = xs { ... }` holds for every `T`, and `appendEnd(nats)` cites it at `Chain[Nat]`.

## Tactics

An explicit proof term is still valid. Add `by` before the braces for a tactic
script. The core tactics are `intro`, `intros`, `exact`, `assumption`, `apply`,
`constructor`, `left`, `right`, `exists`/`use`, `cases`, `induction`, `simp`,
`rw`, `have`, `symm`, `trans`, `exfalso`, `contradiction`, `rfl`, `decide`,
`trivial`, `smt`, `pushNeg`, `ext`, `byContra`, `skip`, `done`, `repeat` and `try`;
a `{ ... }` block is a bullet for
the current goal. A statement may use `forall n : Nat, P`, `exists n : Nat, P`,
`P -> Q` and `true`. Quantified types are Nat, Int, Bool, an enum or a theorem
type variable. `cases` and `induction` take one tactic block per constructor, and
each direct recursive field of an `induction` case takes a hypothesis name after the pattern, in field order. A case with no recursive fields takes no names.
`cases h3` names its new hypotheses after the fact it splits (`h3l`, `h3r`,
`h3mp`, `h3mpr`, `h3w`), and `cases h` keeps `hl`, `hr`, `hmp`, `hmpr` and
`hw`.
The script must close every goal unless it uses `sorry`, which admits the goal
with a warning. Theorem parameters may be proof hypotheses, as in
`h : x ∈ A`; `Set[U]`, `x ∈ A`, `A ⊆ B`, `A ∪ B`, `A ∩ B` and `Aᶜ` are
proof-level set expressions. `x ∉ A` is `not (x ∈ A)`. `P ↔ Q` is an iff;
`constructor` splits it. `ext` splits a set equality into two subsets, and
`byContra h` proves `P` classically by assuming `h : not P` and changing the
goal to `false`. `pushNeg` rewrites negations inward, so `not (not P)`
becomes `P`. `have h := p` infers the
statement of the local proof `p`; write `have h : P := p` when the statement
should be explicit.
`MATH.md` gives the full table, examples and the live goal protocol.

```kestrel
@simp
theorem addZeroR(n : Nat) : add(n, Zero) = n {
    induction n {
        Zero       => rfl,
        Succ(k) ih => congrArg(Succ, ih),   // ih : add(k, Zero) = k
    }
}

theorem t(n : Nat) : add(n, Zero) = n {
    simp                    // uses every @simp rule
}

function count(n : Nat) : Nat requires addZeroR(n) {
    n                       // the body may use the fact addZeroR(n)
}
```

- `@simp` marks a theorem as a rule. A plain `simp` uses every marked rule.
- `simp [rule]` adds a checked theorem or a named equality assumption. The simplifier rewrites left to right, unfolds pure functions and closes the resulting equality. Conditional rules need their assumptions; expanding or cyclic rules stop with a diagnostic.
- A function may require a theorem. Functions without a requirement are still allowed.
- The caller must establish a required theorem's assumptions at compile time. The function body may use its conclusion. This requirement adds no Error result or run-time check.

## Lists with a known length

```kestrel
enum Vec[T](n : Nat) {
    Nil : Vec[T](0)
    Cons(x : T, xs : Vec[T](n)) : Vec[T](n + 1)
}

function head(v : Vec[T](n + 1)) : T = match v {
    Cons(x, _) => x,
}
```

- `Vec[Int](3)` is a list of exactly three whole numbers.
- `head` only accepts a list with at least one element. An empty list is refused before the program runs.
- Each case states its length, so a wrong length is caught.
- In a match arm, the case fixes the length: in the `Nil` arm of a function over `Vec[T](n)`, n is 0, and in the `Cons(x, rest)` arm n is one more than the length of `rest`. So `append(xs : Vec[T](n), ys : Vec[T](m)) : Vec[T](n + m)` checks.
- A constant may multiply a length: `Vec[T](2 * n)` means `Vec[T](n + n)`, and `n * 3` means `n + n + n`. Parenthesized sums can be scaled too. Products of two length names, such as `n * m`, remain refused. Lengths, constant parts and coefficients must fit Nat's supported integer range.

## Sharing by borrowing

```kestrel
class Panel {
    public final square : &Square

    new(square : Square) {
        this.square = square
    }

    public function area() : double = this.square.area()
}

let sq := Square(2.0)
let p := Panel(sq)      // fine: sq lives longer than p
```

- Parameters borrow by default, so reading needs no `&`. `move` takes ownership.
- A field that stores a borrow says so in its type, as in `&Square`. A borrow cannot outlive its owner.
- Many parts can read a value at once. Only one part can change it at a time.
- No lifetime names are written. The compiler works them out.
- Two values that point at each other (a cycle) are refused.

## Tests

```kestrel
import examples.rules.{Square, biggest}
import std.test.{assertEq}

@test
function biggestIsSymmetric() : void {
    let a := Square(1.0)
    let b := Square(3.0)
    assertEq(biggest(a, b).area(), biggest(b, a).area())
}
```

- Tests are functions marked `@test`, and they live in the package's `tests` folder, which mirrors the code's folders: `tests/shop/prices.kes` tests `shop/prices.kes`.
- A test takes no parameters and returns `void` or `void or Error`. A returned Error fails the test, so `?` works inside it.
- A test file may use the `package` members of the folder it mirrors.
- `assertEq(a, b)` checks that two values are equal, and a failure shows both with `debug()`.
- `kestrel test` runs every test in its own process, several at once, and reports each failure.

## Operators

```kestrel
let ok := n > 0 && n < 10 || n == 100   // && and || stop early when they can
let masked := (flags & mask) == 0       // parentheses are required when mixing kinds
let result := x + 1 |> twice            // the pipe binds loosest: twice(x + 1)
let area := 3 * side ** 2               // ** binds tighter than *
let flipped := -(2 ** 2)                // a sign before ** needs parentheses
let bits := 1 << 3                      // a shift count runs from 0 to 63
```

- Arithmetic, comparison, and logic operators follow the Java and C order, applied strictly. Mixing them without parentheses is a compile error, except that a comparison may sit directly inside `&&` and `||`.
- A compound assignment `place op= value` changes a mutable name, field or Array slot in one step. `this.balance += amount`, `counts[1] *= 2`, and `x %= 5` work for `+=`, `-=`, `*=`, `/=` and `%=`. The place is read and then written, so it may not contain a call: store the call's result in a name first.
- `**` binds tighter than `*`. Write `-(2 ** 2)` or `(-2) ** 2`, and `2 ** (3 ** 2)` for a chain. `Int ** Int` stops the program on overflow or a negative exponent.
- A shift count outside 0 to 63 stops the program. `<<` keeps the low 64 bits; `>>` keeps the sign.
- `&&` and `||` skip the right side when the left side already decides the answer.
- The pipe `|>` binds loosest of all.
- The left operand decides which operator form runs. A class can declare a right-side form, such as `RightMultipliable[double]`, so that `2.0 * v` works.
- A class gets `+`, `-`, `*` and `/` by naming the trait and writing the method: `has Addable` with `add(other : Vector) : Vector`, and likewise `Subtractable`/`subtract`, `Multipliable`/`multiply`, `Divisible`/`divide`.
- `infixl 10 "⊕" = add` maps a new binary operator to an existing function. The declaration is syntax, not a proof; a use is an ordinary call. Add `as \oplus` to give the glyph a portable backslash spelling: `infixl 10 "⊕" as \oplus = add`. `infixr` groups to the right and `infix` groups to the left. Precedence runs from 1 (loosest) to 13 (tightest). The declaration is file-local for now.
- Proof statements accept `x \in A`, `A \subseteq B` and the Unicode `∈`/`⊆` forms. Membership and subset have no word spellings. `\forall`, `\exists`, `\to`, `\land`, `\lor`, `\not`, `\ge`, `\le` and `\ne` are aliases too. `forall` and `\forall` mean the same binder. The parser needs no editor support, and the VS Code extension can expand a backslash name on a space.

## Arrays

```kestrel
let a := Array(3, 0)    // three slots, each 0
a[0] = 5
let x := a[0]           // 5
let y := a.get(9)       // None; a[9] would stop the program
```

- An `Array` changes in place and moves like a class. Writing a slot needs the same `mut` access as `MutMap.put`.
- `a[i]` stops the program on a bad index; `a.get(i)` returns `Option` when you want to handle it.
- An Array whose elements implement Text implements Text too, so `print(a)` writes `[1, 2, 3]`; an empty Array prints `[]`.

## Array factories

```kestrel
class Point {
    public final x : Int
    new(x : Int) { this.x = x }
}

let points := Array.generate(3, i => Point(i))
print(points[0].x)
points[1] = Point(9)
print(points[1].x)
```

- The factory runs once per index in increasing order. A zero size calls it zero times; a negative size stops the program.
- Each slot owns its result. Reading a non-copyable element borrows it; replacing a slot releases the previous owner. A live element borrow prevents replacement.
- An effectful factory needs `with Effects` in its caller. `Array(size, value)` still requires a copyable value.

## Converting to an Array

```kestrel
let list := [1, 2, 3]
let array := list.toArray()
array[0] = 10
print(array)                                // [10, 2, 3]
print(range(3).map(n => n * 2).toArray())   // [0, 2, 4]
```

- `toArray()` works on any Iterable and copies its items, in order, into a new Array of the same size. The source runs once.
- The items must be copyable. A List of classes is refused with E_TO_ARRAY; build those Arrays with `Array.generate` instead.

## Growing an Array

```kestrel
let mut squares : Array[Int] = Array(0, 0)
for (i in range(4)) {
    squares.append(i * i)
}
print(squares)            // [0, 1, 4, 9]
print(squares.length())   // 4
```

- `append(value)` adds a slot at the end and grows the Array in place. `Array(0, 0)` starts empty, and `length()` reports the current size.
- Growing can move every element, so `append` is refused while a borrow of an element is still in use (E_BORROW_CONFLICT).

## Joining and searching

```kestrel
let words := ["north", "east", "south"]
print(words.join(", "))            // north, east, south
match words.index("east") {
    Some(i) => print(i),           // 1
    None => print(-1),
}
```

- `join(separator)` turns any Iterable of Text items into one String. `index(value)` returns `Option[Int]`, the position of the first equal item; it is defined for List and Array only.
- `prepend(value)` adds a slot at the front of an Array and moves the other items one slot right.

## Basic methods

```kestrel
print("  kestrel ".trim().toUpper())   // KESTREL
print((-4).abs())                      // 4
print([3, 1, 2].isEmpty())             // false
```

- Strings have trim, toUpper, toLower, isEmpty, contains, startsWith, endsWith and split. Numbers have abs, min and max, plus isEven and isOdd for Int, and sqrt, pow, floor, ceil, round and isFinite for double.
- Any Iterable (List, Array, Set, Map, or a lazy sequence) has isEmpty, any and all (which take a test), and first, last, min and max, which return Option. An item that cannot be copied comes out as a view, Option[&T], as peek does. min and max need Comparable items.
- An Option has isSome and isNone.
- The math functions max, min, abs, pow, sqrt, floor, ceil and round, and the constants PI and E, are available in every program without an import. A declaration of your own with the same name wins.

## Standard data structures

- Stack, Queue, MinHeap and MaxHeap are available in every program without an import. Each keeps its items in an Array.
- A Stack offers push, pop, peek and size. Its peek gives a read-only view of the top value.
- A Queue offers enqueue, which adds at the back, dequeue, which removes and returns the front item, and first, which gives a read-only view of the front item. Nothing else.
- MinHeap and MaxHeap take a Comparable item type and offer push, pop, peek and size. Pop removes the smallest or largest item.
- A program cannot declare a class with one of these names.
## Opaque C pointers

```kestrel
import std.c.{CUInt64}
import c "stdlib.h"

function allocate() : void or Error with Effects {
    let memory := unsafe { malloc(CUInt64(64)?) }
    if memory.isNull() { return RuntimeError.Io("allocation failed") }
    unsafe { free(memory) }
    print("released")
}

match allocate() {
    RuntimeError.Io(message) => print(message),
    _ => {},
}
```

- `CPointer[T]` and `CConstPointer[T]` carry an address and a target type. They copy the address and release nothing on scope exit. The programmer keeps the address valid and frees it through the C library.
- A mutable pointer can become a const pointer; either can become the matching void pointer. The reverse conversions require a C wrapper.
- `isNull()` compares an address with null. C calls still require `unsafe` and Effects. Dereference, pointer arithmetic, structs by value and variadic functions require C wrappers.
- `CChar` follows the C target's plain-char range and signedness. Its checked constructor returns `CChar or Error`; `toInt()` returns Int.

## Dynamic iteration

```kestrel
class Countdown { has Iterator
    public var nextValue : Int
    new(n : Int) { this.nextValue = n }
    public mut function next() : Option[Int] =
        if this.nextValue == 0 then None else {
            this.nextValue = this.nextValue - 1
            Some(this.nextValue)
        }
}

class Range { has Iterable
    public final count : Int
    new(count : Int) { this.count = count }
    public function items() : Countdown = Countdown(this.count)
}

function total(mut values : &dyn Iterable(Item = Int)) : Int with Effects {
    let mut result := 0
    for (value in values) { result = result + value }
    result
}

let values : Box[dyn Iterable(Item = Int)] = box(Range(4))
print(total(values))
```

- The compiler hides each implementation's cursor during dynamic traversal and releases it on completion, break or return. A cursor may borrow its iterable.
- Unknown iteration needs mut and Effects, as a generic Iterable bound does. A concrete iterator keeps its own permissions.
- Copyable items copy out of a cursor, including when next returns an Option stored in a field. Strings and immutable collections keep their shared storage alive after the cursor advances.
- Noncopyable items borrow for one step; toList retains their owners. A custom cursor may return a borrow of a stored Option; the item is lent for one step, and collecting it with toList still needs an owned Option.

## Arithmetic proofs

```kestrel
theorem positive(n : Int) : n > 0 requires h : n >= 1 { smt }

theorem successor(n : Nat) : (n + 1) > n { smt }

let n : Nat = 3
print(n + 1)
print(n - 1)
```

- `smt` uses an installed Z3 during checking; compiled programs need no solver. The kernel independently checks certificates for supported linear integer goals. `kestrel check --trusted FILE` lists successful solver results it cannot check, including nonlinear goals, plus axioms, admissions and proofs that cite them.
- The first backend accepts Int `+`, `-`, `*`, Nat `+`, `-`, comparisons, Bool logic and conditional expressions. It respects short-circuit evaluation. Function calls and enum values need explicit proofs or case splitting.
- In a proof, `Int` is the integers and `Nat` the natural numbers, so a theorem carries no overflow conditions: `(n + 1) > n` holds for every `n`. A Nat subtraction below Zero is still undefined, so `n - 1` needs `n > 0` first.
- Unknown answers, timeout, malformed output and missing tools fail checking. `KESTREL_Z3` selects an executable; queries have a two-second solver limit and a three-second process limit.
- At run time, Nat `+` stops on overflow and `-` stops below Zero. `checkedAdd` and `checkedSubtract` return `Nat or Error`. A program never continues with a wrong value, so a fact proved over the unbounded numbers stays true wherever the program runs.

## Arenas

```kestrel
import std.memory.{Arena, ArenaRef}

class Node {
    public final label : String
    public final next : Option[ArenaRef[Node]]
    new(label : String, move next : Option[ArenaRef[Node]]) {
        this.label = label
        this.next = next
    }
}

function nextLabel(nodes : Arena[Node], h : ArenaRef[Node]) : String =
    match nodes.get(h) {
        Some(node) => match node.next {
            Some(next) => match nodes.get(next) {
                Some(link) => link.label,
                None => "missing",
            },
            None => "end",
        },
        None => "missing",
    }

function graph() : void or Error with Effects {
    let nodes := Arena[Node]()
    let a := nodes.insert(Node("a", None))
    let b := nodes.insert(Node("b", Some(a)))
    nodes.replace(a, Node("a", Some(b)))?
    print(nextLabel(nodes, a))
    print(nextLabel(nodes, b))
}

graph()?
```

- `Arena[T]` owns its nodes; `ArenaRef[T]` is a copyable arena/slot identifier. Import both types from std.memory.
- `insert(value)` moves a value into storage and returns a handle. `get(handle)` returns an Option of a read-only borrow; copyable values retain their usual copy behavior. Read methods need no `&` at a call.
- `replace(handle, value)` moves a replacement into an existing slot and returns void or Error. Another arena's handle gives None from get or RuntimeError.Contract from replace. A rejected replacement releases its value.
- Mutation needs exclusive access and a mut parameter when you borrow the arena. Memory operations need no Effects. Live views prevent insertion or replacement.
- Handles compare by identity and may outlive their arena as inert identifiers. Arena storage never reuses an identity. Its owner releases nodes in reverse insertion order on exit.
- The constructor takes no values. Handles have no public constructor, and the API has no removal or mutable lookup operation.


## Exponent literals

```kestrel
let scale := 1e3
let fraction := 2.5E-4
print(scale)
print(f"{fraction:.6f}")
```

- An exponent makes a decimal literal double, with or without a decimal point. Both e and E accept an optional exponent sign.
- A single underscore may separate digits in the mantissa and exponent. The exponent must contain digits; 1e, 1e+ and 1e_3 are errors.
- Hexadecimal digits e and E remain part of an Int. Other numeric conversion rules stay in force.

## Associated types

```kestrel
trait Source {
    type Item
    function next() : Option[Item]
}

class Counter { has Source
    public final value : Int
    new(value : Int) { this.value = value }
    public function next() : Option[Int] = Some(this.value)
}

function read[T : Source(Item = Int)](source : T) : Int =
    match source.next() { Some(n) => n, None => 0, }

print(read(Counter(7)))
```

- type Item declares an associated type in a trait. It takes no default or bound.
- Each implementation supplies one consistent type through its method signatures. has Source(Item = Int) supplies an explicit binding when signatures leave a choice or do not mention Item.
- A generic bound can fix an associated type, as in Source(Item = Int). A dynamic view must fix all the trait's associated types, such as &dyn Source(Item = Int).
- Several method overloads must lead to one binding. Conflicting, missing or ambiguous bindings are errors. Associated type names have the trait's scope, independent of same-named module types.

## Namespace imports

```kestrel
import std.math as m
import std.str

let root := m.sqrt
print(root(9.0))
print(str.length("λx"))
```

- A whole-module import binds its last path component. import project.geometry binds geometry. An optional as name supplies a different local name.
- Qualify exported values, such as geometry.area(p). Exported type names are also available unqualified, so `let p : Point` and `let p : geometry.Point` both work after the import. Free functions and constants still need the qualifier; a constructor follows its type name. Public and package visibility follow the same rules as selective imports.
- If two whole-module imports export the same type name, the later import reports E_AMBIGUOUS_TYPE. Use the qualified name or a selective import to choose one.
- A namespace is a compile-time name, not a value you can store or pass. Different namespaces can expose functions with the same name; their requirements and borrowing facts stay distinct.
- Session namespace aliases can be replaced; earlier functions and saved function values retain their selected declarations.
- Modules may import each other when every file in the cycle declares only imports, functions, classes, traits, enums and aliases, and every function, method and constructor types all its parameters and results. Typed defaults on functions, methods and constructors work in the cycle. Call-free runtime `requires` conditions work too, retaining guards at header calls. Typed constants work in the cycle. Calls inside conditions, theorems and top-level statements report E_IMPORT_CYCLE_UNSUPPORTED. Inside the cycle, a call's result may borrow any of its class-typed arguments and `this`, since the bodies are not known. Proofs cannot unfold functions from these modules.

## Defined proof arithmetic

```kestrel
function discarded(n : Int) : Int {
    let unused := n + 1
    0
}
theorem bounded(n : Int) : discarded(n) = 0 requires room : n < 9223372036854775807 { rfl }
function certified(n : Int) : Int requires bounded(n) { n }
function guarded(n : Int) : Int or Error requires n < 9223372036854775807 { certified(n) }

print(match guarded(5) { n => n.text(), _ => "blocked", })
function boundary() : Int = 9223372036854775807
print(match guarded(boundary()) { n => n.text(), _ => "blocked", })
```

- The proof checks the unused addition as well as the result. Inside the theorem, arithmetic is over the integers, so `n + 1` cannot fail there and the proof would check without `room`.
- `room` matters for `certified`. A function that requires a theorem hands its facts to run-time code, where Int has 64 bits, so its callers must establish the theorem's assumptions; `guarded` does so with its own `requires`.
- Earlier checked equalities can establish the safety of a function call, including in an induction step. Explicit proofs use exact integer bounds without a solver. Harder bounds can need a smaller lemma, case splitting or smt.
- The theorem requirement is erased, while the ordinary guard still returns Error on the excluded input.

## Postconditions

```kestrel
function nonnegative(n : Int) : Int ensures result >= 0 {
    if n < 0 {return 0}
    n
}

print(nonnegative(-3))
print(nonnegative(4))
```

- `result` names the successful return value. Each ensures condition must be pure and true; a failed promise stops the program with its source location.
- Error returns are exempt, including errors from closing files or joining tasks. A void function may use a condition such as ensures true.
- Conditions may read result, copyable parameters and read-only this. They cannot mutate or move result, read a noncopyable parameter, propagate Error or return early. Parameters cannot be named result when ensures is present.
- Postconditions belong to concrete function and method bodies. They do not transfer to overrides; constructors and abstract declarations cannot use them. A pending check is work after the body call, so that call is not a tail call.

## Bounded proofs

```kestrel
function identity[T : Equals](x : T) : T = x
theorem same[T : Equals](x : T) : identity(x) = x {rfl}
theorem natural(n : Nat) : identity(n) = n {same(n)}

function keep[U : Equals](x : U) : U requires same(x) = identity(x)
let n : Nat = 6
print(keep(n))
print(keep("bounded"))
```

- Theorems use the same bounds as functions, including combined traits and associated types. Calls, citations and simplification establish each required trait; omitting a bound does not grant it. Imports and sessions retain these requirements.
- A bounded function unfolds when its body uses the kernel's supported pure forms. Calls to unknown trait methods remain opaque.
- The equality proved with = is mathematical equality. It does not promise what a user-written equals method returns, or make NaN compare equal to itself. Only Int, Nat and Bool equality facts discharge runtime == checks; unsupported assumptions stay proof obligations.

## Length matches

```kestrel
enum Vec[T](n : Nat) {
    Nil : Vec[T](0)
    Cons(x : T, xs : Vec[T](n)) : Vec[T](n + 1)
}
function rebuild(move xs : Vec[T](n + m), left : Vec[T](n), right : Vec[T](m)) : Vec[T](n + m) = match xs {
    Nil => Nil,
    Cons(x, rest) => Cons(x, rest),
}
function head(xs : Vec[T](n + 1)) : T = match xs {Cons(x, _) => x,}
let left : Vec[Int](1) = Cons(1, Nil)
let right : Vec[Int](1) = Cons(2, Nil)
let values : Vec[Int](2) = rebuild(Cons(4, Cons(5, Nil)), left, right)
print(head(values))
```

- A match arm retains the case's exact length equation. Cons learns n + m = k + 1 without choosing n or m; Nil fixes both at zero. Nested matches use the same facts until the arm ends.
- Fixed multiples work too. The checker proves these linear equalities using exact integer arithmetic and erases the indices before execution. Later arguments can determine the parts of an earlier sum; a total alone cannot choose a split.
- Products of length names and general length inequalities remain outside this subset.

## Borrowed subclass results

```kestrel
class Figure {
    new() {}
    public function area() : double = 0.0
}
class Square extends Figure {
    public final side : double
    new(side : double) {this.side=side}
    @override public function area() : double = this.side * this.side
}
function choose(a : Figure,b : Figure,flag : Bool) : Figure = if flag then a else b
let square := Square(2.0)
let figure := Figure()
print(choose(square,figure,true).area())
print(choose(square,figure,false).area())
```

- A borrowed result preserves the selected subclass and its overrides, including when several classes can reach that result. The caller keeps ownership of both inputs.
- The result supports field reads, nested selections, generic calls and explicit trait views. Exclusive local borrows retain their mutation permission; an ordinary returned borrow stays read-only.
- Owned places still require their exact class. The native backend rejects a view that would leave newly owned return storage behind, and stored superclass fields that need a different native representation.

## Default argument lifetimes

```kestrel
class Point {
    public final label : String
    new(label : String) {this.label=label}
}
function text(point : Point = Point("fresh" + " value")) : String = point.label
function own(move point : Point = Point("owned" + " value")) : Point = point
print(text())
print(own().label)

let seed:=1
function value(n : Int = seed) : Int = n
function caller(seed : Int) : Int = value()
print(caller(9))

function parsed(n : Int or Error = parseInt("bad")) : Int or Error = n?
print(match parsed() {RuntimeError.Parse(_) => 42, n => n, _ => 0,})
```

- An omitted default evaluates for that call using names bound at the function's declaration. A caller's local seed does not replace the script's seed. Sessions retain the selected declaration versions.
- A default passed to a borrowing parameter lives in the caller. Returning a borrow of that fresh value from its owning function fails with E_BORROW_TEMPORARY. Copying a String result or transferring an owned value through a move parameter works.
- Copyable collections retain their storage when a result borrows an item, including through move parameters. Defaults may read earlier scalar/String/C-number and copyable handle parameters, or copied collections when producing a plain value. Class, callable, mutable-container, borrowed-result and this dependencies still need native helper support. Recursive default expansion reports E_DEFAULT_RECURSION.
- A default produces a value. Put return or ? in the function body; a fallible default can declare an Error union as shown above. A nested lambda keeps its own return.

## Unicode names

```kestrel
// Accents may be separate character codes; spelling stays exact.
function sommé(á : Int, b́ : Int = 2) : Int = á + b́
class ǅata[T] {
    public final valeuŕ : T
    new(move valeuŕ : T) {this.valeuŕ=valeuŕ}
}
enum État { Prêt
Attente }
function numéro(état : État) : Int = match état {Prêt => 1, Attente => 0,}
const LIMITÉ : Int = 3
let 数 := 4
let café := 5
let café := 6
let boîte := ǅata(sommé(á = 数))
print(boîte.valeuŕ)
print(numéro(État.Prêt) + LIMITÉ)
print(café + café)
```

- Names use the pinned Unicode 17.0 XID start and continuation sets, with `_` for patterns and constants. Combining marks may follow the initial character. Invisible default-ignorable characters fail lexing.
- A value name starts with a lowercase or uncased start character; a type or enum case starts with an uppercase or titlecase start character. Constants start uppercase and contain no lowercase letters. The existing underscore restrictions still apply.
- Names compare by exact character codes: café and café above are separate bindings. The compiler preserves both spellings; it does not normalize them. Module paths retain their lowercase rule.


## Dependent defaults

```kestrel
function area(width : Int, height : Int = width) : Int = width * height
function labels(move first : String, move second : String = first) : String = first + ":" + second
function show[T : Text](value : T, label : String = value.text()) : String = label
class Rectangle {
    public final width : Int
    public final height : Int
    new(width : Int, height : Int = width) {this.width=width
    this.height=height}
    public function scaled(k : Int, other : Int = k) : Int = this.width * other
}
print(area(3))
print(labels("a" + "b"))
print(show(4))
print(Rectangle(5).height)
print(Rectangle(5).scaled(2))
function apply(f : Int -> Int, value : Int) : Int = f(value)
print(apply(area(_),4))
const PREFIX : String = "global" + " default"
function saved(move value : String = PREFIX) : String = value
print(saved())
print(saved())
```

- Defaults read earlier copied parameter values as read-only values. Supplied expressions run once in source order, followed by defaults in parameter order.
- A partial call evaluates defaults when invoked. String storage stays retained across move parameters, constants and reusable captures.
- A reusable partial cannot consume a captured noncopyable object (E_PARTIAL_MOVE_CAPTURE), matching lambdas. Keep that input as `_` and provide an owned object per call, or use a borrowing parameter. Omitted owned defaults create fresh values per invocation.
- Classes, mutable containers, callbacks and live method receivers can supply plain defaults too. A borrowing default may also be an earlier parameter, a field path or call rooted in one, or `this`, as in `q : Point = p`, `q : Point = p.point`, `q : Point = p.find()` or `q : Point = find(p)`; the call lends that value (or a temporary that lives through the call) for its duration. Other borrowed/aggregate results from these dependencies and dependent mut borrows still report E_DEFAULT_DEPENDENCY.

## Mutable callback types

```kestrel
alias Counter = mut (() -> Int)
function counter(start : Int) : Counter {
    let mut n:=start
    move () => {n=n+1
    n}
}
function next(mut value : Counter) : Int=value()
function finish(move value : Counter) : Int=value()
class Ticker {
    public var step : Counter
    new(move step : Counter) {this.step=step}
    public mut function next() : Int=this.step()
}
let count:=counter(0)
print(next(count))
print(next(count))
let ticker:=Ticker(counter(10))
print(ticker.next())
print(ticker.next())
print(finish(counter(20)))
```

- mut (Int -> Int) marks a callback that may change its captured state. Such values move. Fields holding them must be var.
- mut before a parameter name grants a mutable borrow; move instead takes ownership. A read-only parameter cannot call a mutable callback. Effects independently marks outside work.
- Read-only callbacks can satisfy a mutable callback type. Ordinary collection item reads still lend read-only values; the type adds no permission to mutate through them.

## Collection counts

```kestrel
print([1,2,3].length())
print(Map("a" => 1,"b" => 2,"a" => 3).length())
print(Set(1,1,2).length())
print(SortedSet(3,1,1).length())
print("λ😀".length())
function count(values : List[Int], size : Int = values.length()) : Int=size
print(count([4,5]))
function first(values : List[String], label : String = match values {[head,..]=>head,_=>""}) : String=label
print(first(["kept"+" text"]))
```

- length() reads the stored count of a List, Map, Set, SortedSet, MutMap or MutSet. String.length() counts Unicode scalars. These reads are pure and need no mut permission.
- Defaults can read earlier parameters and a method receiver when producing a scalar, String, C number or copyable pointer/arena handle. Classes, mutable containers and read-only callbacks can supply these values. Supplied arguments still run before defaults.
- A borrowing default may be an earlier parameter, a field path or call rooted in one, or `this`; the call lends that value for its duration. Other borrowed or aggregate results from these dependencies and dependent mut borrows still report E_DEFAULT_DEPENDENCY until their ownership summaries and native helpers are implemented.

## Function input permissions

```kestrel
class Counter {
    var n : Int
    new() {this.n=0}
    public mut function increment() : Int {this.n=this.n+1
    this.n}
}
class Payload {
    public final label : String
    new(label : String) {this.label=label}
}
alias Update = (mut value : Counter) -> Int
alias Consume = (move value : Payload) -> String
alias Combine = (left : Int, right : Int) -> Int
function increment(mut target : Counter) : Int=target.increment()
function takePayload(move item : Payload) : String=item.label
function add(a : Int,b : Int) : Int=a+b
function run(update : Update,mut counter : Counter) : Int=update(value=counter)
function own(consume : Consume,move payload : Payload) : String=consume(value=payload)
let counter:=Counter()
let update : Update=increment
print(update(value=counter))
print(run(update,counter))
let consume : Consume=takePayload
print(own(consume,Payload("owned"+" value")))
let combine : Combine=add
print(combine(right=3,left=2))
```

- Arrow types can name their inputs and declare mut or move on each input. Unnamed inputs retain ordinary borrowing.
- These names provide labels for calls through the written type. The implementation's own parameter names can differ.
- Argument permissions remain separate from captured-state mutation and Effects. Lambdas use the same explicit parameter modifiers as named functions.

## Receiver and callback defaults

```kestrel
trait Measured {
    function width() : Int
    function area(height : Int = this.width()) : Int
}
class Square {
    has Measured
    public final side : Int
    new(side : Int) {this.side=side}
    public function width() : Int=this.side
    public function area(height : Int = this.side) : Int=this.side*height
}
function measure(shape : &dyn Measured) : Int=shape.area()
function count(shapes : List[Square],n : Int = shapes.length()) : Int=n
function value(read : () -> Int,n : Int = read()) : Int=n
let square:=Square(3)
print(square.area())
let area:=square.area(_)
print(area(2))
print(measure(square))
print(count([Square(1),Square(2)]))
print(value(square.width))
```

- A method default reads this at invocation. Saved methods and partial calls retain their receiver and defer defaults until called.
- Trait declarations check their defaults too. A call through a trait view uses that trait declaration's defaults; a concrete call uses the concrete method's defaults.
- Earlier parameters and this are read-only within a default. Outside work still needs Effects on the declaration. Each default runs once for each omitted argument.

## Construction order

```kestrel
function inspect(value : Label) : String=value.read()
class Label {
    public final size : Int
    public final label : String
    new(size : Int) {
        this.size=size
        this.label=((this)).size.text()
        let read:=this.read
        let checked:=read()
        let passed:=inspect(this)
    }
    public function read() : String=this.label
}
print(Label(7).read())
```

- Initialize every field before passing this, calling or extracting its methods, or capturing it in a lambda.
- Direct field reads may use earlier initialized fields. Parentheses preserve this rule. E_THIS_UNINITIALIZED reports early object exposure; an unfinished field read reports E_FIELD_UNINITIALIZED.

## Arbitrary precision integers

```kestrel
import std.math.{max}
let huge : BigInt=1_000_000_000_000_000_000_000+1
print(huge)
print(huge/3)
print(max(2,huge))
print(match huge.checkedToInt() {RuntimeError.Arithmetic(_)=>"outside Int",_=>"wrong"})
print(match parseBigInt("-12345678901234567890") {value=>value.text(),_=>"invalid"})
let values : SortedSet[BigInt]=SortedSet(huge,2,-3,2)
for (value in values) {print(value)}
```

- Integer literals take the BigInt type from annotations, numeric operands and unambiguous call parameters. An Int value widens to BigInt where a BigInt is expected, so no conversion is written. `toBigInt()` spells the same conversion when a method form is wanted. BigInt does not narrow to Int. BigInt values move when transferred; ordinary parameters borrow them.
- BigInt arithmetic has no fixed overflow bound. Division truncates toward zero, and the remainder keeps the dividend's sign. Plain zero division and negative integral powers stop. checkedDivide exposes zero division as Error.
- checkedToInt checks the Int range. checkedToDouble rounds to the nearest finite double, ties to even, or returns Error. parseBigInt accepts whole decimal text with an optional sign; invalid text returns RuntimeError.Parse.

## Static fields

```kestrel
class Registry {
    public static final name : String = "kestrel" + " registry"
    public static final limits : List[Int] = [1, 2, 3]
    new() {}
}

print(Registry.name)
print(Registry.limits.length())
```

- A `static final` field belongs to the type and is read as `Type.name`. It initializes once, on first use, in a thread-safe way.
- Its type is a copyable immutable value: Int, double, Bool, String, List, Map, Set or SortedSet. The initializer is pure and may read constants and other static final fields, not `this` or mutable state.
- A static final field cannot be assigned. A `static var` is shared mutable state with a lock per access; see Static variables below.
- `static` also marks static methods, which have no `this`.

## Static variables

```kestrel
class Counter {
    public static var count : Int = 0
    new() {}
}

Counter.count = 2
Counter.count = Counter.count + 1
print(Counter.count)
```

- A `static var` is shared mutable state. Every read and write takes an internal lock; a read copies the value.
- Reading or assigning one needs `with Effects`, because another task can change it.
- `Counter.count = Counter.count + 1` is two accesses, so concurrent updates can interleave. Use `Atomic.fetchAdd` for an atomic counter, or a Mutex for a compound update.
- The value must be a copyable immutable type. A static var initializes before the program's top-level code; a session version keeps its value across entries, and a redefined class gets fresh storage.
- A static or constant initializer cannot read a static var. `static var` in a generic class reports E_STATIC_GENERIC.

## Increment and decrement

```kestrel
let mut total := 0
total++
total--
for (let mut n : Int = 0; n < 3; n++) {
    total = total + n
}
print(total)
```

- A postfix `++` or `--` is a statement that adds or subtracts one. The target must be a mutable name, field or Array slot.
- Use `k++` in a C-style `for` step, or `k--` to count down. For an arbitrary step, write `k = k + 4`.
- The postfix form is not an expression and has no value; `print(k++)` is refused. Prefix `++k` is refused.

## Ranges

```kestrel
for (i in range(5)) {
    print(i)
}
for (i in range(2, 6)) {
    print(i)
}
for (i in range(6, 2, -2)) {
    print(i)
}
```

- `range(stop)` covers 0 through stop-1. `range(start, stop)` starts at start; both bounds are exclusive at the stop.
- A third argument sets the step; a negative step counts down. A zero step produces no values.
- A range is a lazy iterable, not a List. It allocates no collection; use `toList()` when a concrete list is needed. It composes with map and filter like other lazy sequences.

## Comparing and printing enums and records

```kestrel
enum Color {
    Red
    Green
}
record Point(x : Int, y : Int)
enum Tree {
    Leaf
    Node(Tree, Int, Tree)
}

print(Red == Green)                      // false
print(Point(1, 2) == Point(1, 2))        // true
print(Node(Leaf, 1, Leaf))               // Node(Leaf, 1, Leaf)
print(Point(1, 2))                       // Point(x = 1, y = 2)
let seen := Set(Point(0, 0), Point(0, 0))
print(seen.length())                     // 1
```

- An enum compares, prints and hashes by its case and payloads. A record does the same with its fields. Nothing has to be written, as long as every payload or field type has Equals and Text.
- A record that writes its own `text()` or `equals()` keeps its own version. A class still writes `has Equals` and `equals` itself.
- Enums, records and tuples hash by their parts, so they work as fast Map and Set keys.

## Changing values stored in a MutMap

```kestrel
let counts : MutMap[String, Int] = MutMap()
for (word in ["a", "b", "a"]) {
    if !counts.update(word, n => n + 1) {
        counts.put(word, 1)
    }
}
print(counts.get("a"))                   // Some(2)
```

- `update(key, change)` returns whether the key was present. For a number, a String or a record, `change` returns the new value.
- For a class with `var` fields, `change` receives the stored object as a `mut` parameter and changes it in place: `accounts.update("ana", a => a.deposit(5))`. The lambda's parameter takes `mut` from the expected type, so it needs no marker.

## Functions inside functions

```kestrel
function total(prices : List[double], tax : double) : double {
    function withTax(p : double) : double = p * (1.0 + tax)
    let mut sum := 0.0
    for (p in prices) {
        sum = sum + withTax(p)
    }
    sum
}
print(total([10.0, 20.0], 0.5))          // 45.0
```

- A function declared inside another one may read the enclosing function's values, call itself and call other local functions. It can also be passed as a value, as in `xs.map(withTax)`.

## Induction over trees

```kestrel
enum Tree { Leaf, Node(Tree, Tree) }

function mirror(t : Tree) : Tree = match t {
    Leaf => Leaf,
    Node(a, b) => Node(mirror(b), mirror(a)),
}

theorem mirrorTwice(t : Tree) : mirror(mirror(t)) = t by {
    induction t {
        Leaf => { rfl }
        Node(a, b) iha ihb => { simp [iha, ihb] }
    }
}
```

The two names follow the recursive fields: `iha` proves the statement at `a`,
and `ihb` proves it at `b`. The explicit term form uses the same names before
`=>`. Each use establishes any assumptions at that smaller value. A recursive
field must have the same instantiated type as the induction variable and be
bound to a value name. Recursion inside an Option or List needs explicit
proofs.

## Trait default bodies

```kestrel
trait Named {
    public function name() : String = "anon"
}

class Guest {
    has Named
    new() {}
}

class Member {
    has Named
    new() {}
    @override public function name() : String = "Ada"
}

let guest := Guest()
print(guest.name())
let view : &dyn Named = guest
print(view.name())
print(Member().name())
```

A default may call the trait's other methods and methods inherited through its
`has` list. Each concrete implementation specializes the body, including its
generic and associated types. Dynamic views use the same implementation.
A diamond that inherits one original default shares it; distinct defaults for
the same signature require an explicit `@override` method in the class.
Default methods keep their declared visibility, so write `public` when a direct
call outside the class should be allowed. A missing-method quick fix omits
methods that already have defaults.

## Theorems about classes

```kestrel
record Point(x : Int, y : Int)

theorem pointX(x y : Int) : Point(x, y).x = x { rfl }

class Square {
    public final side : Int
    new(side : Int) { this.side = side }
    public function width() : Int = this.side
}

theorem squareWidth(s : Square) : s.width() = s.side { rfl }

function knownWidth(s : Square) : Int requires squareWidth(s) = s.width()
print(knownWidth(Square(3)))
```

No marker is needed. The kernel reads immutable fields, constructor assignments,
and pure method bodies, including readable parent classes and trait defaults
on a concrete class. Parameters keep each field tied to its receiver. The same
class theorems work with citations, `apply`, `simp`, function requirements and
explicit proof arguments. Proofs disappear before the program runs.

A mutable field, an unreadable constructor, an effectful method or a program
subclass that overrides a method used by the theorem blocks that proof. The
error names the blocking member or subclass. Fields keep their ordinary
visibility. Floating-point fields and generic trait laws remain outside this
proof fragment.

## Built-in list proofs

```kestrel
theorem appendEmpty(xs : List[T]) : xs + [] = xs {
    induction xs {
        [] => rfl,
        [x, ..rest] ih => congrArg(r => [x] + r, ih),
    }
}

theorem doubleNot(b : Bool) : !(!b) = b by {
    cases b { true => { rfl } false => { rfl } }
}

theorem optionCases(o : Option[T]) : o.isSome() or o.isNone() {
    match o { Some(x) => Left(decide), None => Right(decide), }
}
```

The induction hypothesis proves the goal for the tail. Both empty and
nonempty lists must be covered. Explicit recursive proofs can instead use
`match` and cite themselves on the tail. The kernel also reads list literals,
concatenation, length, emptiness, reversal and copyable first/last values.
`=` is proof equality; runtime `==` keeps its existing meaning.

These proofs add no runtime storage or list conversion. Native lists keep
their array-backed representation, and list-pattern tails keep their existing
copying cost. The fragment includes reversal proofs and length preservation for insertion
sort. It does not claim full sorting correctness. Indexed Vec proofs remain outside this fragment.

## Proof results and state evidence

A result type can name a proof claim. The function or method must establish
that claim before it returns. `proof condition` asks the compiler to establish
a pure condition from current facts, without a runtime test:

```kestrel
record Job(name : String)

class Jobs {
    private var items : Array[Job]
    new() { this.items = Array.empty() }
    public function size() : Int = this.items.length()

    public mut function add(job : Job) : proof this.size() > 0 {
        this.items.append(job)
        proof this.size() > 0
    }

    public mut function next(proof ready : this.size() > 0) : Job {
        let job := this.items[this.size() - 1]
        this.items.pop()
        job
    }
}

let jobs := Jobs()
let ready := jobs.add(Job("compile"))
print(jobs.next(ready).name)

jobs.add(Job("test"))
let known := proof jobs.size() > 0
print(jobs.next(known).name)
```

This prints `compile`, then `test`. Insertion returns evidence that the queue
holds work. The second insertion also lets the compiler establish a new named
proof without an `if` or `while`. For the kennel example, two vaccinations
similarly establish `let safe := proof rex.shotCount() >= 2`.

Removing a job invalidates evidence about that queue, including renamed copies
of the proof. Fresh evidence does not revive old witnesses. `: proof claim`
keeps the claim in a public signature; a bare `proof` return type would not tell
the caller what the value proves.

The compiler tracks straight-line field updates and array insertion. Branches
keep common state knowledge; opaque code can require a new condition check.
Proof-result producers currently require direct calls; trait proof results,
saved producer values and mixed data/proof result tuples remain unsupported.
Proof values erase, while the producer's ordinary work still runs. Runtime
`ensures` clauses keep their existing meaning.
