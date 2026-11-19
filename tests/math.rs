//! Standard mathematical modules: natural numbers, sets, functions, real
//! numbers and finite algebra.
use kestrel::modules::check_source;

fn good(source: &str) {
    let checked = check_source("math.kes", source);
    assert!(
        checked.checked.is_ok(),
        "{source}\n{:#?}",
        checked.checked.diagnostics
    );
}

#[test]
fn natural_number_lemmas_are_imported_and_checked() {
    good(
        "import std.nat.{plus, times, plusZeroR, plusSuccR, timesZeroR}\n\
         theorem use(n : Nat) : plus(n, Zero) = n by { apply plusZeroR }\n\
         theorem useTimes(n : Nat) : times(n, Zero) = Zero by { apply timesZeroR }\n\
         let value := plus(2, 3)\n\
         let other := times(4, 5)\n",
    );
}

#[test]
fn sets_expose_membership_subsets_and_disjointness() {
    good(
        "import std.set.{isEmpty, member, subset, properSubset, disjoint, cardinality}\n\
         let a := Set().add(1).add(2)\n\
         let b := Set().add(1).add(2).add(3)\n\
         let yes : Bool = member(b, 2)\n\
         let inside : Bool = subset(a, b)\n\
         let proper : Bool = properSubset(a, b)\n\
         let apart : Bool = disjoint(a, Set().add(9))\n\
         let size : Int = cardinality(b)\n\
         let emptySet : Set[Int] = Set()
         let empty : Bool = isEmpty(emptySet)\n",
    );
    good(
        "import std.set.{subset}\n\
         let a := Set().add(\"x\")\n\
         let b := Set().add(\"x\").add(\"y\")\n\
         let inside : Bool = subset(a, b)\n",
    );
}

#[test]
fn functions_compose_without_borrowing_their_arguments() {
    good(
        "import std.func.{identity, compose, constant}\n\
         function twice(x : Int) : Int = x * 2\n\
         function addOne(x : Int) : Int = x + 1\n\
         let f := compose(twice, addOne)\n\
         let n : Int = f(3)\n\
         let same : Int = identity(4)\n\
         let fixed : String = constant(\"value\", 0)\n",
    );
}

#[test]
fn real_numbers_use_the_double_precision_alias() {
    good(
        "import std.real.{Real, PI, E, abs, sqrt, min, max, isFinite}\n\
         let x : Real = abs(-PI)\n\
         let y : Real = sqrt(9.0)\n\
         let z : Real = min(PI, E)\n\
         let w : Real = max(PI, E)\n\
         let finite : Bool = isFinite(x)\n",
    );
}

#[test]
fn finite_algebra_checks_group_and_ring_laws() {
    good(
        "import std.algebra.{Z2, Z2Group, z2Plus, z2Times, z2Neg, z2PlusAssoc, z2PlusZeroL, z2PlusNeg, z2TimesAssoc, z2Distrib}\n\
         let a : Z2 = Z2.One\n\
         let b : Z2 = z2Plus(a, a)\n\
         let c : Z2 = z2Times(a, a)\n\
         let d : Z2 = z2Neg(a)\n\
         let g := Z2Group()\n\
         let e : Z2 = g.op(a, a)\n\
         theorem assocAgain(a : Z2, b : Z2, c : Z2) : z2Plus(z2Plus(a, b), c) = z2Plus(a, z2Plus(b, c)) by { apply z2PlusAssoc }\n",
    );
}

#[test]
fn the_math_example_checks_as_one_file() {
    good(include_str!("../testdata/language/math.kes"));
}
