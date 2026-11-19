// Native lowering for KesNet Variables. The C engine lives in
// `autograd_runtime.c`; this file only turns checked method calls into calls
// to its small, stable API.

impl Emitter<'_> {
    fn variable_pointer(&self, atom: &Atom) -> String {
        self.atom(atom)
    }

    fn variable_record(&self, atom: &Atom) -> Result<(usize, String), String> {
        let ty = self.atom_type(atom);
        match ty {
            Ty::Record(id) => Ok((id, self.atom(atom))),
            Ty::Ref(id) => Ok((id, format!("*({})", self.atom(atom)))),
            _ => Err("A Variable argument needs a record value".into()),
        }
    }

    fn variable_shape(&self, atom: &Atom, out: &mut String) -> Result<(String, String), String> {
        let (id, expression) = self.variable_record(atom)?;
        match &self.program.layouts[id].kind {
            LayoutKind::Tuple => {
                let fields = &self.program.layouts[id].fields;
                if fields.iter().any(|field| field.ty != Ty::Int) {
                    return Err("A Variable shape tuple must contain Int dimensions".into());
                }
                out.push_str(&format!("KRecord_{id} shape_ = {expression};"));
                let rank = fields.len().to_string();
                let dims = format!(
                    "(int64_t[]){{{}}}",
                    (0..fields.len())
                        .map(|i| format!("shape_.f{i}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                Ok((rank, dims))
            }
            LayoutKind::Array(Ty::Int) => {
                out.push_str(&format!("KRecord_{id} shape_ = {expression};"));
                Ok(("(int64_t)shape_.length".into(), "shape_.items".into()))
            }
            _ => Err("A Variable shape must be a tuple or Array[Int]".into()),
        }
    }

    fn variable_axis(&self, atom: &Atom) -> String {
        self.atom(atom)
    }

    /// The shape-argument prologue ends with no closing brace; callers use a
    /// statement expression so temporaries stay valid for the whole call.
    fn variable_shape_call(
        &self,
        atom: &Atom,
        call: impl FnOnce(&str, &str) -> String,
    ) -> Result<String, String> {
        let mut prologue = String::new();
        let (rank, dims) = self.variable_shape(atom, &mut prologue)?;
        Ok(format!(
            "({{ {prologue} {}; }})",
            call(&rank, &dims)
        ))
    }

    fn variable_rhs(
        &self,
        operation: &str,
        atoms: &[Atom],
        ty: Ty,
        at: String,
    ) -> Result<String, String> {
        let pointer = |index: usize| -> Result<String, String> {
            atoms
                .get(index)
                .map(|atom| self.variable_pointer(atom))
                .ok_or_else(|| format!("Variable operation {operation} is missing an argument"))
        };
        let atom_type = |index: usize| -> Ty {
            atoms.get(index).map_or(Ty::Unit, |atom| self.atom_type(atom))
        };
        let as_double = |index: usize| -> Result<String, String> {
            atoms
                .get(index)
                .map(|atom| self.atom(atom))
                .ok_or_else(|| format!("Variable operation {operation} is missing a scalar"))
        };
        match operation {
            "zeros" | "ones" | "randn" | "rand" | "filled" => {
                let fill = if operation == "ones" {
                    "1.0".to_owned()
                } else if operation == "filled" {
                    as_double(1)?
                } else {
                    "0.0".to_owned()
                };
                let call = match operation {
                    "randn" => self.variable_shape_call(&atoms[0], |rank, dims| {
                        format!("k_ad_randn({rank}, {dims}, false, K_AD_F64, {at})")
                    })?,
                    "rand" => self.variable_shape_call(&atoms[0], |rank, dims| {
                        format!("k_ad_uniform({rank}, {dims}, 0.0, 1.0, false, K_AD_F64, {at})")
                    })?,
                    _ => self.variable_shape_call(&atoms[0], |rank, dims| {
                        format!("k_ad_zeros({rank}, {dims}, {fill}, false, K_AD_F64, {at})")
                    })?,
                };
                Ok(call)
            }
            "fromArray" | "parameter" => {
                let (id, values) = self.variable_record(&atoms[0])?;
                if !matches!(self.program.layouts[id].kind, LayoutKind::Array(Ty::Double)) {
                    return Err("Variable.fromArray needs Array[double]".into());
                }
                if atoms.len() == 1 {
                    Ok(format!(
                        "({{ KRecord_{id} values_ = {values}; k_ad_from_values(values_.length, values_.items, 1, (int64_t[]){{values_.length}}, {}, K_AD_F64, {at}); }})",
                        if operation == "parameter" { "true" } else { "false" }
                    ))
                } else {
                    let mut prologue = String::new();
                    let (rank, dims) = self.variable_shape(&atoms[1], &mut prologue)?;
                    Ok(format!(
                        "({{ KRecord_{id} values_ = {values}; {prologue} k_ad_from_values(values_.length, values_.items, {rank}, {dims}, {}, K_AD_F64, {at}); }})",
                        if operation == "parameter" { "true" } else { "false" }
                    ))
                }
            }
            "fromTensor" => {
                let (id, expression) = self.variable_record(&atoms[0])?;
                if !matches!(self.program.layouts[id].kind, LayoutKind::Tensor) {
                    return Err("Variable.fromTensor needs a DynTensor".into());
                }
                Ok(format!(
                    "k_ad_from_tensor(&({}).value, false, K_AD_F64, {at})",
                    expression
                ))
            }
            "scalar" => Ok(format!(
                "({{ int64_t shape_[1] = {{0}}; k_ad_zeros(0, shape_, {}, false, K_AD_F64, {at}); }})",
                as_double(0)?
            )),
            "arange" => {
                let step = if atoms.len() > 2 { as_double(2)? } else { "1.0".into() };
                Ok(format!(
                    "k_ad_arange({}, {}, {step}, false, K_AD_F64, {at})",
                    as_double(0)?,
                    as_double(1)?
                ))
            }
            "linspace" => Ok(format!(
                "k_ad_linspace({}, {}, {}, false, K_AD_F64, {at})",
                as_double(0)?,
                as_double(1)?,
                as_double(2)?
            )),
            "randint" => {
                let low = self.atom(&atoms[0]);
                let high = self.atom(&atoms[1]);
                self.variable_shape_call(&atoms[2], |rank, dims| {
                    format!("k_ad_randint({rank}, {dims}, {low}, {high}, {at})")
                })
            }
            "eye" => Ok(format!(
                "k_ad_eye({}, false, {at})",
                self.atom(&atoms[0])
            )),
            "normal" => {
                let mean = as_double(1)?;
                let sigma = as_double(2)?;
                self.variable_shape_call(&atoms[0], |rank, dims| {
                    format!("k_ad_normal_tensor({rank}, {dims}, {mean}, {sigma}, false, {at})")
                })
            }
            "bernoulli" => {
                let probability = as_double(1)?;
                self.variable_shape_call(&atoms[0], |rank, dims| {
                    format!("k_ad_bernoulli({rank}, {dims}, {probability}, false, {at})")
                })
            }
            "empty" => Ok("k_variable_empty()".into()),
            "embedding" => Ok(format!(
                "k_ad_embedding({}, {}, {at})",
                pointer(0)?,
                pointer(1)?
            )),
            "setTraining" => Ok(format!("k_ad_set_training({})", self.atom(&atoms[0]))),
            "isTraining" => Ok("k_ad_is_training()".into()),
            "setGradEnabled" => Ok(format!("k_ad_set_grad_enabled({})", self.atom(&atoms[0]))),
            "isGradEnabled" => Ok("k_ad_get_grad_enabled()".into()),
            "retainGrad" => Ok(format!("k_ad_retain_grad({}, {at})", pointer(0)?)),
            "load" => Ok(format!("k_ad_load({}, {at})", self.atom(&atoms[0]))),
            "save" => Ok(format!("k_ad_save({}, {}, {at})", pointer(0)?, self.atom(&atoms[1]))),
            "copyFromFile" => Ok(format!(
                "({{ KVariable loaded_ = k_ad_load({}, {at}); k_ad_copy_inplace({}, loaded_, {at}); }})",
                self.atom(&atoms[1]),
                pointer(0)?
            )),
            "liveNodes" => Ok("k_ad_live_nodes_count()".into()),
            "liveBytes" => Ok("k_ad_live_bytes_count()".into()),
            "uniform" => {
                let low = as_double(1)?;
                let high = as_double(2)?;
                self.variable_shape_call(&atoms[0], |rank, dims| {
                    format!("k_ad_uniform({rank}, {dims}, {low}, {high}, false, K_AD_F64, {at})")
                })
            }
            "zerosLike" | "onesLike" | "randLike" => {
                let fill = if operation == "onesLike" { "1.0" } else { "0.0" };
                Ok(format!(
                    "({{ KVariable source_ = {}; k_ad_zeros(source_.node->rank, source_.node->shape, {fill}, false, source_.node->dtype, {at}); }})",
                    pointer(0)?
                ))
            }
            "seed" => Ok(format!("k_ad_seed({})", self.atom(&atoms[0]))),
            "index" => {
                let count = atoms.len() - 1;
                let mut dims = vec![];
                for atom in atoms.iter().skip(1) {
                    dims.push(self.atom(atom));
                }
                Ok(format!(
                    "k_ad_index({}, {}, (int64_t[]){{{}}}, {at})",
                    pointer(0)?,
                    count,
                    dims.join(", ")
                ))
            }
            "add" | "subtract" | "multiply" | "divide" | "rightAdd" | "rightSubtract"
            | "rightMultiply" | "rightDivide" => {
                let kind = match operation {
                    "add" | "rightAdd" => 0,
                    "subtract" | "rightSubtract" => 1,
                    "multiply" | "rightMultiply" => 2,
                    _ => 3,
                };
                let reverse = operation.starts_with("right");
                let other = atom_type(1);
                if other == Ty::Variable {
                    if reverse {
                        Ok(format!(
                            "k_ad_binary({}, {}, {kind}, {at})",
                            pointer(1)?,
                            pointer(0)?
                        ))
                    } else {
                        Ok(format!(
                            "k_ad_binary({}, {}, {kind}, {at})",
                            pointer(0)?,
                            pointer(1)?
                        ))
                    }
                } else if reverse {
                    Ok(format!(
                        "k_ad_scalar_binary({}, {}, {kind}, {at})",
                        as_double(0)?,
                        pointer(1)?
                    ))
                } else {
                    Ok(format!(
                        "k_ad_binary_scalar({}, {}, {kind}, {at})",
                        pointer(0)?,
                        as_double(1)?
                    ))
                }
            }
            "lstmForward" => Ok(format!(
                "k_ad_lstm_sequence({}, {}, {}, {}, {}, {at})",
                pointer(0)?, pointer(1)?, pointer(2)?, pointer(3)?, pointer(4)?
            )),
            "lstmCell" => Ok(format!(
                "k_ad_lstm_cell({}, {}, {at})",
                pointer(0)?,
                pointer(1)?
            )),
            "lstmHidden" => Ok(format!(
                "k_ad_lstm_hidden({}, {}, {at})",
                pointer(0)?,
                pointer(1)?
            )),
            "matmul" | "mm" => Ok(format!(
                "k_ad_matmul({}, {}, {at})",
                pointer(0)?,
                pointer(1)?
            )),
            "adamStep" => Ok(format!(
                "k_ad_adam_step({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {at})",
                pointer(0)?, pointer(1)?, pointer(2)?, pointer(3)?,
                as_double(4)?, as_double(5)?, as_double(6)?, as_double(7)?,
                as_double(8)?, as_double(9)?, as_double(10)?
            )),
            "dot" => Ok(format!(
                "k_ad_dot({}, {}, {at})",
                pointer(0)?,
                pointer(1)?
            )),
            "slice" => Ok(format!(
                "k_ad_slice({}, {}, {}, {}, {at})",
                pointer(0)?,
                self.atom(&atoms[1]),
                self.atom(&atoms[2]),
                self.atom(&atoms[3])
            )),
            "cat" => Ok(format!(
                "k_ad_cat({}, {}, {}, {at})",
                pointer(0)?,
                pointer(1)?,
                self.atom(&atoms[2])
            )),
            "bmm" => Ok(format!(
                "k_ad_bmm({}, {}, {at})",
                pointer(0)?,
                pointer(1)?
            )),
            "conv2d" => Ok(format!(
                "k_ad_conv2d({}, {}, {}, {}, {}, {}, {}, {at})",
                pointer(0)?,
                pointer(1)?,
                pointer(2)?,
                self.atom(&atoms[3]),
                self.atom(&atoms[4]),
                self.atom(&atoms[5]),
                self.atom(&atoms[6])
            )),
            "adaptiveAvgPool2d" => Ok(format!(
                "k_ad_adaptive_avg_pool2d({}, {}, {}, {at})",
                pointer(0)?,
                self.atom(&atoms[1]),
                self.atom(&atoms[2])
            )),
            "maxPool2d" | "avgPool2d" => Ok(format!(
                "k_ad_pool2d({}, {}, {}, {}, {}, {at})",
                pointer(0)?,
                self.atom(&atoms[1]),
                self.atom(&atoms[2]),
                self.atom(&atoms[3]),
                if operation == "maxPool2d" { "true" } else { "false" }
            )),
            "batchNorm" => Ok(format!(
                "k_ad_batch_norm({}, {}, {}, {}, {}, {}, {}, {}, {at})",
                pointer(0)?,
                pointer(1)?,
                pointer(2)?,
                pointer(3)?,
                pointer(4)?,
                self.atom(&atoms[5]),
                self.atom(&atoms[6]),
                self.atom(&atoms[7])
            )),
            "layerNorm" => Ok(format!(
                "k_ad_layer_norm({}, {}, {}, {}, {at})",
                pointer(0)?,
                pointer(1)?,
                pointer(2)?,
                self.atom(&atoms[3])
            )),
            "negate" => Ok(format!("k_ad_unary({}, 0, {at})", pointer(0)?)),
            "exp" => Ok(format!("k_ad_unary({}, 1, {at})", pointer(0)?)),
            "log" => Ok(format!("k_ad_unary({}, 2, {at})", pointer(0)?)),
            "sqrt" => Ok(format!("k_ad_unary({}, 3, {at})", pointer(0)?)),
            "tanh" => Ok(format!("k_ad_unary({}, 4, {at})", pointer(0)?)),
            "sigmoid" => Ok(format!("k_ad_unary({}, 5, {at})", pointer(0)?)),
            "relu" => Ok(format!("k_ad_unary({}, 6, {at})", pointer(0)?)),
            "abs" => Ok(format!("k_ad_unary({}, 7, {at})", pointer(0)?)),
            "sin" => Ok(format!("k_ad_unary({}, 8, {at})", pointer(0)?)),
            "cos" => Ok(format!("k_ad_unary({}, 9, {at})", pointer(0)?)),
            "tan" => Ok(format!("k_ad_unary({}, 10, {at})", pointer(0)?)),
            "asin" => Ok(format!("k_ad_unary({}, 11, {at})", pointer(0)?)),
            "acos" => Ok(format!("k_ad_unary({}, 12, {at})", pointer(0)?)),
            "atan" => Ok(format!("k_ad_unary({}, 13, {at})", pointer(0)?)),
            "sinh" => Ok(format!("k_ad_unary({}, 14, {at})", pointer(0)?)),
            "cosh" => Ok(format!("k_ad_unary({}, 15, {at})", pointer(0)?)),
            "log1p" => Ok(format!("k_ad_unary({}, 16, {at})", pointer(0)?)),
            "expm1" => Ok(format!("k_ad_unary({}, 17, {at})", pointer(0)?)),
            "rsqrt" => Ok(format!("k_ad_unary({}, 18, {at})", pointer(0)?)),
            "floor" => Ok(format!("k_ad_unary({}, 19, {at})", pointer(0)?)),
            "ceil" => Ok(format!("k_ad_unary({}, 20, {at})", pointer(0)?)),
            "round" => Ok(format!("k_ad_unary({}, 21, {at})", pointer(0)?)),
            "sign" => Ok(format!("k_ad_unary({}, 22, {at})", pointer(0)?)),
            "power" => {
                let exponent = atom_type(1);
                if exponent == Ty::Variable {
                    Ok(format!(
                        "k_ad_pow({}, {}, {at})",
                        pointer(0)?,
                        pointer(1)?
                    ))
                } else {
                    Ok(format!(
                        "k_ad_pow_scalar({}, {}, {at})",
                        pointer(0)?,
                        as_double(1)?
                    ))
                }
            }
            "sum" | "mean" | "prod" | "max" | "min" => {
                let kind = match operation {
                    "sum" => 0,
                    "mean" => 1,
                    "prod" => 2,
                    "max" => 3,
                    _ => 4,
                };
                let axis = if atoms.len() > 1 { self.variable_axis(&atoms[1]) } else { "-1000000".into() };
                let keepdim = if atoms.len() > 2 { self.atom(&atoms[2]) } else { "false".into() };
                Ok(format!(
                    "k_ad_reduce({}, {kind}, {axis}, {keepdim}, {at})",
                    pointer(0)?
                ))
            }
            "argmax" | "argmin" => {
                let axis = if atoms.len() > 1 { self.variable_axis(&atoms[1]) } else { "-1000000".into() };
                let keepdim = if atoms.len() > 2 { self.atom(&atoms[2]) } else { "false".into() };
                Ok(format!(
                    "k_ad_arg_reduce({}, {}, {axis}, {keepdim}, {at})",
                    pointer(0)?,
                    if operation == "argmax" { "true" } else { "false" }
                ))
            }
            "reshape" => self.variable_shape_call(&atoms[1], |rank, dims| {
                format!("k_ad_reshape({}, {rank}, {dims}, {at})", pointer(0).unwrap())
            }),
            "broadcastTo" | "expand" => self.variable_shape_call(&atoms[1], |rank, dims| {
                format!("k_ad_broadcast_to({}, {rank}, {dims}, {at})", pointer(0).unwrap())
            }),
            "transpose" => Ok(format!(
                "k_ad_transpose({}, {}, {}, {at})",
                pointer(0)?,
                self.variable_axis(&atoms[1]),
                self.variable_axis(&atoms[2])
            )),
            "permute" => {
                // Axes arrive as a tuple or Array[Int].
                let mut prologue = String::new();
                let (rank, dims) = self.variable_shape(&atoms[1], &mut prologue)?;
                Ok(format!(
                    "({{ {prologue} k_ad_permute({}, {rank}, {dims}, {at}); }})",
                    pointer(0)?
                ))
            }
            "squeeze" => Ok(format!(
                "k_ad_squeeze({}, {}, {at})",
                pointer(0)?,
                if atoms.len() > 1 { self.atom(&atoms[1]) } else { "-1".into() }
            )),
            "unsqueeze" => Ok(format!(
                "k_ad_unsqueeze({}, {}, {at})",
                pointer(0)?,
                self.atom(&atoms[1])
            )),
            "flatten" => Ok(if atoms.len() > 1 {
                format!("k_ad_flatten_start({}, {}, {at})", pointer(0)?, self.atom(&atoms[1]))
            } else {
                format!("k_ad_flatten({}, {at})", pointer(0)?)
            }),
            "shape" => {
                let Ty::Record(id) = ty else {
                    return Err("Variable.shape needs Array[Int]".into());
                };
                Ok(format!(
                    "({{ int64_t length_ = k_ad_rank({}); KRecord_{id} result_ = {{length_, length_ ? malloc((size_t)length_ * sizeof(int64_t)) : NULL, length_}}; if (length_) k_ad_fill_shape({}, result_.items); result_; }})",
                    pointer(0)?,
                    pointer(0)?
                ))
            }
            "rank" => Ok(format!("k_ad_rank({})", pointer(0)?)),
            "count" => Ok(format!("k_ad_count({})", pointer(0)?)),
            "dtype" => Ok(format!("k_ad_dtype_text({})", pointer(0)?)),
            "device" => Ok(format!("k_ad_device_text({})", pointer(0)?)),
            "requiresGrad" => Ok(format!("k_ad_requires_grad({})", pointer(0)?)),
            "isLeaf" => Ok(format!("k_ad_is_leaf({})", pointer(0)?)),
            "item" => Ok(format!("k_ad_item({}, {at})", pointer(0)?)),
            "data" | "toTensor" => {
                let Ty::Record(id) = ty else {
                    return Err("Variable.data needs a DynTensor result".into());
                };
                Ok(format!(
                    "(KRecord_{id}){{k_ad_data({}, {at})}}",
                    pointer(0)?
                ))
            }
            "gradVariable" => Ok(format!("k_ad_grad_variable({}, {at})", pointer(0)?)),
            "leakyRelu" => Ok(format!("k_ad_leaky_relu({}, {}, {at})", pointer(0)?, as_double(1)?)),
            "elu" => Ok(format!("k_ad_elu({}, {}, {at})", pointer(0)?, as_double(1)?)),
            "gelu" => Ok(format!("k_ad_gelu({}, {at})", pointer(0)?)),
            "silu" => Ok(format!("k_ad_silu({}, {at})", pointer(0)?)),
            "softplus" => Ok(format!(
                "k_ad_softplus({}, {}, {}, {at})",
                pointer(0)?,
                as_double(1)?,
                as_double(2)?
            )),
            "clamp" => Ok(format!(
                "k_ad_clamp({}, {}, {}, {at})",
                pointer(0)?,
                as_double(1)?,
                as_double(2)?
            )),
            "norm" => Ok(format!("k_ad_norm({}, {at})", pointer(0)?)),
            "softmax" | "logSoftmax" => Ok(format!(
                "k_ad_softmax({}, {}, {}, {at})",
                pointer(0)?,
                if atoms.len() > 1 { self.atom(&atoms[1]) } else { "-1".into() },
                if operation == "logSoftmax" { "true" } else { "false" }
            )),
            "crossEntropy" => Ok(format!(
                "k_ad_cross_entropy({}, {}, {at})",
                pointer(0)?,
                pointer(1)?
            )),
            "nllLoss" => Ok(format!(
                "k_ad_nll_loss({}, {}, {at})",
                pointer(0)?,
                pointer(1)?
            )),
            "binaryCrossEntropy" => Ok(format!(
                "k_ad_binary_cross_entropy_with_logits({}, {}, {at})",
                pointer(0)?,
                pointer(1)?
            )),
            "grad" => {
                let Ty::Record(option) = ty else {
                    return Err("Variable.grad needs Option[DynTensor]".into());
                };
                let cases = self.program.layouts[option]
                    .kind
                    .cases()
                    .ok_or("Variable.grad needs an Option layout")?;
                let Ty::Record(tensor) = cases[0].fields[0] else {
                    return Err("Option payload is not a tensor".into());
                };
                Ok(format!(
                    "({{ KTensor grad_ = k_ad_grad({}, {at}); grad_.items ? k_aggregate_{option}_0((KRecord_{tensor}){{grad_}}) : k_aggregate_{option}_1(); }})",
                    pointer(0)?
                ))
            }
            "copy" => Ok(format!("k_variable_copy({})", self.atom(&atoms[0]))),
            "detach" => Ok(format!("k_ad_detach({})", pointer(0)?)),
            "clone" => Ok(format!("k_ad_clone({}, {at})", pointer(0)?)),
            "text" => Ok(format!("k_ad_text({}, {at})", pointer(0)?)),
            "debug" => Ok(format!("k_ad_debug({}, {at})", pointer(0)?)),
            "backward" => Ok(format!(
                "k_ad_backward({}, {}, {at})",
                pointer(0)?,
                if atoms.len() > 1 { self.atom(&atoms[1]) } else { "false".into() }
            )),
            "zeroGrad" => Ok(format!("k_ad_zero_grad({})", pointer(0)?)),
            "requiresGrad_" => Ok(format!(
                "({{ k_ad_set_requires_grad({}, {}, {at}); k_variable_copy({}); }})",
                pointer(0)?,
                self.atom(&atoms[1]),
                pointer(0)?
            )),
            "to" => Ok(format!(
                "k_ad_to({}, {}, {at})",
                pointer(0)?,
                self.atom(&atoms[1])
            )),
            "add_" | "sub_" | "mul_" | "div_" => {
                let kind = match operation {
                    "add_" => 0,
                    "sub_" => 1,
                    "mul_" => 2,
                    _ => 3,
                };
                let other = atom_type(1);
                if other == Ty::Variable {
                    Ok(format!(
                        "k_ad_inplace({}, {}, {kind}, {at})",
                        pointer(0)?,
                        pointer(1)?
                    ))
                } else {
                    Ok(format!(
                        "k_ad_inplace_scalar({}, {}, {kind}, {at})",
                        pointer(0)?,
                        as_double(1)?
                    ))
                }
            }
            "fill_" => Ok(format!(
                "k_ad_fill({}, {}, {at})",
                pointer(0)?,
                as_double(1)?
            )),
            "zero_" => Ok(format!("k_ad_fill({}, 0.0, {at})", pointer(0)?)),
            "copy_" => Ok(format!(
                "k_ad_copy_inplace({}, {}, {at})",
                pointer(0)?,
                pointer(1)?
            )),
            _ => Err(format!("Unknown native Variable operation {operation}")),
        }
    }
}
