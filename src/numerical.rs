//! Declarations for explicitly imported numerical modules. Kernels live in numerical_runtime.c.
use crate::{
    checker::{Definition, Environment, Symbol, Visibility},
    probability::{Distribution, Parameter},
    resolution::{Callable, Reference},
    syntax::SyntaxKind as K,
    types::{FunctionType, Passing, Type},
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const UNARY: &[&str] = &[
    "exp", "expm1", "log", "log1p", "log2", "log10", "sin", "cos", "tan", "asin", "acos", "atan",
    "sinh", "cosh", "tanh", "sqrt", "abs", "floor", "ceil", "round",
];
pub(crate) const LINALG: &[&str] = &[
    "dot",
    "outer",
    "norm",
    "frobeniusNorm",
    "spectralNorm",
    "solve",
    "solveLower",
    "solveUpper",
    "lu",
    "qr",
    "cholesky",
    "svd",
    "pinv",
    "rank",
    "det",
    "inverse",
    "eigh",
    "leastSquares",
];
pub(crate) const STATS: &[&str] = &["mean", "variance", "stddev", "covariance", "correlation"];
fn function(target: &str, inputs: &[(&str, Type)], result: Type) -> FunctionType {
    let Type::Function(mut f) =
        Type::function(inputs.iter().map(|(_, t)| t.clone()).collect(), result)
    else {
        unreachable!()
    };
    for (parameter, (name, _)) in f.parameters.iter_mut().zip(inputs) {
        parameter.name = Some((*name).into());
    }
    f.callable = Some(Callable::Builtin(target.into()));
    f
}
fn add(env: &mut Environment, module: &str, name: &str, inputs: &[(&str, Type)], result: Type) {
    let target = format!("{module}.{name}");
    let f = function(&target, inputs, result);
    env.values.insert(
        name.into(),
        Symbol {
            ty: Type::Function(f),
            quantified: BTreeSet::new(),
            mutable: false,
            visibility: Visibility::Public,
            access: None,
            origin: None,
            reference: Some(Reference::Builtin(target)),
        },
    );
}
fn class(
    env: &mut Environment,
    module: &str,
    name: &str,
    parameters: &[&str],
    fields: &[(&str, Type)],
    mutable: &[&str],
) {
    let full = format!("{module}.{name}");
    env.names.insert(name.into(), full.clone());
    env.definitions.insert(
        full,
        Definition {
            kind: K::Class,
            sealed: true,
            record: false,
            parameters: parameters.iter().map(|s| (*s).into()).collect(),
            associated_types: BTreeSet::new(),
            bounds: BTreeMap::new(),
            fields: fields
                .iter()
                .map(|(n, t)| ((*n).into(), t.clone()))
                .collect(),
            constructor_fields: BTreeSet::new(),
            static_fields: BTreeMap::new(),
            static_vars: BTreeSet::new(),
            mutable_fields: mutable.iter().map(|s| (*s).into()).collect(),
            initialized_fields: fields.iter().map(|(n, _)| (*n).into()).collect(),
            invariants: vec![],
            methods: BTreeMap::new(),
            default_methods: BTreeSet::new(),
            self_results: BTreeSet::new(),
            static_methods: BTreeSet::new(),
            parent: None,
            traits: vec![],
            constructors: vec![],
            variants: vec![],
            alias: None,
            visibility: Visibility::Public,
            member_visibility: fields
                .iter()
                .map(|(n, _)| {
                    (
                        (*n).into(),
                        if *n == "state" {
                            Visibility::Private
                        } else {
                            Visibility::Public
                        },
                    )
                })
                .collect(),
            file: format!("<{module}>"),
        },
    );
}
fn method(
    env: &mut Environment,
    full: &str,
    name: &str,
    inputs: &[(&str, Type)],
    result: Type,
    static_: bool,
    mutates: bool,
) {
    let full = full.to_owned();
    let mut f = function(&format!("{full}.{name}"), inputs, result);
    f.receiver_mutates = mutates;
    if !static_ {
        f.receiver = Some(Box::new(Type::named(&full)));
    }
    if name == "sample" {
        f.parameters[0].passing = Passing::Mut;
    }
    let definition = env.definitions.get_mut(&full).unwrap();
    definition.methods.entry(name.into()).or_default().push(f);
    definition
        .member_visibility
        .insert(name.into(), Visibility::Public);
    if static_ {
        definition.static_methods.insert(name.into());
    }
}
fn probability_parameter((name, kind): &(&'static str, Parameter)) -> (&'static str, Type) {
    let ty = match kind {
        Parameter::Double => Type::named("double"),
        Parameter::Int => Type::named("Int"),
    };
    (name, ty)
}

fn probability_class(env: &mut Environment, distribution: &Distribution) {
    let module = "std.prob";
    let class_path = format!("std.prob.{}", distribution.name);
    let fields: Vec<(&str, Type)> = distribution
        .parameters
        .iter()
        .map(probability_parameter)
        .collect();
    class(env, module, distribution.name, &[], &fields, &[]);

    let value_type = Type::named(&class_path);
    let create_inputs: Vec<(&str, Type)> = fields.clone();
    method(
        env,
        &class_path,
        "create",
        &create_inputs,
        value_type.clone(),
        true,
        false,
    );

    let observation = if distribution.discrete {
        ("k", Type::named("Int"))
    } else {
        ("x", Type::named("double"))
    };
    let operations = [
        distribution.probability_name(),
        distribution.log_probability_name(),
        "cdf",
    ];
    for operation in operations {
        method(
            env,
            &class_path,
            operation,
            std::slice::from_ref(&observation),
            Type::named("double"),
            false,
            false,
        );
    }
    method(
        env,
        &class_path,
        "sample",
        &[("rng", Type::named("std.random.Rng"))],
        distribution.sample_result(),
        false,
        false,
    );

    for operation in operations {
        let mut inputs = vec![observation.clone()];
        inputs.extend(fields.iter().cloned());
        method(
            env,
            &class_path,
            operation,
            &inputs,
            Type::named("double"),
            true,
            false,
        );
    }
}

pub(crate) fn environment(module: &str) -> Option<Environment> {
    let mut env = Environment::default();
    let tensor = crate::types::tensor_type(None);
    let double = Type::named("double");
    let int = Type::named("Int");
    match module {
        "std.linalg" => {
            for (name, params, fields) in [
                (
                    "LU",
                    vec!["F", "P"],
                    vec![
                        ("factors", Type::Parameter("F".into())),
                        ("pivots", Type::Parameter("P".into())),
                    ],
                ),
                (
                    "QR",
                    vec!["Q", "R"],
                    vec![
                        ("q", Type::Parameter("Q".into())),
                        ("r", Type::Parameter("R".into())),
                    ],
                ),
                (
                    "SVD",
                    vec!["U", "S", "V"],
                    vec![
                        ("u", Type::Parameter("U".into())),
                        ("s", Type::Parameter("S".into())),
                        ("vt", Type::Parameter("V".into())),
                    ],
                ),
                (
                    "Eigh",
                    vec!["W", "V"],
                    vec![
                        ("values", Type::Parameter("W".into())),
                        ("vectors", Type::Parameter("V".into())),
                    ],
                ),
            ] {
                // Tensor-valued fields must be var; Kestrel forbids final fields of mutable types.
                class(
                    &mut env,
                    module,
                    name,
                    &params,
                    &fields,
                    &fields.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
                );
            }
            method(
                &mut env,
                "std.linalg.LU",
                "solve",
                &[("b", tensor.clone())],
                tensor.clone(),
                false,
                false,
            );
            for name in LINALG {
                let inputs = if matches!(
                    *name,
                    "dot" | "outer" | "solve" | "solveLower" | "solveUpper" | "leastSquares"
                ) {
                    vec![("a", tensor.clone()), ("b", tensor.clone())]
                } else {
                    vec![("a", tensor.clone())]
                };
                let result = match *name {
                    "dot" | "norm" | "frobeniusNorm" | "spectralNorm" | "det" => double.clone(),
                    "rank" => int.clone(),
                    "lu" => Type::applied(
                        "std.linalg.LU",
                        vec![tensor.clone(), Type::applied("Array", vec![int.clone()])],
                    ),
                    "qr" => Type::applied("std.linalg.QR", vec![tensor.clone(); 2]),
                    "svd" => Type::applied("std.linalg.SVD", vec![tensor.clone(); 3]),
                    "eigh" => Type::applied("std.linalg.Eigh", vec![tensor.clone(); 2]),
                    _ => tensor.clone(),
                };
                add(&mut env, module, name, &inputs, result);
            }
        }
        "std.stats" => {
            for name in STATS {
                add(
                    &mut env,
                    module,
                    name,
                    &[("source", tensor.clone())],
                    if matches!(*name, "covariance" | "correlation") {
                        tensor.clone()
                    } else {
                        double.clone()
                    },
                );
            }
        }
        "std.mlrt" => {
            add(
                &mut env,
                module,
                "knnPredict",
                &[
                    ("training", tensor.clone()),
                    ("labels", tensor.clone()),
                    ("test", tensor.clone()),
                    ("neighbors", int.clone()),
                ],
                tensor.clone(),
            );
        }
        "std.random" => {
            class(
                &mut env,
                module,
                "Rng",
                &[],
                &[("state", int.clone())],
                &["state"],
            );
            let rng = Type::named("std.random.Rng");
            method(
                &mut env,
                "std.random.Rng",
                "seeded",
                &[("seed", int.clone())],
                rng,
                true,
                false,
            );
            method(
                &mut env,
                "std.random.Rng",
                "uniform",
                &[("low", double.clone()), ("high", double.clone())],
                double.clone(),
                false,
                true,
            );
            method(
                &mut env,
                "std.random.Rng",
                "normal",
                &[("mean", double.clone()), ("sigma", double.clone())],
                double.clone(),
                false,
                true,
            );
            method(
                &mut env,
                "std.random.Rng",
                "integers",
                &[("low", int.clone()), ("high", int.clone())],
                int.clone(),
                false,
                true,
            );
            let mut seed_function =
                function("std.random.seed", &[("seed", int)], Type::named("void"));
            seed_function.effects = true;
            env.values.insert(
                "seed".into(),
                Symbol {
                    ty: Type::Function(seed_function),
                    quantified: BTreeSet::new(),
                    mutable: false,
                    visibility: Visibility::Public,
                    access: None,
                    origin: None,
                    reference: Some(Reference::Builtin("std.random.seed".into())),
                },
            );
        }
        "std.prob" => {
            // `sample`'s mutable argument needs the canonical Rng definition even
            // when the caller imports only a distribution.
            env.definitions
                .extend(environment("std.random").unwrap().definitions);
            for distribution in crate::probability::DISTRIBUTIONS {
                probability_class(&mut env, distribution);
            }
        }
        _ => return None,
    }
    Some(env)
}
