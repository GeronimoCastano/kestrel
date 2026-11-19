//! Distribution metadata shared by the checker, core lowering and the editor.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Parameter {
    Double,
    Int,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Distribution {
    pub name: &'static str,
    pub kind: u32,
    pub parameters: &'static [(&'static str, Parameter)],
    pub discrete: bool,
}

pub(crate) const DISTRIBUTIONS: &[Distribution] = &[
    Distribution {
        name: "Normal",
        kind: 0,
        parameters: &[("mean", Parameter::Double), ("sigma", Parameter::Double)],
        discrete: false,
    },
    Distribution {
        name: "Uniform",
        kind: 1,
        parameters: &[("low", Parameter::Double), ("high", Parameter::Double)],
        discrete: false,
    },
    Distribution {
        name: "Exponential",
        kind: 2,
        parameters: &[("rate", Parameter::Double)],
        discrete: false,
    },
    Distribution {
        name: "Gamma",
        kind: 3,
        parameters: &[("alpha", Parameter::Double), ("beta", Parameter::Double)],
        discrete: false,
    },
    Distribution {
        name: "Beta",
        kind: 4,
        parameters: &[("alpha", Parameter::Double), ("beta", Parameter::Double)],
        discrete: false,
    },
    Distribution {
        name: "ChiSquared",
        kind: 5,
        parameters: &[("dof", Parameter::Double)],
        discrete: false,
    },
    Distribution {
        name: "StudentT",
        kind: 6,
        parameters: &[("dof", Parameter::Double)],
        discrete: false,
    },
    Distribution {
        name: "Cauchy",
        kind: 7,
        parameters: &[
            ("location", Parameter::Double),
            ("scale", Parameter::Double),
        ],
        discrete: false,
    },
    Distribution {
        name: "Laplace",
        kind: 8,
        parameters: &[
            ("location", Parameter::Double),
            ("scale", Parameter::Double),
        ],
        discrete: false,
    },
    Distribution {
        name: "Logistic",
        kind: 9,
        parameters: &[
            ("location", Parameter::Double),
            ("scale", Parameter::Double),
        ],
        discrete: false,
    },
    Distribution {
        name: "Weibull",
        kind: 10,
        parameters: &[("k", Parameter::Double), ("lambda", Parameter::Double)],
        discrete: false,
    },
    Distribution {
        name: "Lognormal",
        kind: 11,
        parameters: &[("mu", Parameter::Double), ("sigma", Parameter::Double)],
        discrete: false,
    },
    Distribution {
        name: "Pareto",
        kind: 12,
        parameters: &[("scale", Parameter::Double), ("alpha", Parameter::Double)],
        discrete: false,
    },
    Distribution {
        name: "Gumbel",
        kind: 13,
        parameters: &[
            ("location", Parameter::Double),
            ("scale", Parameter::Double),
        ],
        discrete: false,
    },
    Distribution {
        name: "Rayleigh",
        kind: 14,
        parameters: &[("sigma", Parameter::Double)],
        discrete: false,
    },
    Distribution {
        name: "Triangular",
        kind: 15,
        parameters: &[
            ("low", Parameter::Double),
            ("mode", Parameter::Double),
            ("high", Parameter::Double),
        ],
        discrete: false,
    },
    Distribution {
        name: "Bernoulli",
        kind: 16,
        parameters: &[("p", Parameter::Double)],
        discrete: true,
    },
    Distribution {
        name: "Binomial",
        kind: 17,
        parameters: &[("trials", Parameter::Int), ("p", Parameter::Double)],
        discrete: true,
    },
    Distribution {
        name: "Poisson",
        kind: 18,
        parameters: &[("lambda", Parameter::Double)],
        discrete: true,
    },
    Distribution {
        name: "Geometric",
        kind: 19,
        parameters: &[("p", Parameter::Double)],
        discrete: true,
    },
    Distribution {
        name: "NegativeBinomial",
        kind: 20,
        parameters: &[("successes", Parameter::Int), ("p", Parameter::Double)],
        discrete: true,
    },
    Distribution {
        name: "Hypergeometric",
        kind: 21,
        parameters: &[
            ("population", Parameter::Int),
            ("successes", Parameter::Int),
            ("draws", Parameter::Int),
        ],
        discrete: true,
    },
    Distribution {
        name: "DiscreteUniform",
        kind: 22,
        parameters: &[("low", Parameter::Int), ("high", Parameter::Int)],
        discrete: true,
    },
];

pub(crate) fn find(name: &str) -> Option<&'static Distribution> {
    DISTRIBUTIONS.iter().find(|d| d.name == name)
}

impl Distribution {
    pub fn class_path(&self) -> String {
        format!("std.prob.{}", self.name)
    }

    pub fn probability_name(&self) -> &'static str {
        if self.discrete { "pmf" } else { "pdf" }
    }

    pub fn log_probability_name(&self) -> &'static str {
        if self.discrete { "logPmf" } else { "logPdf" }
    }

    pub fn sample_result(&self) -> crate::types::Type {
        if self.discrete {
            crate::types::Type::named("Int")
        } else {
            crate::types::Type::named("double")
        }
    }
}
