impl Emitter<'_> {
    fn numerical_rhs(
        &self,
        operation: &str,
        atoms: &[Atom],
        result: Ty,
        at: String,
    ) -> Result<String, String> {
        let args = atoms.iter().map(|a| self.atom(a)).collect::<Vec<_>>();
        let tensor = |i: usize| format!("&({}value)", self.through(&atoms[i]));
        let field = |i: usize, name: &str| -> Result<String, String> {
            let id = match self.atom_type(&atoms[i]) {
                Ty::Record(id) | Ty::Ref(id) | Ty::Boxed(id) => id,
                _ => return Err("Numerical object needs a record".into()),
            };
            let position = self.program.layouts[id]
                .fields
                .iter()
                .position(|f| f.name == name)
                .ok_or_else(|| format!("Missing numerical field {name}"))?;
            Ok(format!("{}f{position}", self.through(&atoms[i])))
        };
        let wrap = |call: String| -> Result<String, String> {
            let Ty::Record(id) = result else {
                return Err("Numerical result needs a tensor layout".into());
            };
            Ok(format!("(KRecord_{id}){{{call}}}"))
        };
        let aggregate = |parts: &[String]| -> Result<String, String> {
            let Ty::Record(id) = result else {
                return Err("Numerical result needs a record".into());
            };
            Ok(format!("(KRecord_{id}){{{}}}", parts.join(",")))
        };
        if operation == "mlrt.knnPredict" {
            return wrap(format!(
                "k_ml_knn_predict({}, {}, {}, {}, {at})",
                tensor(0),
                tensor(1),
                tensor(2),
                args[3]
            ));
        }
        if let Some(name) = operation.strip_prefix("math.") {
            let code = crate::numerical::UNARY
                .iter()
                .position(|n| *n == name)
                .ok_or("Unknown numerical unary operation")?;
            return if result == Ty::Double {
                Ok(format!(
                    "{}({})",
                    if name == "abs" { "fabs" } else { name },
                    args[0]
                ))
            } else {
                wrap(format!("k_math_tensor({}, {code}, {at})", tensor(0)))
            };
        }
        if let Some(name) = operation.strip_prefix("stats.") {
            let code = match name {
                "mean" => 0,
                "variance" => 1,
                "stddev" => 2,
                "covariance" => 3,
                "correlation" => 4,
                _ => return Err("Unknown statistic".into()),
            };
            if code >= 3 {
                return wrap(format!(
                    "k_stats_covariance({}, {}, {}, {at})",
                    tensor(0),
                    args[2],
                    code == 4
                ));
            }
            let call = format!(
                "k_stats_reduce({}, {code}, {}, {}, {}, {at})",
                tensor(0),
                args[1],
                args[2],
                args[3]
            );
            return if result == Ty::Double {
                Ok(format!("k_stats_scalar({call})"))
            } else {
                wrap(call)
            };
        }
        if let Some(name) = operation.strip_prefix("linalg.") {
            let call = match name {
                "norm" | "frobeniusNorm" => return Ok(format!("k_tensor_norm({})", tensor(0))),
                "dot" => return Ok(format!("k_linalg_dot({}, {}, {at})", tensor(0), tensor(1))),
                "outer" => format!("k_linalg_outer({}, {}, {at})", tensor(0), tensor(1)),
                "solve" | "solveLower" | "solveUpper" => format!(
                    "k_linalg_solve({}, {}, {}, {at})",
                    tensor(0),
                    tensor(1),
                    match name {
                        "solveLower" => 1,
                        "solveUpper" => 2,
                        _ => 0,
                    }
                ),
                "factorSolve" => format!(
                    "k_linalg_factor_solve_array(&({}.value), {}.length, {}.items, {}, {at})",
                    field(0, "factors")?,
                    field(0, "pivots")?,
                    field(0, "pivots")?,
                    tensor(1)
                ),
                "inverse" => format!("k_linalg_inverse({}, {at})", tensor(0)),
                "cholesky" => format!("k_linalg_cholesky({}, {at})", tensor(0)),
                "det" => return Ok(format!("k_linalg_det({}, {at})", tensor(0))),
                "spectralNorm" => {
                    return Ok(format!("k_linalg_spectral_norm({}, {at})", tensor(0)));
                }
                "rank" => {
                    return Ok(format!(
                        "k_linalg_rank({}, {}, {}, {at})",
                        tensor(0),
                        args[1],
                        args[2]
                    ));
                }
                "pinv" => format!(
                    "k_linalg_pinv({}, {}, {}, {at})",
                    tensor(0),
                    args[1],
                    args[2]
                ),
                "leastSquares" => format!(
                    "k_linalg_least_squares({}, {}, {}, {}, {at})",
                    tensor(0),
                    tensor(1),
                    args[2],
                    args[3]
                ),
                "lu" | "qr" | "svd" | "eigh" => {
                    let Ty::Record(id) = result else {
                        return Err("Factorization needs a record".into());
                    };
                    let mut fields = vec![];
                    for f in &self.program.layouts[id].fields {
                        let Ty::Record(tensor_id) = f.ty else {
                            return Err("Factorization field needs a tensor".into());
                        };
                        if name == "lu" && f.name == "pivots" {
                            fields.push(format!("(KRecord_{tensor_id}){{.length=pivot_count_,.items=pivots_,.capacity=pivot_count_}}"));
                            continue;
                        }
                        let part = match f.name.as_str() {
                            "factors" | "q" | "u" | "values" => "first",
                            "r" | "s" | "vectors" => "second",
                            "vt" => "third",
                            _ => return Err("Unknown factorization field".into()),
                        };
                        fields.push(format!("(KRecord_{tensor_id}){{parts_.{part}}}"));
                    }
                    let pivots = if name == "lu" {
                        "int64_t pivot_count_=parts_.second.length; int64_t *pivots_=k_numerical_alloc((size_t)pivot_count_,sizeof(int64_t),".to_owned()+&at+"); for(int64_t i_=0;i_<pivot_count_;++i_)pivots_[i_]=(int64_t)parts_.second.items[i_]; k_tensor_drop(&parts_.second);"
                    } else {
                        String::new()
                    };
                    return Ok(format!(
                        "({{ KNumericalParts parts_ = k_linalg_{name}({}, {at}); {pivots} {}; }})",
                        tensor(0),
                        aggregate(&fields)?
                    ));
                }
                _ => return Err(format!("Unknown linear algebra operation {name}")),
            };
            return wrap(call);
        }
        if operation == "random.seed" {
            return Ok(format!("(k_rng_seed_default({}), (void)0)", args[0]));
        }
        if operation == "random.Rng.seeded" {
            return aggregate(&[format!("k_rng_seed({})", args[0])]);
        }
        if let Some(name) = operation.strip_prefix("random.Rng.") {
            return Ok(format!(
                "k_rng_{name}(&({}), {}, {}, {at})",
                field(0, "state")?,
                args[1],
                args[2]
            ));
        }
        if let Some(rest) = operation.strip_prefix("prob.") {
            let (class, mode) = rest
                .split_once('.')
                .ok_or_else(|| "Probability target has no method".to_owned())?;
            let distribution = crate::probability::find(class)
                .ok_or_else(|| format!("Unknown probability distribution {class}"))?;
            let kind = distribution.kind;
            let count = distribution.parameters.len();
            let cast = |index: usize| -> String {
                let expression = &args[index];
                if matches!(self.atom_type(&atoms[index]), Ty::Int | Ty::Nat) {
                    format!("(double)({expression})")
                } else {
                    expression.clone()
                }
            };
            let field_double = |index: usize, name: &str, parameter: usize| -> Result<String, String> {
                let expression = field(index, name)?;
                Ok(if matches!(
                    distribution.parameters[parameter].1,
                    crate::probability::Parameter::Int
                ) {
                    format!("(double)({expression})")
                } else {
                    expression
                })
            };
            let mut parameters = vec!["0.0".to_owned(); 3];
            if mode == "create" {
                let Ty::Record(id) = result else {
                    return Err("Distribution needs a record".into());
                };
                for (index, slot) in (0..count).zip(parameters.iter_mut()) {
                    *slot = cast(index);
                }
                let parts = self.program.layouts[id]
                    .fields
                    .iter()
                    .map(|field| {
                        distribution
                            .parameters
                            .iter()
                            .position(|(name, _)| *name == field.name)
                            .map(|index| args[index].clone())
                            .ok_or_else(|| format!("Missing probability field {}", field.name))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                return Ok(format!(
                    "({{ k_prob_validate({kind}, {}, {}, {}, {at}); {}; }})",
                    parameters[0],
                    parameters[1],
                    parameters[2],
                    aggregate(&parts)?
                ));
            }
            if matches!(mode, "densityStatic" | "logStatic" | "cdfStatic") {
                for (index, _) in distribution.parameters.iter().enumerate() {
                    parameters[index] = cast(index + 1);
                }
                let x = cast(0);
                let call = match mode {
                    "densityStatic" => "k_prob_pdf",
                    "logStatic" => "k_prob_logpdf",
                    _ => "k_prob_cdf",
                };
                return Ok(format!(
                    "{call}({kind}, {}, {}, {}, {x}, {at})",
                    parameters[0], parameters[1], parameters[2]
                ));
            }
            if matches!(mode, "densityInstance" | "logInstance" | "cdfInstance") {
                let mut parameter_fields = vec!["0.0".to_owned(); 3];
                for (index, (name, _)) in distribution.parameters.iter().enumerate() {
                    parameter_fields[index] = field_double(0, name, index)?;
                }
                let x = cast(1);
                let call = match mode {
                    "densityInstance" => "k_prob_pdf",
                    "logInstance" => "k_prob_logpdf",
                    _ => "k_prob_cdf",
                };
                return Ok(format!(
                    "{call}({kind}, {}, {}, {}, {x}, {at})",
                    parameter_fields[0], parameter_fields[1], parameter_fields[2]
                ));
            }
            if mode == "sampleInstance" {
                let mut parameter_fields = vec!["0.0".to_owned(); 3];
                for (index, (name, _)) in distribution.parameters.iter().enumerate() {
                    parameter_fields[index] = field_double(0, name, index)?;
                }
                let call = format!(
                    "k_prob_sample({kind}, {}, {}, {}, &({}), {at})",
                    parameter_fields[0],
                    parameter_fields[1],
                    parameter_fields[2],
                    field(1, "state")?
                );
                return Ok(if result == Ty::Int {
                    format!("(int64_t)({call})")
                } else {
                    call
                });
            }
            if mode.starts_with("sampleStatic") {
                let has_rng = mode.contains("Rng");
                let has_shape = mode.contains("Tensor");
                let rng_index = count;
                let shape_index = count + usize::from(has_rng);
                let rng = if has_rng {
                    format!("&({})", field(rng_index, "state")?)
                } else {
                    "NULL".to_owned()
                };
                let mut parameters = vec!["0.0".to_owned(); 3];
                for (index, slot) in (0..count).zip(parameters.iter_mut()) {
                    *slot = cast(index);
                }
                if !has_shape {
                    let call = format!(
                        "k_prob_sample({kind}, {}, {}, {}, {rng}, {at})",
                        parameters[0], parameters[1], parameters[2]
                    );
                    return Ok(if result == Ty::Int {
                        format!("(int64_t)({call})")
                    } else {
                        call
                    });
                }
                let shape_atom = atoms
                    .get(shape_index)
                    .ok_or_else(|| "Probability sample has no shape".to_owned())?;
                let (setup, rank, dims, cleanup) = match self.atom_type(shape_atom) {
                    Ty::Int | Ty::Nat => {
                        let expression = &args[shape_index];
                        (
                            format!("int64_t dims_[1] = {{ (int64_t)({expression}) }};"),
                            "1".to_owned(),
                            "dims_".to_owned(),
                            String::new(),
                        )
                    }
                    Ty::Record(id) | Ty::Ref(id) | Ty::Boxed(id) => {
                        match &self.program.layouts[id].kind {
                            LayoutKind::Array(Ty::Int) => {
                                let shape = self.through(shape_atom);
                                (
                                    String::new(),
                                    format!("(int64_t)({shape}length)"),
                                    format!("{shape}items"),
                                    String::new(),
                                )
                            }
                            LayoutKind::List(Ty::Int) => {
                                let shape = self.through(shape_atom);
                                (
                                    format!(
                                        "int64_t rank_ = {shape}handle ? {shape}handle->length : 0; int64_t *dims_ = k_numerical_alloc((size_t)rank_, sizeof(int64_t), {at}); for (int64_t i_ = 0; i_ < rank_; ++i_) dims_[i_] = {shape}handle->storage[i_].value;"
                                    ),
                                    "rank_".to_owned(),
                                    "dims_".to_owned(),
                                    "free(dims_);".to_owned(),
                                )
                            }
                            LayoutKind::Tuple => {
                                let fields = self.program.layouts[id].fields.clone();
                                if !fields.iter().all(|field| field.ty == Ty::Int) {
                                    return Err("Probability shape tuple needs Int fields".into());
                                }
                                let shape = self.through(shape_atom);
                                let expressions = (0..fields.len())
                                    .map(|index| format!("(int64_t)({shape}f{index})"))
                                    .collect::<Vec<_>>();
                                if expressions.is_empty() {
                                    (
                                        "int64_t *dims_ = NULL;".to_owned(),
                                        "0".to_owned(),
                                        "dims_".to_owned(),
                                        String::new(),
                                    )
                                } else {
                                    (
                                        format!(
                                            "int64_t dims_[{}] = {{{}}};",
                                            expressions.len(),
                                            expressions.join(", ")
                                        ),
                                        format!("{}", expressions.len()),
                                        "dims_".to_owned(),
                                        String::new(),
                                    )
                                }
                            }
                            _ => return Err("Unsupported probability shape".into()),
                        }
                    }
                    _ => return Err("Unsupported probability shape".into()),
                };
                let call = format!(
                    "k_prob_sample_tensor({kind}, {}, {}, {}, {rng}, {rank}, {dims}, {at})",
                    parameters[0], parameters[1], parameters[2]
                );
                return Ok(format!(
                    "({{ {setup} KTensor sampled_ = {call}; {cleanup} {}; }})",
                    wrap("sampled_".to_owned())?
                ));
            }
            return Err(format!("Unknown probability operation {mode}"));
        }
        Err(format!("Unknown numerical operation {operation}"))
    }
}
