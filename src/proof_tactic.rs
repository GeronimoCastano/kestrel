//! A Lean-style tactic language over the Stage 7 proof kernel.
//!
//! A `by { ... }` block is checked by reducing proof goals. Every reduction is
//! justified by the same kernel that checks explicit terms, so the tactic layer
//! adds no trusted steps: it either closes a goal through `prove`, splits a goal
//! into strictly smaller goals, or introduces a local name.
use super::*;

#[derive(Clone)]
struct TacticGoal {
    ctx: Context,
    target: Prop,
    _span: Span,
}

/// One goal as the editor shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProofGoal {
    pub variables: Vec<(String, String)>,
    pub hypotheses: Vec<(String, String)>,
    pub target: String,
}

/// The proof state at the cursor, with an optional explanation when no tactic
/// proof is available there.
#[derive(Clone, Debug, Default)]
pub struct GoalReport {
    pub theorem: String,
    pub goals: Vec<ProofGoal>,
    /// Sibling goals created by a cases or induction branch. `goals` is the
    /// branch the cursor is in; `cases` lists every branch at that split.
    pub cases: Vec<CaseGoal>,
    pub error: Option<String>,
    pub complete: bool,
    pub admitted: bool,
    /// The cursor is inside a theorem or example header. This stays true when
    /// the body has an error, so the editor can keep the last goal state.
    pub in_theorem: bool,
}

/// One branch of a cases or induction split.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CaseGoal {
    pub label: String,
    pub goal: ProofGoal,
    pub line: usize,
    pub column: usize,
    pub active: bool,
}

#[derive(Clone)]
pub(crate) struct StepState {
    pub(crate) span: Span,
    pub(crate) before: Vec<ProofGoal>,
    pub(crate) after: Vec<ProofGoal>,
    /// The cases tactic that created this branch, and the branch label.
    pub(crate) parent: Option<Span>,
    pub(crate) label: String,
    pub(crate) offset: usize,
}

/// Pretty-print a proposition for the editor. The compiler still reports the
/// ASCII spelling; `∀`, `∃`, `→`, `∧`, `∨` and `¬` are display only.
pub(crate) fn pretty_prop(p: &Prop) -> String {
    pretty_at(p, 0)
}

/// Display a proposition, adding parentheses when a child binds more loosely
/// than its parent. The numbers are local to this printer; 0 is the loosest.
fn pretty_at(p: &Prop, outer: u8) -> String {
    let (text, prec) = match p {
        Prop::Eq(a, b) => (format!("{a} = {b}"), 5),
        Prop::Holds(t) => (t.to_string(), 5),
        Prop::Mem(a, b) => (format!("{a} ∈ {b}"), 5),
        Prop::Subset { left, right, .. } => (format!("{left} ⊆ {right}"), 5),
        Prop::And(a, b) => (format!("{} ∧ {}", pretty_at(a, 4), pretty_at(b, 4)), 3),
        Prop::Or(a, b) => (format!("{} ∨ {}", pretty_at(a, 3), pretty_at(b, 3)), 2),
        Prop::Not(inner) => match inner.as_ref() {
            Prop::Mem(a, b) => (format!("{a} ∉ {b}"), 5),
            p => (format!("¬{}", pretty_at(p, 4)), 4),
        },
        Prop::Implies(a, b) => (format!("{} → {}", pretty_at(a, 3), pretty_at(b, 1)), 1),
        Prop::Iff(a, b) => (format!("{} ↔ {}", pretty_at(a, 1), pretty_at(b, 1)), 0),
        Prop::Atom(term) => (term.to_string(), 5),
        Prop::Forall { name, ty, body } => (
            format!("∀ {} : {ty}, {}", shown(name), pretty_at(body, 1)),
            1,
        ),
        Prop::Exists { name, ty, body } => (
            format!("∃ {} : {ty}, {}", shown(name), pretty_at(body, 0)),
            0,
        ),
        Prop::True => ("True".into(), 5),
        Prop::False => ("False".into(), 5),
    };
    if prec < outer {
        format!("({text})")
    } else {
        text
    }
}

fn snapshot(goals: &[TacticGoal]) -> Vec<ProofGoal> {
    goals
        .iter()
        .map(|goal| {
            let mut variables = vec![];
            for (name, term, ty) in &goal.ctx.scope.terms {
                if matches!(term, Term::Var(_)) && *ty != Type::Invalid {
                    let entry = (name.clone(), ty.to_string());
                    if !variables.contains(&entry) {
                        variables.push(entry);
                    }
                }
            }
            ProofGoal {
                variables,
                hypotheses: goal
                    .ctx
                    .hyps
                    .iter()
                    .map(|(name, prop)| (name.clone(), pretty_prop(prop)))
                    .collect(),
                target: pretty_prop(&goal.target),
            }
        })
        .collect()
}

fn described(goals: &[TacticGoal]) -> String {
    match goals {
        [] => "all goals are closed".into(),
        [one] => format!("the remaining goal is `{}`", pretty_prop(&one.target)),
        many => format!("{} goals remain", many.len()),
    }
}

/// The node inside any parentheses a tactic argument carries, so `apply(h.left)`
/// classifies the same way as `exact(h.left)`.
fn ungroup(mut node: &Node) -> &Node {
    while node.kind == K::Group && node.children.len() == 1 {
        node = &node.children[0];
    }
    node
}

