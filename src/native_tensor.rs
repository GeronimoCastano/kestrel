impl Emitter<'_> {
    fn tensor_rhs(
        &self,
        operation: &str,
        atoms: &[Atom],
        result: Ty,
        at: String,
    ) -> Result<String, String> {
        let args = atoms.iter().map(|a| self.atom(a)).collect::<Vec<_>>();
        let array = |start: usize| {
            format!(
                "(int64_t[]){{{}}}",
                if start == args.len() {
                    "0".into()
                } else {
                    args[start..].join(",")
                }
            )
        };
        let pointer = |index: usize| format!("&({}value)", self.through(&atoms[index]));
        let wrap = |call: String| -> Result<String, String> {
            let Ty::Record(id) = result else {
                return Err("Tensor operation needs a record result".into());
            };
            Ok(format!("(KRecord_{id}){{{call}}}"))
        };
        if let Some(operation) = operation.strip_suffix("ArrayShape") {
            let shape = self.through(atoms.last().unwrap());
            let call = match operation {
                "new" => format!(
                    "k_tensor_new({shape}length, {shape}items, {}, {at})",
                    args[0]
                ),
                "construct" => format!(
                    "k_tensor_construct({}, {shape}length, {shape}items, {at})",
                    args[0]
                ),
                "transposeAxes" => format!(
                    "k_tensor_transpose({}, {shape}length, {shape}items, {at})",
                    pointer(0)
                ),
                "broadcast" | "reshape" => format!(
                    "k_tensor_{operation}({}, {shape}length, {shape}items, {at})",
                    pointer(0)
                ),
                _ => return Err(format!("Unsupported array shape operation {operation}")),
            };
            return wrap(call);
        }
        match operation {
            "new" => wrap(format!(
                "k_tensor_new({}, {}, {}, {at})",
                atoms.len() - 1,
                array(1),
                args[0]
            )),
            "construct" => wrap(format!(
                "k_tensor_construct({}, {}, {}, {at})",
                args[0],
                atoms.len() - 1,
                array(1)
            )),
            "copy" => wrap(format!("k_tensor_copy({}, {at})", pointer(0))),
            "transposeView" => wrap(format!("k_tensor_transpose_view({})", pointer(0))),
            "contiguous" => wrap(format!("k_tensor_contiguous({}, {at})", pointer(0))),
            "text" | "debug" => Ok(format!("k_tensor_{operation}({}, {at})", pointer(0))),
            "onesLike" | "zerosLike" => {
                let fill = if operation == "onesLike" {
                    "1.0"
                } else {
                    "0.0"
                };
                wrap(format!(
                    "k_tensor_new_like({}, {fill}, {at})",
                    pointer(0)
                ))
            }
            "requireRank" => Ok(format!(
                "k_tensor_require_rank({}, {}, {at})",
                pointer(0),
                args[1]
            )),
            "norm" => Ok(format!("k_tensor_norm({})", pointer(0))),
            "rank" => Ok(format!("{}value.rank", self.through(&atoms[0]))),
            "dimension" => Ok(format!(
                "k_tensor_dimension({}, {})",
                pointer(0),
                args[1]
            )),
            "getFlat" => Ok(format!(
                "k_tensor_flat({}, {})",
                pointer(0),
                args[1]
            )),
            "length" => Ok(format!("{}value.length", self.through(&atoms[0]))),
            "coordinate" => Ok(format!(
                "k_tensor_coordinate({}, {}, {})",
                pointer(0),
                args[1],
                args[2]
            )),
            "setFlat" => Ok(format!(
                "({}value.items[{}] = {})",
                self.through(&atoms[0]),
                args[1],
                args[2]
            )),
            "at" => Ok(format!(
                "k_tensor_at({}, {}, {}, {at})",
                pointer(0),
                atoms.len() - 1,
                array(1)
            )),
            "set" => Ok(format!(
                "k_tensor_set({}, {}, {}, {}, {at})",
                pointer(0),
                args[1],
                atoms.len() - 2,
                array(2)
            )),
            "slice" => {
                let (groups, _) = args[1..].as_chunks::<3>();
                let parts = groups
                    .iter()
                    .map(|p| format!("{{{},{},{}}}", p[0], p[1], p[2]))
                    .collect::<Vec<_>>()
                    .join(",");
                wrap(format!(
                    "k_tensor_slice({}, {}, (KTensorIndex[]){{{parts}}}, {at})",
                    pointer(0),
                    (atoms.len() - 1) / 3
                ))
            }
            "negate" => wrap(format!("k_tensor_negate({}, {at})", pointer(0))),
            "powerInt" => wrap(format!(
                "k_tensor_power_int({}, {}, {at})",
                pointer(0),
                args[1]
            )),
            "powerDouble" => wrap(format!(
                "k_tensor_power_double({}, {}, {at})",
                pointer(0),
                args[1]
            )),
            "multiply" | "matmul" | "add" | "subtract" => wrap(format!(
                "k_tensor_{operation}({}, {}, {at})",
                pointer(0),
                pointer(1)
            )),
            "scale" | "divide" | "add_scalar" | "subtract_scalar" | "right_subtract" => wrap(
                format!("k_tensor_{operation}({}, {}, {at})", pointer(0), args[1]),
            ),
            "broadcast" | "reshape" => wrap(format!(
                "k_tensor_{operation}({}, {}, {}, {at})",
                pointer(0),
                atoms.len() - 1,
                array(1)
            )),
            "transpose" | "transposeAxes" => wrap(format!(
                "k_tensor_transpose({}, {}, {}, {at})",
                pointer(0),
                if operation == "transpose" {
                    -1
                } else {
                    (atoms.len() - 1) as i64
                },
                array(1)
            )),
            "sum" | "mean" | "min" | "max" => {
                let code = match operation {
                    "sum" => 0,
                    "mean" => 1,
                    "min" => 2,
                    _ => 3,
                };
                if result == Ty::Double {
                    Ok(format!(
                        "k_tensor_reduce({}, {code}, {}, {}, NULL, {at})",
                        pointer(0),
                        args[1],
                        args[2]
                    ))
                } else {
                    wrap(format!(
                        "k_tensor_reduce_tensor({}, {code}, {}, {at})",
                        pointer(0),
                        args[1]
                    ))
                }
            }
            "fromArray" => {
                // The Array and Tensor own different storage; the argument is borrowed.
                let owner = self.through(&atoms[0]);
                wrap(format!(
                    "k_tensor_from_array({owner}length, {owner}items, {at})"
                ))
            }
            "checkShape" => {
                let Ty::Record(id) = result else {
                    return Err("checkShape needs its union result".into());
                };
                let LayoutKind::Union(types, _) = &self.program.layouts[id].kind else {
                    return Err("checkShape needs its union layout".into());
                };
                let success = types
                    .iter()
                    .position(crate::types::is_tensor)
                    .ok_or("checkShape needs a tensor case")?;
                let error = types
                    .iter()
                    .position(|t| *t == crate::types::Type::named("Error"))
                    .ok_or("checkShape needs an Error case")?;
                let LayoutKind::Union(_, cases) = &self.program.layouts[id].kind else {
                    unreachable!()
                };
                let Ty::Record(tensor) = cases[success].fields[0] else {
                    return Err("checkShape needs a tensor layout".into());
                };
                Ok(format!(
                    "(k_tensor_has_shape({}, {}, {}) ? k_aggregate_{id}_{success}((KRecord_{tensor}){{k_tensor_copy({}, {at})}}) : k_aggregate_{id}_{error}(k_runtime_error(6, K_TEXT(\"Tensor shape check failed\"))))",
                    pointer(0),
                    atoms.len() - 1,
                    array(1),
                    pointer(0)
                ))
            }
            _ => Err(format!("Unknown tensor operation {operation}")),
        }
    }
}
