use crate::source::{SourceFile, Span};
use std::fmt::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxKind {
    File,
    Project,
    Import,
    ImportNames,
    Package,
    Dependency,
    Function,
    /// `opaque function ...` or `opaque type ...`: a proof-only declaration
    /// with no body and no run-time representation.
    Opaque,
    Constructor,
    Class,
    Trait,
    Enum,
    Variant,
    Alias,
    OperatorDeclaration,
    Const,
    Attribute,
    /// The contiguous `///` block that touches a declaration; its children are `DocLine`s.
    Doc,
    /// One line of a documentation block, without the `///` marker.
    DocLine,
    Modifier,
    Has,
    TypeParameters,
    TypeParameter,
    Bound,
    ParameterList,
    Parameter,
    ReturnType,
    Effects,
    Requires,
    Ensures,
    /// `invariant <expression>` in a class body.
    Invariant,
    TypeName,
    TypeApplication,
    IndexedType,
    TupleType,
    ShapeType,
    FunctionType,
    UnionType,
    BorrowType,
    MutableType,
    DynType,
    AssociatedType,
    AssociatedTypeDeclaration,
    Name,
    Literal,
    Hole,
    Group,
    Tuple,
    List,
    Comprehension,
    /// `{x : U | P}`: the set of values of `U` that satisfy `P`.
    SetBuilder,
    /// `{a, b}`, `{a}` or the empty set `∅`.
    SetLiteral,
    /// A later `for p in xs if g` clause of a comprehension, nested inside the earlier ones.
    ComprehensionFor,
    /// The type an `extend` block adds a function to, kept as the first child of that function.
    ExtendTarget,
    Call,
    Argument,
    NamedArgument,
    ArrowArgument,
    BracketApplication,
    /// `a[i]` on an Array (Q53).
    Index,
    Slice,
    Member,
    Record,
    RecordField,
    Unary,
    Binary,
    Pipe,
    Propagate,
    Lambda,
    LambdaParameters,
    Block,
    If,
    Match,
    MatchArm,
    Guard,
    Binding,
    /// `final this.radius : double := radius` inside a constructor.
    FieldDeclaration,
    Assignment,
    For,
    ForBinding,
    While,
    Return,
    Break,
    Continue,
    Using,
    Unsafe,
    Scope,
    Spawn,
    Otherwise,
    PatternName,
    PatternWildcard,
    PatternLiteral,
    PatternConstructor,
    /// One named payload of a case pattern: `height = h`.
    NamedPattern,
    PatternTuple,
    PatternList,
    PatternRest,
    /// `p | q` as a whole match arm pattern; the children are the alternatives.
    PatternAlt,
    /// `a..b`, a half-open integer range.
    PatternRange,
    /// `a..=b`, a closed integer range.
    PatternRangeInclusive,
    FormatString,
    FormatText,
    Interpolation,
    FormatSpec,
    Theorem,
    /// An unnamed theorem that is checked but cannot be cited.
    Example,
    /// A named statement accepted without a proof.
    Axiom,
    Proposition,
    /// `forall (n : Nat), P` or `forall n : Nat, P` in a statement.
    Forall,
    /// `exists (n : Nat), P` or `exists n : Nat, P` in a statement.
    Exists,
    /// The proof that a block's condition holds, bound by `if h : cond` or `while h : cond`.
    /// Its child is the condition. The parser makes it, and a program cannot write it (Q115).
    ProofWitness,
    /// The braces after a theorem header; its child is the proof block.
    Proof,
    /// `induction n { Zero => p, Succ(k) ih => q }` inside a proof.
    Induction,
    /// `simp` or `simp [rule, ...]` inside a proof.
    Simp,
    /// Optional trusted arithmetic solver inside a proof (Q72).
    Smt,
    /// The `by { ... }` block of a theorem, whose child is a TacticScript.
    TacticProof,
    /// A newline-separated sequence of tactics.
    TacticScript,
    /// One tactic invocation; its first child is the keyword.
    Tactic,
    /// A `{ ... }` block of tactics, used by repeat, try and proof branches.
    TacticBlock,
    /// One `pattern => { ... }` branch of a tactic-level cases or induction.
    TacticBranch,
    Operator,
    /// The synthetic literal `1` inside a desugared `++`/`--` statement.
    One,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub kind: SyntaxKind,
    pub span: Span,
    pub children: Vec<Node>,
}

impl Node {
    pub fn new(kind: SyntaxKind, span: Span, children: Vec<Node>) -> Self {
        Self {
            kind,
            span,
            children,
        }
    }

    pub fn leaf(kind: SyntaxKind, span: Span) -> Self {
        Self::new(kind, span, vec![])
    }

    pub fn child(&self, kind: SyntaxKind) -> Option<&Node> {
        self.children.iter().find(|child| child.kind == kind)
    }

    pub fn descendants(&self) -> impl Iterator<Item = &Node> {
        let mut pending = vec![self];
        std::iter::from_fn(move || {
            let node = pending.pop()?;
            pending.extend(node.children.iter().rev());
            Some(node)
        })
    }

    pub fn dump(&self, source: &SourceFile) -> String {
        let mut output = String::new();
        let mut pending = vec![(self, 0)];
        while let Some((node, depth)) = pending.pop() {
            let text = if node.children.is_empty() {
                format!(" {:?}", source.slice(node.span))
            } else {
                String::new()
            };
            writeln!(
                output,
                "{}{:?}@{}..{}{}",
                "  ".repeat(depth),
                node.kind,
                node.span.start,
                node.span.end,
                text
            )
            .unwrap();
            pending.extend(node.children.iter().rev().map(|child| (child, depth + 1)));
        }
        output
    }
}