impl Kernel<'_> {
    fn tactic_goal(ctx: Context, target: Prop, span: Span) -> TacticGoal {
        TacticGoal {
            ctx,
            target,
            _span: span,
        }
    }

    fn tactic_subgoal(&self, ctx: &Context, target: Prop, span: Span) -> TacticGoal {
        TacticGoal {
            ctx: ctx.clone(),
            target,
            _span: span,
        }
    }

    /// Checks a theorem's `by { ... }` proof.
    pub(super) fn tactic_proof(
        &mut self,
        body: &Node,
        statement: &Prop,
        ctx: &Context,
        current: &mut Current,
    ) -> Found<()> {
        let Some(script) = body.child(K::TacticScript) else {
            return fail(
                body.span,
                "E_PROOF_SHAPE",
                "This tactic proof has no script.",
                "Write by { ... }.",
            );
        };
        let initial = Self::tactic_goal(ctx.clone(), statement.clone(), body.span);
        let mut record = vec![];
        self.run_script(script, initial, current, &mut record)
    }

    /// Runs one tactic script. The script must close every goal it creates.
    fn run_script(
        &mut self,
        script: &Node,
        initial: TacticGoal,
        current: &mut Current,
        record: &mut Vec<StepState>,
    ) -> Found<()> {
        let mut state = vec![initial];
        self.run_tactics_into(script, &mut state, current, Some(record))?;
        if state.is_empty() {
            Ok(())
        } else {
            fail(
                script.span,
                "E_PROOF_INCOMPLETE",
                format!("This proof is not finished: {}.", described(&state)),
                "Add a tactic that closes every goal, such as exact, rfl, assumption or simp.",
            )
        }
    }

    fn run_tactics_into(
        &mut self,
        script: &Node,
        state: &mut Vec<TacticGoal>,
        current: &mut Current,
        _record: Option<&mut Vec<StepState>>,
    ) -> Found<()> {
        for tactic in &script.children {
            if tactic.kind == K::TacticBlock {
                self.run_tactics_into(tactic, state, current, None)?;
                continue;
            }
            let before = snapshot(state);
            self.apply_tactic(tactic, state, current)?;
            let after = snapshot(state);
            if self.record_tactics {
                self.tactic_states.push(StepState {
                    span: tactic.span,
                    before,
                    after,
                    parent: None,
                    label: String::new(),
                    offset: tactic.span.start,
                });
            }
        }
        Ok(())
    }

    fn apply_tactic(
        &mut self,
        node: &Node,
        state: &mut Vec<TacticGoal>,
        current: &mut Current,
    ) -> Found<()> {
        let Some(keyword) = node.child(K::Name).map(|n| self.text(n).to_owned()) else {
            return fail(
                node.span,
                "E_TACTIC",
                "This tactic has no name.",
                "Start the line with a tactic such as intro, exact or simp.",
            );
        };
        if keyword == "done" {
            if state.is_empty() {
                return Ok(());
            }
            return fail(
                node.span,
                "E_PROOF_INCOMPLETE",
                format!("done found an unfinished goal: {}.", described(state)),
                "Close every goal before done.",
            );
        }
        if keyword == "skip" {
            return Ok(());
        }
        if keyword == "sorry" {
            current.admitted = true;
            self.warnings.push(Diagnostic::warning(
                "W_PROOF_SORRY",
                "This proof uses `sorry`; the theorem is not trusted for function requirements.",
                node.span,
            ));
            state.remove(0);
            return Ok(());
        }
        if keyword == "repeat" || keyword == "try" {
            let block = node.children.get(1).ok_or_else(|| {
                Diagnostic::new("E_TACTIC", "This tactic needs a block.", node.span)
            })?;
            self.tactic_block(block, state, current, node.span, keyword == "try")?;
            return Ok(());
        }
        let Some(goal) = state.first().cloned() else {
            return fail(
                node.span,
                "E_PROOF_INCOMPLETE",
                format!("There is no goal left for {keyword}."),
                "This tactic comes after every goal was closed.",
            );
        };
        let args = &node.children[1..];
        if std::env::var("KESTREL_TACTICS").is_ok() {
            eprintln!(
                "tactic {keyword}: {} goal(s): {}",
                state.len(),
                state
                    .first()
                    .map(|g| pretty_prop(&g.target))
                    .unwrap_or_default()
            );
        }
        let produced = match keyword.as_str() {
            "intro" | "intros" => {
                self.tactic_intro(node, args, goal, current, keyword == "intros")?
            }
            "exact" => {
                let Some(term) = args.first() else {
                    return self.tactic_argument(node, "exact needs a proof term.");
                };
                self.prove(term, &goal.target, &goal.ctx, current)?;
                vec![]
            }
            "assumption" => {
                let found = goal
                    .ctx
                    .hyps
                    .iter()
                    .rev()
                    .any(|(_, prop)| self.same(prop, &goal.target));
                if !found {
                    return fail(
                        node.span,
                        "E_PROOF_ASSUMPTION",
                        format!("No assumption proves `{}`.", pretty_prop(&goal.target)),
                        "Add a hypothesis, or use exact with a term.",
                    );
                }
                vec![]
            }
            "rfl" => {
                self.close_rfl(node, &goal)?;
                vec![]
            }
            "decide" => {
                match self.decide(&goal.target) {
                    Some(true) => self.defined_prop(&goal.target, &goal.ctx, node.span)?,
                    Some(false) => {
                        return fail(
                            node.span,
                            "E_PROOF_FALSE",
                            format!(
                                "decide computed `{}` and found it false.",
                                pretty_prop(&goal.target)
                            ),
                            "Check the statement.",
                        );
                    }
                    None => {
                        return fail(
                            node.span,
                            "E_PROOF_SHAPE",
                            "decide proves a statement with no variables.",
                            "Split the variables first, or use another tactic.",
                        );
                    }
                }
                vec![]
            }
            "simp" => {
                let simp = Node::new(K::Simp, node.span, node.children[1..].to_vec());
                if self.closes_simple(&goal.target) {
                    self.defined_prop(&goal.target, &goal.ctx, node.span)?;
                } else {
                    // simplify rewrites, unfolds the outer set membership and
                    // checks the structural closures that remain.
                    self.simplify(&simp, &goal.target, &goal.ctx, current)?;
                }
                vec![]
            }
            "smt" => {
                let smt = Node::new(K::Smt, node.span, vec![]);
                self.solve_arithmetic(&smt, &goal.target, &goal.ctx)?;
                vec![]
            }
            "constructor" => self.tactic_constructor(node, &goal)?,
            "ext" => self.tactic_ext(node, &goal)?,
            "pushNeg" => {
                let mut goal = goal;
                // A positive subset goal stays written; a membership is
                // unfolded so its negations can move.
                let target = if matches!(goal.target, Prop::Mem(..)) {
                    self.unfold_head(&goal.target)
                } else {
                    goal.target.clone()
                };
                goal.target = self.push_neg(target);
                vec![goal]
            }
            "byContra" => {
                let Some(name) = args.first() else {
                    return self
                        .tactic_argument(node, "byContra needs a name for the negated goal.");
                };
                let name = self.text(name).to_owned();
                let mut goal = goal;
                goal.ctx
                    .hyps
                    .push((name, Prop::Not(Box::new(goal.target.clone()))));
                goal.target = Prop::False;
                vec![goal]
            }
            "left" | "right" => self.tactic_left_right(node, &goal, keyword == "left")?,
            "symm" => {
                let Prop::Eq(a, b) = &goal.target else {
                    return fail(
                        node.span,
                        "E_PROOF_SHAPE",
                        format!(
                            "symm proves an equality, but the goal is `{}`.",
                            pretty_prop(&goal.target)
                        ),
                        "Use symm for a goal written with `=`.",
                    );
                };
                let mut swapped = goal.clone();
                swapped.target = Prop::Eq(b.clone(), a.clone());
                vec![swapped]
            }
            "trans" => {
                let Some(middle) = args.first() else {
                    return self.tactic_argument(node, "trans needs a middle term.");
                };
                let Prop::Eq(a, b) = &goal.target else {
                    return fail(
                        node.span,
                        "E_PROOF_SHAPE",
                        format!(
                            "trans proves an equality, but the goal is `{}`.",
                            pretty_prop(&goal.target)
                        ),
                        "Use trans for a goal written with `=`.",
                    );
                };
                let (mid, ty) = self.term(middle, None, &goal.ctx.scope)?;
                let left_ty = self.term_type(a, &goal.ctx);
                if !compatible(&left_ty, &ty) {
                    return fail(
                        middle.span,
                        "E_PROOF_TYPE",
                        format!("This middle term is {ty}, but the goal compares {left_ty}."),
                        "Use a term of the goal's type.",
                    );
                }
                vec![
                    self.tactic_subgoal(&goal.ctx, Prop::Eq(a.clone(), mid.clone()), node.span),
                    self.tactic_subgoal(&goal.ctx, Prop::Eq(mid, b.clone()), node.span),
                ]
            }
            "exfalso" => {
                vec![TacticGoal {
                    ctx: goal.ctx.clone(),
                    target: Prop::False,
                    _span: node.span,
                }]
            }
            "contradiction" => {
                self.close_contradiction(node, &goal)?;
                vec![]
            }
            "trivial" => {
                if self.close_trivial(&goal).is_ok() {
                    vec![]
                } else {
                    return fail(
                        node.span,
                        "E_PROOF_MISMATCH",
                        format!("trivial could not close `{}`.", pretty_prop(&goal.target)),
                        "Use exact, rfl, simp or assumption.",
                    );
                }
            }
            "exists" | "use" => {
                let Some(witness) = args.first() else {
                    return self.tactic_argument(node, "exists needs a witness value.");
                };
                self.tactic_exists(node, &goal, witness)?
            }
            "byCases" => self.tactic_by_cases(node, args, &goal)?,
            "calc" => self.tactic_calc(node, args, &goal, current)?,
            "cases" => self.tactic_cases(node, args, goal, current, false, false, &[])?,
            "induction" => {
                let generalized = args
                    .get(1)
                    .filter(|arg| arg.kind == K::Group)
                    .map(|group| group.children.as_slice())
                    .unwrap_or(&[]);
                self.tactic_cases(node, args, goal, current, true, false, generalized)?
            }
            "strongInduction" => self.tactic_cases(node, args, goal, current, true, true, &[])?,
            "have" => vec![self.tactic_have(node, args, goal, current)?],
            "apply" => self.tactic_apply(node, args, &goal, current)?,
            "rw" | "rewrite" => {
                let rule_node = Node::new(K::Simp, node.span, node.children[1..].to_vec());
                let rewritten = self.rewrite(&rule_node, &goal.target, &goal.ctx, current)?;
                let mut rewritten_goal = goal;
                rewritten_goal.target = rewritten;
                vec![rewritten_goal]
            }
            _ => {
                return fail(
                    node.span,
                    "E_TACTIC_UNKNOWN",
                    format!("`{keyword}` is not a known tactic."),
                    "Use intro, exact, apply, cases, induction, simp, rw, constructor, left, right, exists, have, assumption, contradiction, exfalso, symm, trans, rfl, decide, trivial, smt, pushNeg, ext, byContra, skip, done, sorry, repeat or try.",
                );
            }
        };
        state.remove(0);
        for (index, goal) in produced.into_iter().enumerate() {
            state.insert(index, goal);
        }
        Ok(())
    }

    fn tactic_argument<T>(&self, node: &Node, message: &str) -> Found<T> {
        fail(
            node.span,
            "E_TACTIC_ARGUMENT",
            message,
            "Give this tactic the value it needs on the same line.",
        )
    }

    fn tactic_intro(
        &mut self,
        node: &Node,
        names: &[Node],
        goal: TacticGoal,
        current: &mut Current,
        all: bool,
    ) -> Found<Vec<TacticGoal>> {
        let mut goal = goal;
        let mut given = names.iter();
        loop {
            let target = self.unfold_head(&goal.target);
            let name = if all {
                match &target {
                    Prop::Forall { name, .. } => name.clone(),
                    _ => self.fresh("h"),
                }
            } else {
                let Some(node) = given.next() else {
                    return self.tactic_argument(node, "intro needs a name for each binder.");
                };
                self.text(node).to_owned()
            };
            match target {
                Prop::Forall {
                    name: bound,
                    ty,
                    body,
                } => {
                    let mut map = BTreeMap::new();
                    map.insert(bound, Term::Var(name.clone()));
                    let mut fresh = self.fresh;
                    let target = body.subst_prop(&map, &mut fresh);
                    self.fresh = fresh;
                    goal.ctx.scope = goal.ctx.scope.with(&name, Term::Var(name.clone()), ty);
                    goal.target = target;
                }
                Prop::Implies(premise, conclusion) => {
                    goal.ctx.hyps.push((name, *premise));
                    goal.target = *conclusion;
                }
                Prop::Not(inner) => {
                    goal.ctx.hyps.push((name, *inner));
                    goal.target = Prop::False;
                }
                other => {
                    return fail(
                        node.span,
                        "E_PROOF_SHAPE",
                        format!(
                            "There is nothing to introduce: the goal is `{}`.",
                            pretty_prop(&other)
                        ),
                        "Use intro only for a forall, implication or not goal.",
                    );
                }
            }
            if !all {
                if given.clone().next().is_none() {
                    break;
                }
            } else if !matches!(
                self.unfold_head(&goal.target),
                Prop::Forall { .. } | Prop::Implies(..) | Prop::Not(..)
            ) {
                break;
            }
        }
        let _ = current;
        Ok(vec![goal])
    }

    fn close_rfl(&mut self, node: &Node, goal: &TacticGoal) -> Found<()> {
        // A membership goal unfolds one step, so `rfl` proves `A ∈ {A}` and
        // `A ∈ {x : U | x = A}` as the equalities they are.
        let target = if matches!(goal.target, Prop::Mem(..)) {
            self.unfold_head(&goal.target)
        } else {
            goal.target.clone()
        };
        let target = &target;
        if matches!(
            self.normal(target),
            Prop::Holds(Term::Bool(true)) | Prop::True
        ) {
            return self.defined_prop(&goal.target, &goal.ctx, node.span);
        }
        if let Prop::Iff(a, b) = target {
            if self.same(a, b) {
                return self.defined_prop(&goal.target, &goal.ctx, node.span);
            }
            let Prop::Iff(left, right) = self.normal(target) else {
                unreachable!()
            };
            return fail(
                node.span,
                "E_PROOF_MISMATCH",
                format!(
                    "rfl needs both sides of the iff to be definitionally equal, but they reduce to `{}` and `{}`.",
                    pretty_prop(&left),
                    pretty_prop(&right)
                ),
                "Unfold a definition with cases, or prove both directions with constructor.",
            );
        }
        if let Prop::Eq(a, b) = target {
            if self.same(
                &Prop::Eq(a.clone(), a.clone()),
                &Prop::Eq(a.clone(), b.clone()),
            ) {
                return self.defined_prop(&goal.target, &goal.ctx, node.span);
            }
            let Prop::Eq(left, right) = self.normal(target) else {
                unreachable!()
            };
            return fail(
                node.span,
                "E_PROOF_MISMATCH",
                format!(
                    "rfl needs both sides to reduce to the same term, but they reduce to `{left}` and `{right}`."
                ),
                "Split a variable with cases or induction so the functions can unfold.",
            );
        }
        fail(
            node.span,
            "E_PROOF_SHAPE",
            format!(
                "rfl proves an equality or an iff, but the goal is `{}`.",
                pretty_prop(&goal.target)
            ),
            "Use rfl for a goal written with `=` or `↔`.",
        )
    }

    fn close_trivial(&mut self, goal: &TacticGoal) -> Found<()> {
        if self.closes_simple(&goal.target) {
            self.defined_prop(&goal.target, &goal.ctx, Span::new(0, 0))?;
            return Ok(());
        }
        if goal
            .ctx
            .hyps
            .iter()
            .any(|(_, prop)| self.same(prop, &goal.target))
        {
            return Ok(());
        }
        fail(
            Span::new(0, 0),
            "E_PROOF_MISMATCH",
            "trivial found no immediate proof.",
            "Use exact, rfl, simp or assumption.",
        )
    }

    /// Propositions that need no assumptions or rule citations. Each case is a
    /// kernel rule, so closing by this test adds no trusted step.
    fn closes_simple(&mut self, prop: &Prop) -> bool {
        match prop {
            Prop::True => true,
            Prop::False => false,
            Prop::Eq(a, b) => self.same(
                &Prop::Eq(a.clone(), a.clone()),
                &Prop::Eq(a.clone(), b.clone()),
            ),
            Prop::Holds(Term::Bool(value)) => *value,
            Prop::Holds(_) | Prop::Mem(..) | Prop::Subset { .. } | Prop::Atom(_) => false,
            Prop::And(a, b) => self.closes_simple(a) && self.closes_simple(b),
            Prop::Or(a, b) => self.closes_simple(a) || self.closes_simple(b),
            Prop::Not(inner) => matches!(self.normal(inner), Prop::False),
            Prop::Implies(premise, conclusion) => {
                matches!(self.normal(premise), Prop::False) || self.closes_simple(conclusion)
            }
            Prop::Iff(a, b) => self.same(a, b),
            Prop::Forall { .. } | Prop::Exists { .. } => false,
        }
    }

    fn close_contradiction(&mut self, node: &Node, goal: &TacticGoal) -> Found<()> {
        if goal
            .ctx
            .hyps
            .iter()
            .any(|(_, prop)| matches!(self.normal(prop), Prop::False))
        {
            return Ok(());
        }
        for (_, prop) in &goal.ctx.hyps {
            match self.normal(prop) {
                Prop::Holds(Term::Bool(false)) | Prop::False => return Ok(()),
                Prop::Eq(a, b) if values_clash(&a, &b) => return Ok(()),
                // `not (x = x)` is false too.
                Prop::Not(inner) if matches!(inner.as_ref(), Prop::Eq(a, b) if a == b) => {
                    return Ok(());
                }
                _ => {}
            }
        }
        fail(
            node.span,
            "E_PROOF_SHAPE",
            "contradiction found no false assumption.",
            "Add a hypothesis that is already false, or prove the goal directly.",
        )
    }

    fn tactic_constructor(&mut self, node: &Node, goal: &TacticGoal) -> Found<Vec<TacticGoal>> {
        let target = self.unfold_head(&goal.target);
        match &target {
            Prop::And(a, b) => Ok(vec![
                self.tactic_subgoal(&goal.ctx, (**a).clone(), node.span),
                self.tactic_subgoal(&goal.ctx, (**b).clone(), node.span),
            ]),
            Prop::Iff(a, b) => Ok(vec![
                self.tactic_subgoal(&goal.ctx, Prop::Implies(a.clone(), b.clone()), node.span),
                self.tactic_subgoal(&goal.ctx, Prop::Implies(b.clone(), a.clone()), node.span),
            ]),
            Prop::True => Ok(vec![]),
            other => fail(
                node.span,
                "E_PROOF_SHAPE",
                format!(
                    "constructor builds an and, iff or True goal, but the goal is `{}`.",
                    pretty_prop(other)
                ),
                "Use constructor for P and Q, or prove the goal another way.",
            ),
        }
    }

    /// Move negations inward using the classical equivalences. `pushNeg` is a
    /// derived convenience over the same classical logic as `byContra`.
    fn push_neg(&mut self, prop: Prop) -> Prop {
        match prop {
            Prop::Not(inner) => {
                let inner = self.unfold_head(&inner);
                match inner {
                    Prop::Not(p) => self.push_neg(*p),
                    Prop::True => Prop::False,
                    Prop::False => Prop::True,
                    Prop::And(a, b) => Prop::Or(
                        Box::new(self.push_neg(Prop::Not(a))),
                        Box::new(self.push_neg(Prop::Not(b))),
                    ),
                    Prop::Or(a, b) => Prop::And(
                        Box::new(self.push_neg(Prop::Not(a))),
                        Box::new(self.push_neg(Prop::Not(b))),
                    ),
                    Prop::Implies(a, b) => Prop::And(
                        Box::new(self.push_neg(*a)),
                        Box::new(self.push_neg(Prop::Not(b))),
                    ),
                    Prop::Forall { name, ty, body } => Prop::Exists {
                        name,
                        ty,
                        body: Box::new(self.push_neg(Prop::Not(body))),
                    },
                    Prop::Exists { name, ty, body } => Prop::Forall {
                        name,
                        ty,
                        body: Box::new(self.push_neg(Prop::Not(body))),
                    },
                    Prop::Iff(a, b) => {
                        let left = (*a).clone();
                        let right = (*b).clone();
                        Prop::Or(
                            Box::new(Prop::And(
                                Box::new(self.push_neg(left.clone())),
                                Box::new(self.push_neg(Prop::Not(b))),
                            )),
                            Box::new(Prop::And(
                                Box::new(self.push_neg(Prop::Not(a))),
                                Box::new(self.push_neg(right)),
                            )),
                        )
                    }
                    other => Prop::Not(Box::new(other)),
                }
            }
            Prop::And(a, b) => Prop::And(Box::new(self.push_neg(*a)), Box::new(self.push_neg(*b))),
            Prop::Or(a, b) => Prop::Or(Box::new(self.push_neg(*a)), Box::new(self.push_neg(*b))),
            Prop::Implies(a, b) => {
                Prop::Implies(Box::new(self.push_neg(*a)), Box::new(self.push_neg(*b)))
            }
            Prop::Iff(a, b) => Prop::Iff(Box::new(self.push_neg(*a)), Box::new(self.push_neg(*b))),
            Prop::Forall { name, ty, body } => Prop::Forall {
                name,
                ty,
                body: Box::new(self.push_neg(*body)),
            },
            Prop::Exists { name, ty, body } => Prop::Exists {
                name,
                ty,
                body: Box::new(self.push_neg(*body)),
            },
            other => other,
        }
    }

    fn tactic_ext(&mut self, node: &Node, goal: &TacticGoal) -> Found<Vec<TacticGoal>> {
        let Prop::Eq(left, right) = &goal.target else {
            return fail(
                node.span,
                "E_PROOF_SHAPE",
                format!(
                    "ext proves an equality between sets, but the goal is `{}`.",
                    pretty_prop(&goal.target)
                ),
                "Use ext for a goal written `A = B`.",
            );
        };
        let ty = self.term_type(left, &goal.ctx);
        let Some(element) = set_element_type(&ty) else {
            return fail(
                node.span,
                "E_PROOF_SHAPE",
                format!("ext needs sets, but this equality compares {ty}."),
                "Use ext only for set equality.",
            );
        };
        let forward = self.subset_prop(left.clone(), right.clone(), element.clone());
        let backward = self.subset_prop(right.clone(), left.clone(), element);
        Ok(vec![
            self.tactic_subgoal(&goal.ctx, forward, node.span),
            self.tactic_subgoal(&goal.ctx, backward, node.span),
        ])
    }

    fn tactic_left_right(
        &mut self,
        node: &Node,
        goal: &TacticGoal,
        left: bool,
    ) -> Found<Vec<TacticGoal>> {
        let target_prop = self.unfold_head(&goal.target);
        let Prop::Or(a, b) = &target_prop else {
            return fail(
                node.span,
                "E_PROOF_SHAPE",
                format!(
                    "left and right prove an or goal, but the goal is `{}`.",
                    pretty_prop(&goal.target)
                ),
                "Use left or right only for a goal written with or.",
            );
        };
        let target = if left {
            a.as_ref().clone()
        } else {
            b.as_ref().clone()
        };
        Ok(vec![self.tactic_subgoal(&goal.ctx, target, node.span)])
    }

    fn tactic_exists(
        &mut self,
        node: &Node,
        goal: &TacticGoal,
        witness: &Node,
    ) -> Found<Vec<TacticGoal>> {
        let target_prop = self.unfold_head(&goal.target);
        let Prop::Exists { name, ty, body } = &target_prop else {
            return fail(
                node.span,
                "E_PROOF_SHAPE",
                format!(
                    "exists proves an existential goal, but the goal is `{}`.",
                    pretty_prop(&goal.target)
                ),
                "Use exists only for a goal written with exists.",
            );
        };
        let (term, found) = self.term(witness, Some(ty), &goal.ctx.scope)?;
        if !compatible(ty, &found) {
            return fail(
                witness.span,
                "E_PROOF_TYPE",
                format!("This witness is {found}, but the goal needs {ty}."),
                "Give a value of the quantified type.",
            );
        }
        let mut map = BTreeMap::new();
        map.insert(name.clone(), term);
        let mut fresh = self.fresh;
        let body = body.subst_prop(&map, &mut fresh);
        self.fresh = fresh;
        Ok(vec![self.tactic_subgoal(&goal.ctx, body, node.span)])
    }

    fn tactic_have(
        &mut self,
        node: &Node,
        args: &[Node],
        mut goal: TacticGoal,
        current: &mut Current,
    ) -> Found<TacticGoal> {
        let Some(name) = args.first() else {
            return self.tactic_argument(node, "have needs a name.");
        };
        let name = self.text(name).to_owned();
        // `have h := term` infers the statement from the proof term.
        if args.len() == 2 {
            let proof = &args[1];
            if proof.kind == K::TacticProof {
                return fail(
                    proof.span,
                    "E_TACTIC_ARGUMENT",
                    "An unannotated have cannot infer a goal from a tactic script.",
                    "Write have h : P by { ... }, or use a proof term after :=.",
                );
            }
            let claim_prop = self.infer(proof, &goal.ctx, current)?;
            goal.ctx.hyps.push((name, claim_prop));
            return Ok(goal);
        }
        let Some(claim) = args.get(1) else {
            return self.tactic_argument(node, "have needs a statement.");
        };
        let claim_node = claim.children.first().unwrap_or(claim);
        let claim_prop = self.prop(claim_node, &goal.ctx.scope)?;
        let proof = args.get(2).ok_or_else(|| {
            Diagnostic::new(
                "E_TACTIC_ARGUMENT",
                "have needs a proof after := or by.",
                node.span,
            )
            .with_hint("Write have h := term, have h : P := term, or have h : P by { ... }.")
        })?;
        match proof.kind {
            K::TacticProof => {
                let script = proof.child(K::TacticScript).ok_or_else(|| {
                    Diagnostic::new("E_TACTIC", "This proof has no tactic script.", proof.span)
                })?;
                let initial = Self::tactic_goal(goal.ctx.clone(), claim_prop.clone(), proof.span);
                let mut ignored = vec![];
                self.run_script(script, initial, current, &mut ignored)?;
            }
            _ => self.prove(proof, &claim_prop, &goal.ctx, current)?,
        }
        goal.ctx.hyps.push((name, claim_prop));
        Ok(goal)
    }

    /// `calc a = b := h1` followed by `_ = c := h2` chains transitivity.
    /// Every link is checked against its own equation, and the first and last
    /// terms are checked against the goal, so closing the goal is sound.
    fn tactic_calc(
        &mut self,
        node: &Node,
        args: &[Node],
        goal: &TacticGoal,
        current: &mut Current,
    ) -> Found<Vec<TacticGoal>> {
        let Some(lines) = args.first().filter(|arg| arg.kind == K::Group) else {
            return self.tactic_argument(node, "calc needs at least one line.");
        };
        let Prop::Eq(goal_left, goal_right) = &goal.target else {
            return fail(
                node.span,
                "E_PROOF_SHAPE",
                format!(
                    "calc proves an equality, but the goal is `{}`.",
                    pretty_prop(&goal.target)
                ),
                "Use calc only for an equality, or chain implications another way.",
            );
        };
        let mut previous: Option<Term> = None;
        let mut first: Option<Term> = None;
        for line in &lines.children {
            let (left, right, proof) = match line.children.as_slice() {
                [left, right, proof] => (left, right, proof),
                _ => {
                    return self.tactic_argument(line, "A calc line needs two terms and a proof.");
                }
            };
            let left_term = if left.kind == K::Hole {
                let Some(previous) = &previous else {
                    return fail(
                        left.span,
                        "E_PROOF_SHAPE",
                        "The first calc line needs its left side.",
                        "Write calc a = b := h1, then _ = c := h2.",
                    );
                };
                previous.clone()
            } else {
                self.term(left, None, &goal.ctx.scope)?.0
            };
            let left_ty = self.term_type(&left_term, &goal.ctx);
            let right_term = self.term(right, Some(&left_ty), &goal.ctx.scope)?.0;
            if let Some(previous) = &previous
                && !self.terms_same(previous, &left_term)
            {
                return fail(
                    line.span,
                    "E_PROOF_SHAPE",
                    "This calc line does not start where the previous one ended.",
                    "Use _ on every line after the first, or repeat the previous right side.",
                );
            }
            let claim = Prop::Eq(left_term.clone(), right_term.clone());
            self.prove(proof, &claim, &goal.ctx, current)?;
            first.get_or_insert(left_term);
            previous = Some(right_term);
        }
        let (Some(first), Some(last)) = (first, previous) else {
            return self.tactic_argument(node, "calc needs at least one line.");
        };
        if !self.terms_same(&first, goal_left) {
            return fail(
                node.span,
                "E_PROOF_SHAPE",
                "The calc chain does not start at the goal's left side.",
                "Start calc with the left side of the goal.",
            );
        }
        if !self.terms_same(&last, goal_right) {
            return fail(
                node.span,
                "E_PROOF_SHAPE",
                "The calc chain does not end at the goal's right side.",
                "End calc with the right side of the goal.",
            );
        }
        Ok(vec![])
    }

    fn terms_same(&mut self, a: &Term, b: &Term) -> bool {
        let pair = Prop::Eq(a.clone(), b.clone());
        let same = Prop::Eq(a.clone(), a.clone());
        self.same(&pair, &same)
    }

    #[allow(clippy::too_many_arguments)]
    fn tactic_cases(
        &mut self,
        node: &Node,
        args: &[Node],
        mut goal: TacticGoal,
        current: &mut Current,
        induction: bool,
        strong: bool,
        generalized: &[Node],
    ) -> Found<Vec<TacticGoal>> {
        let Some(scrutinee) = args.first() else {
            return self.tactic_argument(node, "cases and induction need a variable.");
        };
        if scrutinee.kind == K::Member {
            if induction {
                return fail(
                    scrutinee.span,
                    "E_PROOF_SHAPE",
                    "induction needs a variable, not a projection.",
                    "Bind the projection with have, or use cases.",
                );
            }
            let mut members = vec![];
            let mut cursor = scrutinee;
            while cursor.kind == K::Member {
                members.push(self.text(&cursor.children[1]).to_owned());
                cursor = &cursor.children[0];
            }
            if cursor.kind != K::Name {
                return fail(
                    scrutinee.span,
                    "E_PROOF_SHAPE",
                    "cases can project a hypothesis by name.",
                    "Write cases h.left or cases h.right.",
                );
            }
            let root = self.text(cursor).to_owned();
            let Some((_, mut prop)) = goal
                .ctx
                .hyps
                .iter()
                .rev()
                .find(|(n, _)| *n == root)
                .cloned()
            else {
                return fail(
                    scrutinee.span,
                    "E_PROOF_UNKNOWN",
                    format!("`{root}` is not a hypothesis."),
                    "Project a hypothesis introduced earlier in the proof.",
                );
            };
            members.reverse();
            for member in &members {
                prop = self.normal(&prop);
                prop = match (member.as_str(), prop) {
                    ("left" | "l", Prop::And(a, _)) => *a,
                    ("right" | "r", Prop::And(_, b)) => *b,
                    ("left" | "l", Prop::Iff(a, b)) => Prop::Implies(a, b),
                    ("right" | "r", Prop::Iff(a, b)) => Prop::Implies(b, a),
                    (_, other) => {
                        return fail(
                            scrutinee.span,
                            "E_PROOF_SHAPE",
                            format!(
                                "cases cannot project `{member}` from `{}`.",
                                pretty_prop(&other)
                            ),
                            "Project a conjunction with left or right.",
                        );
                    }
                };
            }
            return self.cases_hypothesis(node, &root, prop, &goal, false);
        }
        let name = self.text(scrutinee).to_owned();
        if !induction
            && let Some((_, prop)) = goal
                .ctx
                .hyps
                .iter()
                .rev()
                .find(|(n, _)| *n == name)
                .cloned()
        {
            return self.cases_hypothesis(node, &name, prop, &goal, true);
        }
        let Some((_, Term::Var(var), ty)) = goal.ctx.scope.get(&name).cloned() else {
            return fail(
                scrutinee.span,
                "E_PROOF_SHAPE",
                format!("`{name}` is not a parameter the proof can split."),
                "Split a theorem parameter, a quantified variable or a local name.",
            );
        };
        let Type::Named(_, _) = &ty else {
            return fail(
                scrutinee.span,
                "E_PROOF_SHAPE",
                format!("`{name}` is {ty}, which has no cases."),
                "Split a Nat or an enum.",
            );
        };
        if strong {
            if !matches!(&ty, Type::Named(id, args) if id == "Nat" && args.is_empty()) {
                return fail(
                    scrutinee.span,
                    "E_PROOF_SHAPE",
                    format!("strong induction needs a Nat, but `{name}` is {ty}."),
                    "Use strongInduction on a Nat parameter, or induction for another type.",
                );
            }
            if !generalized.is_empty() {
                return fail(
                    node.span,
                    "E_PROOF_TACTIC",
                    "strongInduction does not take generalizing.",
                    "Revert the other variables with a quantified statement instead.",
                );
            }
        }
        let Some(variants) = self.value_cases(&ty) else {
            return fail(
                scrutinee.span,
                "E_PROOF_SHAPE",
                format!("`{name}` is {ty}, which has no cases."),
                "Split a Nat, Bool, Option, List or enum.",
            );
        };
        // `induction x generalizing y, z` reverts y and z before the split, so
        // the induction hypothesis is quantified over them and can be used at
        // a value the statement did not fix.
        let mut generalized_names: Vec<(String, String, Type)> = vec![];
        let mut base_target = goal.target.clone();
        if !generalized.is_empty() {
            for node in generalized {
                let written = self.text(node).to_owned();
                let Some((_, Term::Var(internal), gen_ty)) = goal.ctx.scope.get(&written).cloned()
                else {
                    return fail(
                        node.span,
                        "E_PROOF_SHAPE",
                        format!("`{written}` is not a variable this induction can generalize."),
                        "Generalize a theorem parameter or a quantified variable.",
                    );
                };
                if goal
                    .ctx
                    .assumptions
                    .iter()
                    .any(|(_, fact)| fact.mentions(&internal))
                    || goal
                        .ctx
                        .hyps
                        .iter()
                        .any(|(n, fact)| n != &written && fact.mentions(&internal))
                {
                    return fail(
                        node.span,
                        "E_PROOF_SHAPE",
                        format!(
                            "`{written}` appears in a hypothesis, so generalizing it needs a reverted implication."
                        ),
                        "Keep the hypothesis out of scope, or prove the quantified statement directly.",
                    );
                }
                generalized_names.push((written, internal, gen_ty));
            }
            let mut scope = goal.ctx.scope.clone();
            scope
                .terms
                .retain(|(n, _, _)| !generalized_names.iter().any(|(written, _, _)| n == written));
            let ctx = Context {
                scope,
                hyps: goal.ctx.hyps.clone(),
                assumptions: goal.ctx.assumptions.clone(),
                smaller: goal.ctx.smaller.clone(),
                conditional: goal.ctx.conditional.clone(),
                induction_hyps: goal.ctx.induction_hyps.clone(),
            };
            for (_, internal, gen_ty) in generalized_names.iter().rev() {
                base_target = Prop::Forall {
                    name: internal.clone(),
                    ty: gen_ty.clone(),
                    body: Box::new(base_target),
                };
            }
            goal.ctx = ctx;
            goal.target = base_target.clone();
        }
        let Some(branches) = args.iter().find(|arg| arg.kind == K::TacticBlock) else {
            return fail(
                node.span,
                "E_PROOF_CASES",
                "Splitting a value needs one tactic block per case.",
                "Write cases n { Zero => { ... }, Succ(k) => { ... } }.",
            );
        };
        let mut covered = BTreeSet::new();
        let mut pending: Vec<(String, &Node, TacticGoal)> = vec![];
        for branch in &branches.children {
            let pattern = &branch.children[0];
            let ih_names = &branch.children[1..branch.children.len() - 1];
            let block = branch.children.last().unwrap();
            let mut scope = goal.ctx.scope.clone();
            let pat = self.pattern(pattern, &ty, &mut scope)?;
            let case = match &pat {
                Pat::Ctor(case, _) => {
                    if Self::covers_case(&pat) {
                        covered.insert(case.clone());
                    }
                    if matches!(&ty, Type::Named(id, _) if id == "List") {
                        self.text(pattern).to_owned()
                    } else {
                        case.clone()
                    }
                }
                _ => {
                    return fail(
                        pattern.span,
                        "E_PROOF_CASES",
                        "A case needs a constructor pattern such as Zero or Succ(k).",
                        "Write one branch for each case.",
                    );
                }
            };
            let case_term = self.pat_term(&pat, &ty);
            let mut ctx = goal.ctx.clone();
            let mut map = BTreeMap::new();
            map.insert(var.clone(), case_term.clone());
            let mut fresh = self.fresh;
            ctx.scope.terms = ctx
                .scope
                .terms
                .iter()
                .map(|(n, t, ty)| (n.clone(), t.subst(&map, &mut fresh), ty.clone()))
                .collect();
            ctx.hyps = ctx
                .hyps
                .iter()
                .map(|(n, p)| (n.clone(), p.subst_prop(&map, &mut fresh)))
                .collect();
            ctx.smaller = ctx.smaller.iter().map(|(n, i)| (n.clone(), *i)).collect();
            ctx.conditional = ctx
                .conditional
                .iter()
                .map(|(n, needs, prop)| {
                    (
                        n.clone(),
                        needs
                            .iter()
                            .map(|p| p.subst_prop(&map, &mut fresh))
                            .collect(),
                        prop.subst_prop(&map, &mut fresh),
                    )
                })
                .collect();
            let target = base_target.subst_prop(&map, &mut fresh);
            self.fresh = fresh;
            for (n, t, ty) in scope.terms.iter() {
                if ctx.scope.get(n).is_none() {
                    ctx.scope.terms.push((n.clone(), t.clone(), ty.clone()));
                }
            }
            let origin = goal.ctx.smaller.get(&var).copied().or_else(|| {
                current
                    .theorem
                    .params
                    .iter()
                    .position(|(name, _)| name == &var)
            });
            if let Some(index) = origin {
                let mut bound = BTreeSet::new();
                pat.binders(&mut bound);
                for name in bound {
                    ctx.smaller.insert(name, index);
                }
            }
            if !induction && !ih_names.is_empty() {
                return fail(
                    branch.span,
                    "E_PROOF_CASES",
                    "A cases branch does not introduce induction hypotheses.",
                    "Use induction when the branch needs recursive hypotheses.",
                );
            }
            if strong {
                let recursive = self.recursive_binders(&pat, &ty, branch.span)?;
                if case == "Zero" {
                    if !ih_names.is_empty() {
                        return fail(
                            branch.span,
                            "E_PROOF_CASES",
                            "The Zero branch of strong induction has no hypothesis.",
                            "Remove the name after the pattern.",
                        );
                    }
                } else {
                    if recursive.len() != 1 {
                        return fail(
                            branch.span,
                            "E_PROOF_CASES",
                            "The Succ branch of strong induction needs the predecessor.",
                            "Write Succ(k) ih.",
                        );
                    }
                    if ih_names.len() != 1 {
                        return fail(
                            branch.span,
                            "E_PROOF_CASES",
                            "A strong induction branch takes exactly one hypothesis.",
                            "Write Succ(k) ih.",
                        );
                    }
                    let ih_name = self.text(&ih_names[0]).to_owned();
                    let j = self.fresh("j");
                    let k = recursive[0].clone();
                    let mut fresh = self.fresh;
                    let at_j = {
                        let map = BTreeMap::from([(var.clone(), Term::Var(j.clone()))]);
                        base_target.subst_prop(&map, &mut fresh)
                    };
                    let mut body = at_j;
                    let mut needs = goal
                        .ctx
                        .assumptions
                        .iter()
                        .filter(|(_, p)| p.mentions(&var))
                        .map(|(_, p)| {
                            let map = BTreeMap::from([(var.clone(), Term::Var(j.clone()))]);
                            p.subst_prop(&map, &mut fresh)
                        })
                        .collect::<Vec<_>>();
                    for need in needs.drain(..).rev() {
                        body = Prop::Implies(Box::new(need), Box::new(body));
                    }
                    let bound = Prop::Holds(Term::Op(
                        "<=".into(),
                        vec![Term::Var(j.clone()), Term::Var(k)],
                    ));
                    body = Prop::Implies(Box::new(bound), Box::new(body));
                    let ih = Prop::Forall {
                        name: j,
                        ty: Type::named("Nat"),
                        body: Box::new(body),
                    };
                    self.fresh = fresh;
                    ctx.induction_hyps.push(ih_name.clone());
                    ctx.hyps.push((ih_name, ih));
                }
            } else if induction {
                let recursive = self.recursive_binders(&pat, &ty, branch.span)?;
                self.induction_names(branch.span, ih_names, recursive.len())?;
                for (name, small) in ih_names.iter().zip(&recursive) {
                    let ih_name = self.text(name).to_owned();
                    let ih_map = BTreeMap::from([(var.clone(), Term::Var(small.clone()))]);
                    let mut fresh = self.fresh;
                    let ih = base_target.subst_prop(&ih_map, &mut fresh);
                    let needs = goal
                        .ctx
                        .assumptions
                        .iter()
                        .filter(|(_, p)| p.mentions(&var))
                        .map(|(_, p)| p.subst_prop(&ih_map, &mut fresh))
                        .collect::<Vec<_>>();
                    self.fresh = fresh;
                    if needs.is_empty() {
                        ctx.induction_hyps.push(ih_name.clone());
                        ctx.hyps.push((ih_name, ih));
                    } else {
                        ctx.conditional.push((ih_name, needs, ih));
                    }
                }
            }
            let initial = TacticGoal {
                ctx,
                target,
                _span: block.span,
            };
            if self.record_tactics {
                let entry = snapshot(std::slice::from_ref(&initial));
                self.tactic_states.push(StepState {
                    span: branch.span,
                    before: entry.clone(),
                    after: entry,
                    parent: Some(node.span),
                    label: case.clone(),
                    offset: branch.span.start,
                });
            }
            pending.push((case.clone(), block, initial));
        }
        let missing = variants
            .iter()
            .filter(|(c, _)| !covered.contains(c))
            .map(|(c, _)| c.clone())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            let fixes = crate::actions::tactic_case_fixes(
                &self.parsed.source,
                branches,
                &ty,
                &variants,
                &missing,
                induction,
            );
            return fail_with(
                branches.span,
                "E_PROOF_CASES",
                format!(
                    "The proof does not cover {}.",
                    missing
                        .iter()
                        .map(|case| sequences::case_label(case))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                "Add a branch for every case.",
                fixes,
            );
        }
        for (case, block, initial) in pending {
            let mut inner = vec![initial];
            self.run_tactics_into(block, &mut inner, current, None)?;
            if !inner.is_empty() {
                return fail(
                    block.span,
                    "E_PROOF_INCOMPLETE",
                    format!("The {case} case still has {}.", described(&inner)),
                    "Close every goal in this branch.",
                );
            }
        }
        Ok(vec![])
    }

    fn cases_hypothesis(
        &mut self,
        node: &Node,
        name: &str,
        prop: Prop,
        goal: &TacticGoal,
        remove_source: bool,
    ) -> Found<Vec<TacticGoal>> {
        // A named predicate is a definition; split its unfolded form.
        let prop = self.normal(&prop);
        match self.unfold_head(&prop) {
            Prop::And(a, b) => {
                let mut ctx = goal.ctx.clone();
                ctx.hyps.push((format!("{name}l"), *a));
                ctx.hyps.push((format!("{name}r"), *b));
                Ok(vec![self.tactic_subgoal(
                    &ctx,
                    goal.target.clone(),
                    node.span,
                )])
            }
            Prop::Or(a, b) => {
                let mut left = goal.ctx.clone();
                left.hyps.push((format!("{name}l"), *a));
                let mut right = goal.ctx.clone();
                right.hyps.push((format!("{name}r"), *b));
                Ok(vec![
                    self.tactic_subgoal(&left, goal.target.clone(), node.span),
                    self.tactic_subgoal(&right, goal.target.clone(), node.span),
                ])
            }
            Prop::Iff(a, b) => {
                let mut ctx = goal.ctx.clone();
                ctx.hyps
                    .push((format!("{name}mp"), Prop::Implies(a.clone(), b.clone())));
                ctx.hyps.push((format!("{name}mpr"), Prop::Implies(b, a)));
                Ok(vec![self.tactic_subgoal(
                    &ctx,
                    goal.target.clone(),
                    node.span,
                )])
            }
            Prop::Exists {
                name: bound,
                ty,
                body,
            } => {
                // The witness needs a name the proof can write. Prefer the
                // quantifier's own binder, and step aside from any name the
                // context already uses, so nothing is conflated.
                let base = shown(&bound);
                let base = if base.starts_with('@') { "s" } else { base };
                let used = used_names(&goal.ctx, &goal.target);
                let mut alias = base.to_owned();
                let mut index = 0;
                while used.contains(&alias) {
                    index += 1;
                    alias = format!("{base}{index}");
                }
                let witness = self.fresh(&alias);
                let mut map = BTreeMap::new();
                map.insert(bound.clone(), Term::Var(witness.clone()));
                let mut fresh = self.fresh;
                let body = body.subst_prop(&map, &mut fresh);
                self.fresh = fresh;
                let mut ctx = goal.ctx.clone();
                ctx.scope = ctx.scope.with(&alias, Term::Var(witness.clone()), ty);
                ctx.hyps.push((format!("{name}w"), body));
                Ok(vec![self.tactic_subgoal(
                    &ctx,
                    goal.target.clone(),
                    node.span,
                )])
            }
            // `h : a = b` renames the variable side everywhere in the goal and
            // the context. This is definitional equality, not a new rule. Two
            // constructor values are injective: `Succ(a) = Succ(b)` gives the
            // field equations, and different cases close the goal.
            Prop::Eq(left, right) => {
                let mut map = BTreeMap::new();
                let mut equations: Vec<(Term, Term)> = vec![];
                let mut changed = false;
                let mut clash = false;
                self.equality_elimination(
                    &left,
                    &right,
                    &mut map,
                    &mut equations,
                    &mut changed,
                    &mut clash,
                    node.span,
                )?;
                if clash {
                    return Ok(vec![]);
                }
                if !changed && map.is_empty() && equations.is_empty() {
                    return fail(
                        node.span,
                        "E_PROOF_SHAPE",
                        format!(
                            "cases cannot split `{}` because neither side is a name or a constructor.",
                            pretty_prop(&Prop::Eq(left, right))
                        ),
                        "Split an equality between a local name and a term.",
                    );
                }
                for variable in map.keys() {
                    if goal.ctx.conditional.iter().any(|(_, needs, statement)| {
                        needs.iter().any(|p| p.mentions(variable)) || statement.mentions(variable)
                    }) || goal.ctx.induction_hyps.iter().any(|name| {
                        goal.ctx
                            .hyps
                            .iter()
                            .any(|(n, p)| n == name && p.mentions(variable))
                    }) {
                        return fail(
                            node.span,
                            "E_PROOF_SHAPE",
                            format!(
                                "cases cannot rewrite `{}` while it takes part in an induction.",
                                shown(variable)
                            ),
                            "Split the variable with induction instead.",
                        );
                    }
                }
                let mut ctx = goal.ctx.clone();
                if remove_source {
                    ctx.hyps.retain(|(n, _)| n != name);
                }
                ctx.hyps = ctx
                    .hyps
                    .into_iter()
                    .map(|(n, p)| (n, p.subst_prop(&map, &mut self.fresh)))
                    .collect();
                ctx.scope.terms = ctx
                    .scope
                    .terms
                    .into_iter()
                    .map(|(n, term, ty)| (n, term.subst(&map, &mut self.fresh), ty))
                    .collect();
                for (index, (a, b)) in equations.into_iter().enumerate() {
                    let a = a.subst(&map, &mut self.fresh);
                    let b = b.subst(&map, &mut self.fresh);
                    ctx.hyps.push((format!("{name}{index}"), Prop::Eq(a, b)));
                }
                let target = goal.target.subst_prop(&map, &mut self.fresh);
                Ok(vec![self.tactic_subgoal(&ctx, target, node.span)])
            }
            other => fail(
                node.span,
                "E_PROOF_SHAPE",
                format!("cases cannot split `{}`.", pretty_prop(&other)),
                "Split an and, or or exists assumption, or give cases a variable and branches.",
            ),
        }
    }

    /// Constructor injectivity for `cases` on an equality. Names on one side
    /// are substituted; field pairs with no name on either side become new
    /// hypotheses; different constructors close the goal.
    #[allow(clippy::too_many_arguments)]
    fn equality_elimination(
        &mut self,
        left: &Term,
        right: &Term,
        map: &mut BTreeMap<String, Term>,
        equations: &mut Vec<(Term, Term)>,
        changed: &mut bool,
        clash: &mut bool,
        _span: Span,
    ) -> Found<()> {
        let (left, right) = {
            let mut reducer = self.reducer();
            (reducer.nf(left), reducer.nf(right))
        };
        // A folded numeral still stands for a Succ chain, so peel it like one.
        fn numeral_as_ctor(term: Term) -> Term {
            match term {
                Term::Nat(n) if n > 0 => Term::Ctor {
                    ty: "Nat".into(),
                    case: "Succ".into(),
                    args: vec![Term::Nat(n - 1)],
                },
                other => other,
            }
        }
        let left = numeral_as_ctor(left);
        let right = numeral_as_ctor(right);
        if left == right {
            *changed = true;
            return Ok(());
        }
        match (&left, &right) {
            (
                Term::Ctor {
                    ty: a,
                    case: b,
                    args: xs,
                },
                Term::Ctor {
                    ty: c,
                    case: d,
                    args: ys,
                },
            ) if a == c && b == d && xs.len() == ys.len() => {
                *changed = true;
                for (x, y) in xs.iter().zip(ys).rev() {
                    match (x, y) {
                        (Term::Var(v), _) if !map.contains_key(v) => {
                            map.insert(v.clone(), y.clone());
                        }
                        (_, Term::Var(v)) if !map.contains_key(v) => {
                            map.insert(v.clone(), x.clone());
                        }
                        _ if x != y => equations.push((x.clone(), y.clone())),
                        _ => {}
                    }
                }
                Ok(())
            }
            (Term::Var(v), _) => {
                *changed = true;
                map.insert(v.clone(), right.clone());
                Ok(())
            }
            (_, Term::Var(v)) => {
                *changed = true;
                map.insert(v.clone(), left.clone());
                Ok(())
            }
            _ if values_clash(&left, &right) => {
                // Different constructors cannot be equal, so this branch proves
                // any goal, as `contradiction` does.
                *clash = true;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// `byCases h : expr` splits a Bool expression into the goal where it is
    /// true and the goal where it is false. Every Bool has one of those two
    /// values, so the two branches cover the cases.
    fn tactic_by_cases(
        &mut self,
        node: &Node,
        args: &[Node],
        goal: &TacticGoal,
    ) -> Found<Vec<TacticGoal>> {
        let (Some(name), Some(value)) = (args.first(), args.get(1)) else {
            return self.tactic_argument(node, "byCases needs `h : expr`.");
        };
        let name = self.text(name).to_owned();
        let (term, ty) = self.term(value, Some(&Type::named("Bool")), &goal.ctx.scope)?;
        if ty != Type::named("Bool") {
            return fail(
                value.span,
                "E_PROOF_TYPE",
                format!("byCases needs a Bool expression, but this is {ty}."),
                "Write byCases h : condition with a Bool condition.",
            );
        }
        let mut when_true = goal.clone();
        when_true
            .ctx
            .hyps
            .push((name.clone(), Prop::Eq(term.clone(), Term::Bool(true))));
        let mut when_false = goal.clone();
        when_false
            .ctx
            .hyps
            .push((name, Prop::Eq(term, Term::Bool(false))));
        Ok(vec![when_true, when_false])
    }

    fn tactic_apply(
        &mut self,
        node: &Node,
        args: &[Node],
        goal: &TacticGoal,
        current: &mut Current,
    ) -> Found<Vec<TacticGoal>> {
        let Some(rule_node) = args.first() else {
            return self.tactic_argument(node, "apply needs a theorem, assumption or implication.");
        };
        let rule_node = ungroup(rule_node);
        // A projection is a fact, not a named theorem: `apply(h.left)` uses the
        // conjunction elimination rule directly.
        let projected = rule_node.kind == K::Member
            && self.projects_fact(&rule_node.children[0], &goal.ctx, current);
        let (callee, explicit) = match rule_node.kind {
            K::Call => (
                crate::lexer::path_name(self.text(&rule_node.children[0])),
                self.arguments(rule_node),
            ),
            _ => (crate::lexer::path_name(self.text(rule_node)), vec![]),
        };
        let mut supplied = BTreeMap::new();
        let mut rule = if projected {
            self.infer(rule_node, &goal.ctx, current)?
        } else if let Some((_, prop)) = goal
            .ctx
            .hyps
            .iter()
            .rev()
            .find(|(n, _)| *n == callee)
            .cloned()
        {
            prop
        } else if let Some(theorem) = self.theorems.get(&callee).cloned() {
            if theorem.trusted_smt {
                self.smt_trusted.set(true);
            }
            if theorem.admitted {
                current.admitted = true;
            }
            if explicit.len() > theorem.params.len() {
                return fail(
                    node.span,
                    "E_TACTIC_ARGUMENT",
                    format!(
                        "`{callee}` takes {} argument(s), but {} were given.",
                        theorem.params.len(),
                        explicit.len()
                    ),
                    "Pass only the theorem's parameters, in order.",
                );
            }
            let mut map = BTreeMap::new();
            for (arg, (name, ty)) in explicit.iter().zip(&theorem.params) {
                let (term, found) = self.term(arg, Some(ty), &goal.ctx.scope)?;
                if !compatible(ty, &found) {
                    return fail(
                        arg.span,
                        "E_PROOF_TYPE",
                        format!("This argument is {found}, but `{callee}` needs {ty}."),
                        "Pass arguments in the theorem's parameter order.",
                    );
                }
                self.defined_term(&term, &goal.ctx, arg.span)?;
                map.insert(name.clone(), term);
            }
            if let Some(error) = dispatch_override_error(
                &theorem.dispatches,
                &self.checked.environment.definitions,
                node.span,
            ) {
                return Err(error);
            }
            let arguments = map.clone();
            self.add_class_projection_substitutions(&theorem.params, &arguments, &mut map);
            let mut fresh = self.fresh;
            let mut statement = theorem.statement.subst_prop(&map, &mut fresh);
            self.fresh = fresh;
            for (_, premise) in &theorem.requires {
                let premise = premise.subst_prop(&map, &mut fresh);
                self.fresh = fresh;
                statement = Prop::Implies(Box::new(premise), Box::new(statement));
            }
            supplied = map;
            statement
        } else {
            return fail(
                rule_node.span,
                "E_PROOF_UNKNOWN",
                format!("`{callee}` is not an assumption or a checked theorem."),
                "Apply an assumption from requires or an earlier theorem.",
            );
        };
        if self.same(&rule, &goal.target) {
            return Ok(vec![]);
        }
        let mut holes = BTreeMap::new();
        // Only theorem parameters and quantified binders are holes. A free
        // variable of a hypothesis is fixed, even when the goal has a different
        // name with the same shape.
        if let Some(theorem) = self.theorems.get(&callee) {
            for (name, ty) in self.theorem_matching_params(theorem) {
                if !supplied.contains_key(&name) {
                    holes.insert(name, ty);
                }
            }
        }
        // A universal statement's binders are holes; peel them first so the
        // remaining conclusion can match the goal as a whole.
        let mut peeling = rule.clone();
        loop {
            match peeling {
                Prop::Forall { name, ty, body } => {
                    holes.insert(name, ty);
                    peeling = *body;
                }
                other => {
                    peeling = other;
                    break;
                }
            }
        }
        let whole_holes = holes.keys().cloned().collect::<BTreeSet<_>>();
        // A theorem whose conclusion is itself an implication can be applied
        // directly to an implication goal: `apply identity` proves `Q -> Q`,
        // and `apply id` proves `(A and B) -> (A and B)`. Try the remaining
        // statement first, then fall back to peeling premises.
        let mut whole = supplied.clone();
        let matched_whole = unify_prop(&peeling, &goal.target, &whole_holes, &mut whole)
            && holes.keys().all(|name| whole.contains_key(name));
        let mut premises = vec![];
        rule = peeling;
        if matched_whole {
            supplied = whole;
        } else {
            loop {
                // Keep a subset conclusion in its written form while the goal
                // is also a subset, so the two match directly. A membership
                // goal unfolds the subset into its quantified implication.
                if matches!(rule, Prop::Subset { .. }) && matches!(goal.target, Prop::Subset { .. })
                {
                    break;
                }
                rule = self.unfold_head(&rule);
                match rule {
                    Prop::Forall { name, ty, body } => {
                        holes.insert(name, ty);
                        rule = *body;
                    }
                    Prop::Implies(premise, conclusion) => {
                        premises.push(*premise);
                        rule = *conclusion;
                    }
                    Prop::Not(inner) => {
                        premises.push(*inner);
                        rule = Prop::False;
                    }
                    other => {
                        rule = other;
                        break;
                    }
                }
            }
        }
        let has_class_params = self
            .theorems
            .get(&callee)
            .is_some_and(|theorem| self.theorem_matching_params(theorem) != theorem.params);
        if has_class_params {
            rule = self.normal(&rule);
        }
        let target = if has_class_params {
            self.normal(&goal.target)
        } else {
            goal.target.clone()
        };
        // Matching fills only the holes. Explicit arguments were already
        // substituted into the statement, so they must not be applied again:
        // a supplied `a := 1` would otherwise rewrite the goal's own `a`.
        let mut binding = if matched_whole {
            supplied.clone()
        } else {
            BTreeMap::new()
        };
        let hole_names = holes.keys().cloned().collect::<BTreeSet<_>>();
        if !matched_whole && !unify_prop(&rule, &target, &hole_names, &mut binding) {
            // Definitions in an argument can differ syntactically while being
            // the same proposition (`fdiff(s, t)` against `FDiff(s, t)`). With
            // no holes left, definitional equality is the whole test.
            let same = holes.is_empty() && {
                let rule = self.normal(&rule);
                let target = self.normal(&goal.target);
                self.same(&rule, &target)
            };
            if !same {
                return fail(
                    node.span,
                    "E_PROOF_MISMATCH",
                    format!(
                        "`{callee}` concludes `{}`, which does not match the goal `{}`.",
                        pretty_prop(&rule),
                        pretty_prop(&goal.target)
                    ),
                    "Apply a theorem whose conclusion matches the goal.",
                );
            }
        }
        if !matched_whole {
            for (name, value) in &supplied {
                binding.entry(name.clone()).or_insert_with(|| value.clone());
            }
        }
        for name in holes.keys() {
            if !binding.contains_key(name) {
                return fail(
                    node.span,
                    "E_PROOF_UNKNOWN",
                    format!(
                        "`{callee}` needs a value for `{name}`, and the goal does not determine it."
                    ),
                    "Pass the argument explicitly, as in apply addZeroR(n).",
                );
            }
        }
        if let Some(theorem) = self.theorems.get(&callee).cloned() {
            if !self.complete_class_arguments(&theorem, &mut binding) {
                return fail(
                    node.span,
                    "E_PROOF_UNKNOWN",
                    "This application does not determine every class field.",
                    "Pass the class argument explicitly.",
                );
            }
            for (name, _) in &theorem.params {
                self.defined_term(&binding[name], &goal.ctx, node.span)?;
            }
        }
        if let Some(theorem) = self.theorems.get(&callee).cloned()
            && !self.rewrite_bounds(&theorem, &binding, &goal.ctx)
        {
            return fail(
                node.span,
                "E_PROOF_BOUND",
                "This application needs a trait bound that the goal does not establish.",
                "Add the bound to the theorem's type parameter, or apply it at a type that provides the trait.",
            );
        }
        let mut fresh = self.fresh;
        let mut goals = vec![];
        let inferred = binding
            .iter()
            .filter(|(name, _)| holes.contains_key(*name))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        for premise in premises {
            let premise = premise.subst_prop(&inferred, &mut fresh);
            let free = free_prop(&premise);
            if free
                .iter()
                .any(|name| holes.contains_key(name) && !binding.contains_key(name))
            {
                return fail(
                    node.span,
                    "E_PROOF_UNKNOWN",
                    "A premise of this theorem still has an undetermined argument.",
                    "Apply the theorem with explicit arguments.",
                );
            }
            goals.push(self.tactic_subgoal(&goal.ctx, premise, node.span));
        }
        self.fresh = fresh;
        let defs = self.defined_prop(&goal.target, &goal.ctx, node.span);
        if defs.is_err() {
            // The applied rule establishes the goal only through its premises;
            // definedness is checked when those premises are proved.
        }
        Ok(goals)
    }

    fn tactic_block(
        &mut self,
        block: &Node,
        state: &mut Vec<TacticGoal>,
        current: &mut Current,
        span: Span,
        attempt: bool,
    ) -> Found<()> {
        let saved = state.clone();
        let saved_trust = self.smt_trusted.get();
        let mut rounds = 0;
        loop {
            let before = snapshot(state);
            if let Err(error) = self.run_tactics_into(block, state, current, None) {
                if attempt {
                    *state = saved;
                    self.smt_trusted.set(saved_trust);
                    return Ok(());
                }
                return Err(error);
            }
            rounds += 1;
            if state.is_empty() || snapshot(state) == before || rounds >= 64 {
                break;
            }
        }
        if !attempt && rounds >= 64 && !state.is_empty() {
            return fail(
                span,
                "E_PROOF_LIMIT",
                "repeat stopped after 64 rounds without finishing.",
                "Make each round simplify the goal.",
            );
        }
        Ok(())
    }
}

/// The free variables of a proposition, respecting its own binders.
fn free_names(prop: &Prop, out: &mut BTreeSet<String>) {
    prop.free(out);
}

/// Every name the proof can mention: hypotheses, scoped values and the free
/// variables of their statements and the goal.
fn used_names(ctx: &Context, target: &Prop) -> BTreeSet<String> {
    let mut used = BTreeSet::new();
    for (name, prop) in &ctx.hyps {
        used.insert(name.clone());
        free_names(prop, &mut used);
    }
    free_names(target, &mut used);
    for (name, term, _) in &ctx.scope.terms {
        used.insert(name.clone());
        term.free(&mut used);
    }
    used.extend(ctx.smaller.keys().cloned());
    used.extend(ctx.conditional.iter().map(|(name, _, _)| name.clone()));
    used
}

fn free_prop(prop: &Prop) -> BTreeSet<String> {
    let mut free = BTreeSet::new();
    prop.free(&mut free);
    free
}

/// Beta-reduces lambda applications without unfolding named functions. Used by
/// matching so a pattern such as `f(n)` follows a binding `f := (x => x)` to `n`.
fn reduce_lambdas(term: &Term) -> Term {
    match term {
        Term::App(function, args) => {
            let function = reduce_lambdas(function);
            let args = args.iter().map(reduce_lambdas).collect::<Vec<_>>();
            if let Term::Lam(params, body) = function {
                if args.len() >= params.len() {
                    let mut map = BTreeMap::new();
                    for ((name, _), arg) in params.iter().zip(args.iter()) {
                        map.insert(name.clone(), arg.clone());
                    }
                    let reduced = body.subst(&map, &mut 0);
                    let rest = args[params.len()..].to_vec();
                    if rest.is_empty() {
                        return reduce_lambdas(&reduced);
                    }
                    return reduce_lambdas(&Term::App(Box::new(reduced), rest));
                }
                return Term::App(Box::new(Term::Lam(params, body)), args);
            }
            Term::App(Box::new(function), args)
        }
        Term::Lam(params, body) => Term::Lam(params.clone(), Box::new(reduce_lambdas(body))),
        Term::If(c, a, b) => Term::If(
            Box::new(reduce_lambdas(c)),
            Box::new(reduce_lambdas(a)),
            Box::new(reduce_lambdas(b)),
        ),
        Term::Ctor { ty, case, args } => Term::Ctor {
            ty: ty.clone(),
            case: case.clone(),
            args: args.iter().map(reduce_lambdas).collect(),
        },
        Term::Op(op, args) => Term::Op(op.clone(), args.iter().map(reduce_lambdas).collect()),
        Term::Match(value, arms) => Term::Match(
            Box::new(reduce_lambdas(value)),
            arms.iter()
                .map(|(pat, body)| (pat.clone(), reduce_lambdas(body)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// One-way matching for `apply`: pattern variables in `holes` bind to terms.
fn unify_term(
    pattern: &Term,
    target: &Term,
    holes: &BTreeSet<String>,
    binding: &mut BTreeMap<String, Term>,
) -> bool {
    // Follow bindings already made in this match, then beta-reduce, so a
    // repeated variable matches at every occurrence.
    match pattern {
        Term::Var(name) if holes.contains(name) => {
            if let Some(found) = binding.get(name) {
                return found == target;
            }
            binding.insert(name.clone(), target.clone());
            true
        }
        Term::Var(name) => matches!(target, Term::Var(other) if name == other),
        Term::Nat(a) => matches!(target, Term::Nat(b) if a == b),
        Term::Int(a) => matches!(target, Term::Int(b) if a == b),
        Term::Bool(a) => matches!(target, Term::Bool(b) if a == b),
        Term::Ctor { ty, case, args } => {
            matches!(target, Term::Ctor { ty: t, case: c, args: a } if ty == t && case == c && a.len() == args.len())
                && args
                    .iter()
                    .zip(match target {
                        Term::Ctor { args, .. } => args,
                        _ => unreachable!(),
                    })
                    .all(|(p, t)| unify_term(p, t, holes, binding))
        }
        Term::Call { function, args, .. } => {
            matches!(target, Term::Call { function: f, args: a, .. } if function == f && a.len() == args.len())
                && args
                    .iter()
                    .zip(match target {
                        Term::Call { args, .. } => args,
                        _ => unreachable!(),
                    })
                    .all(|(p, t)| unify_term(p, t, holes, binding))
        }
        Term::Opaque { name, args } => {
            matches!(target, Term::Opaque { name: n, args: a } if name == n && a.len() == args.len())
                && args
                    .iter()
                    .zip(match target {
                        Term::Opaque { args, .. } => args,
                        _ => unreachable!(),
                    })
                    .all(|(p, t)| unify_term(p, t, holes, binding))
        }
        Term::Op(op, args) => {
            matches!(target, Term::Op(o, a) if op == o && a.len() == args.len())
                && args
                    .iter()
                    .zip(match target {
                        Term::Op(_, args) => args,
                        _ => unreachable!(),
                    })
                    .all(|(p, t)| unify_term(p, t, holes, binding))
        }
        Term::If(c, a, b) => {
            if let Term::If(tc, ta, tb) = target {
                unify_term(c, tc, holes, binding)
                    && unify_term(a, ta, holes, binding)
                    && unify_term(b, tb, holes, binding)
            } else {
                false
            }
        }
        Term::App(function, args) => {
            // A pattern such as `f(n)` follows a binding `f := (x => e)` by
            // beta-reducing it once, rather than rewriting the whole pattern.
            if let Term::Var(name) = function.as_ref()
                && let Some(bound) = binding.get(name)
                && matches!(bound, Term::Lam(..))
            {
                let resolved = reduce_lambdas(&Term::App(Box::new(bound.clone()), args.clone()));
                return unify_term(&resolved, target, holes, binding);
            }
            if let Term::App(other, other_args) = target {
                other_args.len() == args.len()
                    && unify_term(function, other, holes, binding)
                    && args
                        .iter()
                        .zip(other_args)
                        .all(|(p, t)| unify_term(p, t, holes, binding))
            } else {
                false
            }
        }
        Term::Lam(..) => pattern == target,
        Term::Prop(a) => matches!(target, Term::Prop(b) if unify_prop(a, b, holes, binding)),
        Term::FamilyUnion { set, element } => matches!(
            target,
            Term::FamilyUnion { set: other, element: other_element }
                if element == other_element && unify_term(set, other, holes, binding)
        ),
        Term::FamilyInter { set, element } => matches!(
            target,
            Term::FamilyInter { set: other, element: other_element }
                if element == other_element && unify_term(set, other, holes, binding)
        ),
        Term::Builder {
            name,
            element,
            body,
        } => match target {
            Term::Builder {
                name: other_name,
                element: other_element,
                body: other_body,
            } => {
                if element != other_element {
                    return false;
                }
                // Match under the binder: rename the target's binder to the
                // pattern's, then unify the predicates so theorem parameters
                // inside the builder can still become holes.
                let other_body = other_body.rename_free(other_name, name, &mut 0);
                unify_prop(body, &other_body, holes, binding)
            }
            _ => false,
        },
        Term::Finite(elements) => match target {
            Term::Finite(other) => {
                elements.len() == other.len()
                    && elements
                        .iter()
                        .zip(other)
                        .all(|(a, b)| unify_term(a, b, holes, binding))
            }
            _ => false,
        },
        Term::Match(..) => pattern == target,
    }
}

pub(super) fn unify_prop(
    pattern: &Prop,
    target: &Prop,
    holes: &BTreeSet<String>,
    binding: &mut BTreeMap<String, Term>,
) -> bool {
    // A quoted proposition is the same proposition as the one it wraps.
    if let Prop::Atom(Term::Prop(inner)) = pattern {
        return unify_prop(inner, target, holes, binding);
    }
    if let Prop::Atom(Term::Prop(inner)) = target {
        return unify_prop(pattern, inner, holes, binding);
    }
    // A `Prop` parameter is a hole that can stand for any proposition, simple
    // or compound. Record it quoted so later occurrences match the same shape.
    if let Prop::Atom(Term::Var(name)) = pattern
        && holes.contains(name)
    {
        let quoted = Term::Prop(Box::new(target.clone()));
        return match binding.get(name) {
            Some(found) => found == &quoted,
            None => {
                binding.insert(name.clone(), quoted);
                true
            }
        };
    }
    match (pattern, target) {
        (Prop::Eq(a, b), Prop::Eq(c, d)) => {
            unify_term(a, c, holes, binding) && unify_term(b, d, holes, binding)
        }
        (Prop::Holds(a), Prop::Holds(b)) => unify_term(a, b, holes, binding),
        (Prop::And(a, b), Prop::And(c, d))
        | (Prop::Or(a, b), Prop::Or(c, d))
        | (Prop::Implies(a, b), Prop::Implies(c, d))
        | (Prop::Iff(a, b), Prop::Iff(c, d)) => {
            unify_prop(a, c, holes, binding) && unify_prop(b, d, holes, binding)
        }
        (Prop::Not(a), Prop::Not(b)) => unify_prop(a, b, holes, binding),
        (Prop::Mem(a, b), Prop::Mem(c, d)) => {
            unify_term(a, c, holes, binding) && unify_term(b, d, holes, binding)
        }
        (
            Prop::Subset {
                left: a, right: b, ..
            },
            Prop::Subset {
                left: c, right: d, ..
            },
        ) => unify_term(a, c, holes, binding) && unify_term(b, d, holes, binding),
        (Prop::True, Prop::True) | (Prop::False, Prop::False) => true,
        (Prop::Atom(a), Prop::Atom(b)) => unify_term(a, b, holes, binding),
        _ => false,
    }
}

/// The live proof state at a byte offset in a parsed, checked file. The editor
/// asks for this after each keystroke; a theorem that uses an explicit term has
/// one goal and no tactic snapshots.
pub fn goals_at(parsed: &Parsed, checked: &Checked, offset: usize) -> GoalReport {
    let nodes = parsed
        .root
        .children
        .iter()
        .filter(|n| matches!(n.kind, K::Theorem | K::Example | K::Axiom))
        .collect::<Vec<_>>();
    let Some(index) = nodes
        .iter()
        .position(|n| offset >= n.span.start && offset <= n.span.end)
    else {
        return GoalReport::default();
    };
    let mut kernel = Kernel::new(parsed, checked);
    for earlier in &nodes[..index] {
        let _ = kernel.theorem(earlier);
    }
    let mut report = kernel.theorem_goals(nodes[index], offset);
    report.in_theorem = true;
    report
}

impl Kernel<'_> {
    fn theorem_goals(&mut self, node: &Node, offset: usize) -> GoalReport {
        let (name, _theorem, ctx, statement) = match self.theorem_parts(node) {
            Ok(parts) => parts,
            Err(error) => {
                return GoalReport {
                    theorem: node
                        .child(K::Name)
                        .map(|n| self.text(n).to_owned())
                        .unwrap_or_default(),
                    goals: vec![],
                    cases: vec![],
                    error: Some(format!("{}: {}", error.code, error.message)),
                    complete: false,
                    admitted: false,
                    in_theorem: true,
                };
            }
        };
        let title = if node.kind == K::Example {
            "example".to_owned()
        } else {
            name.clone()
        };
        let Some(body) = node.child(K::Proof).and_then(|p| p.children.first()) else {
            return GoalReport {
                theorem: title.clone(),
                goals: vec![],
                cases: vec![],
                error: Some("This theorem has no proof.".into()),
                complete: false,
                admitted: false,
                in_theorem: true,
            };
        };
        let initial = Self::tactic_goal(ctx.clone(), statement.clone(), body.span);
        let initial_goals = snapshot(std::slice::from_ref(&initial));
        if body.kind != K::TacticProof {
            return GoalReport {
                theorem: title.clone(),
                goals: initial_goals,
                cases: vec![],
                error: Some(
                    "This theorem uses an explicit proof term; the goal is shown whole.".into(),
                ),
                complete: false,
                admitted: false,
                in_theorem: true,
            };
        }
        let Some(script) = body.child(K::TacticScript) else {
            return GoalReport {
                theorem: title.clone(),
                goals: initial_goals,
                cases: vec![],
                error: Some("This tactic proof has no script.".into()),
                complete: false,
                admitted: false,
                in_theorem: true,
            };
        };
        let admitted = _theorem.admitted;
        let mut current = Current {
            name: name.clone(),
            start: node.span.start,
            theorem: _theorem,
            candidates: None,
            admitted: false,
        };
        let mut state = vec![initial];
        self.tactic_states.clear();
        self.record_tactics = true;
        let outcome = self.run_tactics_into(script, &mut state, &mut current, None);
        self.record_tactics = false;
        let record = std::mem::take(&mut self.tactic_states);
        let final_goals = snapshot(&state);
        let mut chosen = initial_goals.clone();
        let mut branch = None;
        if let Some(first) = record.first() {
            if offset < first.span.start {
                chosen = initial_goals.clone();
            } else {
                chosen = final_goals.clone();
                let containing = record
                    .iter()
                    .filter(|step| offset >= step.span.start && offset < step.span.end)
                    .min_by_key(|step| step.span.end.saturating_sub(step.span.start));
                if let Some(step) = containing {
                    chosen = step.before.clone();
                } else {
                    for step in &record {
                        if offset >= step.span.end {
                            chosen = step.after.clone();
                        }
                    }
                }
                branch = record
                    .iter()
                    .filter(|step| {
                        step.parent.is_some() && offset >= step.span.start && offset < step.span.end
                    })
                    .min_by_key(|step| step.span.end.saturating_sub(step.span.start));
            }
        }
        let mut cases = vec![];
        let group = branch.and_then(|step| step.parent).or_else(|| {
            record
                .iter()
                .filter_map(|step| step.parent)
                .find(|parent| offset >= parent.start && offset < parent.end)
        });
        if let Some(parent) = group {
            let mut first = true;
            for step in record.iter().filter(|step| step.parent == Some(parent)) {
                if let Some(goal) = step.before.first() {
                    let (line, column) = self.parsed.source.position(step.offset);
                    let active = branch.map_or(first, |branch| branch.span == step.span);
                    first = false;
                    cases.push(CaseGoal {
                        label: step.label.clone(),
                        goal: goal.clone(),
                        line,
                        column,
                        active,
                    });
                }
            }
        }
        let error = outcome.err().map(|error| {
            let hint = error
                .hint
                .as_deref()
                .map(|hint| format!(" {hint}"))
                .unwrap_or_default();
            format!("{}: {}{}", error.code, error.message, hint)
        });
        GoalReport {
            theorem: title,
            goals: chosen,
            cases,
            complete: error.is_none() && state.is_empty(),
            error,
            admitted: admitted || current.admitted,
            in_theorem: true,
        }
    }
}
