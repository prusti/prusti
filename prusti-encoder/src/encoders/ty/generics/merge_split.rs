use prusti_rustc_interface::index::Idx;
use task_encoder::{EncodeFullError, EncodeFullResult, TaskEncoder, TaskEncoderDependencies};
use vir::{CastType, MethodIdn};

use crate::encoders::{
    Pure,
    ty::{
        RustTyDecomposition,
        indirect_pred::{IndirectPredEnc, LifetimePositionBlock},
        lifted::ty_constructor::TyConstructorEnc,
        pure::TyPureEnc,
    },
};

use super::{
    GArgsTyEnc, GenericParamsEnc,
    casters::CastersEnc,
    lifetime_position::{LifetimePosition, LifetimePositionEnc},
};

/// Trusted methods that merge the indirect predicates of the lifetime
/// positions `[first, end)` of an instantiated type into one predicate over
/// their intervals, and split it again. A call to a generic function merges
/// each block of positions that a position of the callee covers into the
/// callee's predicate before the call, and splits it after the call. A block
/// may be empty, when the callee's position is the lifetimes in a type
/// parameter instantiated with a type without lifetimes: its predicate holds
/// nothing. Both methods state that extending through the merged predicate
/// reads what extending through each position's predicate on its key reads,
/// grafted together.
pub struct MergeSplitEnc;

/// The predicate a block is merged into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MergeTarget {
    /// The instantiated type's own predicate.
    Own,
    /// The predicate of a type parameter instantiated with the type, on the
    /// generic snapshot. The block is all positions of the type.
    Param,
}

#[derive(Debug, Clone, Copy)]
pub struct MergeSplit<'vir> {
    pub merge: MergeSplitIdn<'vir>,
    pub split: MergeSplitIdn<'vir>,
}

pub type MergeSplitIdn<'vir> = MethodIdn<'vir, (vir::Snap, vir::ManyTyVal, vir::ManyCSnap)>;

impl<'vir> task_encoder::OutputRefAny for MergeSplit<'vir> {}

/// The instantiated type has a const parameter, which canonical keys do not
/// support yet.
#[derive(Debug, Clone)]
pub struct ConstParamInBlock;

impl TaskEncoder for MergeSplitEnc {
    task_encoder::encoder_cache!(MergeSplitEnc);
    const ENCODER_NAME: &'static str = "merge/split encoder";

    type TaskDescription<'vir> = (
        RustTyDecomposition<'vir>,
        LifetimePosition,
        LifetimePosition,
        MergeTarget,
    );
    type OutputRef<'vir> = MergeSplit<'vir>;
    type OutputFullLocal<'vir> = [vir::Method<'vir>; 2];
    type EncodingError = ConstParamInBlock;

    // Instantiations that differ only in their context or lifetimes share the
    // methods. A type parameter's own predicate is already the parameter's.
    fn task_to_key<'vir>(
        (ty, first, end, target): &Self::TaskDescription<'vir>,
    ) -> Self::TaskKey<'vir> {
        let target = if ty.ty.specifics.is_param() {
            MergeTarget::Own
        } else {
            *target
        };
        (ty.canonical().unwrap_or(*ty), *first, *end, target)
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        let (ty, first, end, target) = *task_key;
        if ty.canonical().is_none() {
            return Err(EncodeFullError::EncodingError(ConstParamInBlock, None));
        }
        let graph = deps.require_dep::<LifetimePositionEnc>(ty)?;
        let count = graph.positions().count();
        assert!(first <= end && end.index() <= count, "block out of range");
        assert!(
            target == MergeTarget::Own || (first.index() == 0 && end.index() == count),
            "a type parameter's predicate covers all positions of its instantiation"
        );
        let snapshot = deps.require_ref::<TyPureEnc>(ty.ty)?.snapshot;
        let ind = deps.require_ref::<IndirectPredEnc>(ty.ty)?;
        let params = deps.require_dep::<GenericParamsEnc>(ty.args.context())?;
        let args = deps.require_dep::<GArgsTyEnc>(ty.args)?;
        let param_target = match target {
            MergeTarget::Own => None,
            MergeTarget::Param => {
                let param = RustTyDecomposition::param();
                Some((
                    deps.require_ref::<IndirectPredEnc>(param)?,
                    deps.require_ref::<CastersEnc<Pure>>((param, ty.ty))?,
                    deps.require_ref::<TyConstructorEnc>(ty.ty)?,
                ))
            }
        };
        vir::with_vcx(|vcx| {
            let name = ty.instantiated_name();
            let (f, e) = (first.index(), end.index());
            let kind = match target {
                MergeTarget::Own => "ind",
                MergeTarget::Param => "param_ind",
            };
            let arg_tys = (snapshot, params.ty_args(), params.const_args());
            let merge = MethodIdn::new(
                vir::vir_format_identifier!(vcx, "merge_{kind}_{name}_{f}_{e}"),
                arg_tys,
            );
            let split = MethodIdn::new(
                vir::vir_format_identifier!(vcx, "split_{kind}_{name}_{f}_{e}"),
                arg_tys,
            );
            deps.emit_output_ref(*task_key, MergeSplit { merge, split })?;

            let k_decl = vcx.mk_local_decl("k", snapshot);
            // The contracts name each written-out type argument once, in a `let`.
            let mut bindings = Vec::new();
            let ty_args: &[vir::ExprTyVal<'vir>] = args.get_ty();
            let tys = ty_args
                .iter()
                .enumerate()
                .map(|(i, &ty_arg)| {
                    if matches!(ty_arg.kind, vir::ExprKindGenData::Local(_)) {
                        return ty_arg;
                    }
                    let name = if ty_args.len() == 1 {
                        "ty".to_string()
                    } else {
                        format!("ty{i}")
                    };
                    let decl = vcx.mk_local_decl(vcx.alloc_str(&name), vir::TYPE_TYVAL);
                    bindings.push((decl, ty_arg));
                    vcx.mk_local_ex(decl)
                })
                .collect::<Vec<_>>();
            let with_bindings = |e: vir::ExprBool<'vir>| {
                bindings
                    .iter()
                    .rev()
                    .fold(e, |e, &(decl, ty_arg)| vcx.mk_let_expr(decl, ty_arg, e))
            };
            let block = LifetimePositionBlock {
                graph: &graph,
                first,
                end,
                ind,
                tys: vcx.alloc_slice(&tys),
                consts: args.get_const(),
                k: vcx.mk_local_ex(k_decl),
            };
            let positions = block.positions();
            // Outermost first, so that each key's extensions are framed by the
            // predicates before it.
            let preds = positions
                .iter()
                .map(|&p| {
                    let (lo, hi) = graph.interval(p);
                    block.predicate(block.key(p), lo, hi)
                })
                .collect::<Vec<_>>();
            // An empty block's interval is empty, at its place among the
            // positions.
            let (lo, hi) = if first < end {
                let (lo, _) = graph.interval(first);
                let (_, hi) = graph.interval(LifetimePosition::new(end.index() - 1));
                (lo, hi)
            } else {
                let at = if first.index() < count {
                    graph.interval(first).0
                } else if let Some(last) = count.checked_sub(1) {
                    graph.interval(LifetimePosition::new(last)).1
                } else {
                    vcx.mk_uint::<0>()
                };
                (at, at)
            };
            let grafted = block.fold(&positions);
            let (merged, extended, grafted) = match param_target {
                None => (
                    block.predicate(block.k, lo, hi),
                    ind.extend.call()(block.k, block.tys, block.consts, lo, hi),
                    grafted,
                ),
                Some((param_ind, casters, ty_constructor)) => {
                    let generic = |e: vir::ExprSnap<'vir>| -> vir::ExprSnap<'vir> {
                        casters.make_generic.call()(e.downcast_ty(), block.tys, block.consts)
                            .upcast_ty()
                    };
                    let param_ty = (ty_constructor.ty_constructor)(block.tys, block.consts);
                    let no_consts: &[vir::ExprCSnap<'vir>] = &[];
                    let generic_k = generic(block.k);
                    let merged = vir::expr! { vcx;
                        acc([param_ind.predicate](
                            generic_k,
                            [..[&[param_ty]]],
                            [..[no_consts]],
                            lo,
                            hi
                        ))
                    };
                    let extended =
                        param_ind.extend.call()(generic_k, &[param_ty], no_consts, lo, hi);
                    (merged, extended, generic(grafted))
                }
            };
            let decls = (k_decl, params.ty_decls(), params.const_decls());
            let merge_post =
                conjunction(&[merged, vcx.mk_eq_expr(extended, vcx.mk_old_expr(grafted))]);
            let merge = vcx.mk_method(
                merge,
                decls,
                &[],
                vcx.alloc_slice(&[with_bindings(conjunction(&preds))]),
                vcx.alloc_slice(&[with_bindings(merge_post)]),
                None,
            );
            let mut split_posts = preds;
            split_posts.push(vcx.mk_eq_expr(grafted, vcx.mk_old_expr(extended)));
            let split = vcx.mk_method(
                split,
                decls,
                &[],
                vcx.alloc_slice(&[with_bindings(merged)]),
                vcx.alloc_slice(&[with_bindings(conjunction(&split_posts))]),
                None,
            );
            Ok(([merge, split], ()))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for methods in Self::all_outputs_local_no_errors(program) {
            for method in methods {
                program.add_method(method);
            }
        }
    }
}

/// `parts` joined by `&&` from the left, which Viper prints as one flat
/// conjunction, in the order the parts frame each other.
// TODO: `vcx.mk_conj` builds a balanced tree, which keeps long conjunctions shallow but
// prints with parentheses; once it joins short conjunctions from the left, use it here.
fn conjunction<'vir>(parts: &[vir::ExprBool<'vir>]) -> vir::ExprBool<'vir> {
    vir::with_vcx(|vcx| {
        parts
            .iter()
            .copied()
            .reduce(|conj, part| {
                vcx.mk_bin_op_expr(vir::BinOpKind::And, conj, part)
                    .downcast_ty()
            })
            .unwrap_or_else(|| vcx.mk_bool::<true>())
    })
}

/// Tests that verify `merge`/`split` with Silicon: merging the predicates of
/// the positions of a block, and casting the result to the predicate of a type
/// parameter where the block is one, keeps what extending through them reads;
/// likewise casting back and splitting.
#[cfg(test)]
mod verification_tests {
    use prusti_rustc_interface::{index::Idx, span::DUMMY_SP};
    use task_encoder::TaskEncoder;

    use super::{LifetimePosition, MergeSplitEnc, MergeTarget};
    use crate::encoders::ty::{
        RustTyDecomposition, generics::IndirectCastersEnc, indirect_pred::IndirectPredEnc,
        test_util::verify,
    };

    /// Asserts that the methods in `viper` verify together with the indirect
    /// predicate of the type alias `X` defined in `src`, its casts, and
    /// `merge`/`split` of `X` for each of `blocks`, given by its first and end
    /// position and its target; and that each of them does not with
    /// `assert false` at the end of its body, the probe that its state is
    /// consistent.
    fn assert_merge_split(src: &str, blocks: &[(usize, usize, MergeTarget)], viper: &str) {
        let verify_with_merge = |viper: &str| {
            verify(
                src,
                |ty| {
                    IndirectPredEnc::encode(ty.ty, true, DUMMY_SP).unwrap();
                    for &(first, end, target) in blocks {
                        let (first, end) =
                            (LifetimePosition::new(first), LifetimePosition::new(end));
                        let block = (ty, first, end, target);
                        MergeSplitEnc::encode(block, true, DUMMY_SP).unwrap();
                    }
                    let casts = (RustTyDecomposition::param(), ty.ty);
                    IndirectCastersEnc::encode(casts, true, DUMMY_SP).unwrap();
                },
                viper,
            )
        };
        if let Err(output) = verify_with_merge(viper) {
            panic!("verification failed:\n{output}");
        }
        // The bodies end in a line of just `}`.
        let ends = viper
            .match_indices("\n            }")
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert!(!ends.is_empty(), "no method body to probe");
        for end in ends {
            let probe = format!(
                "{}\n              assert false{}",
                &viper[..end],
                &viper[end..]
            );
            match verify_with_merge(&probe) {
                Ok(()) => panic!("probe verified:\n{probe}"),
                Err(output) => assert!(
                    output.contains("Assert might fail"),
                    "probe failed differently:\n{output}"
                ),
            }
        }
    }

    /// `&'a mut i32` as a type parameter, merge/split are identities.
    #[test]
    fn reference_as_param() {
        assert_merge_split(
            "type X<'a> = &'a mut i32;",
            &[(0, 1, MergeTarget::Own), (0, 1, MergeTarget::Param)],
            r#"
            define I32 s_Int_i32_type()
            define RI s_Ref_mutable_type(I32)
            define G(k) make_generic_s_Ref_mutable(k, I32)

            method merge(k: s_Ref_mutable)
              requires p_Ref_mutable_ind(k, I32, 0, 1)
              ensures p_Param_ind(G(k), RI, 0, 1)
              ensures extend_Param(G(k), RI, 0, 1) == old(G(extend_Ref_mutable(k, I32, 0, 1)))
            {
              merge_ind_Ref_mutable$lt$Int_i32$gt$_0_1(k)
              make_generic_ind_Ref_mutable(k, I32, 0, 1)
            }

            method split(k: s_Ref_mutable)
              requires p_Param_ind(G(k), RI, 0, 1)
              ensures p_Ref_mutable_ind(k, I32, 0, 1)
              ensures G(extend_Ref_mutable(k, I32, 0, 1)) == old(extend_Param(G(k), RI, 0, 1))
            {
              make_concrete_ind_Ref_mutable(k, I32, 0, 1)
              split_ind_Ref_mutable$lt$Int_i32$gt$_0_1(k)
            }

            method merge_param(k: s_Ref_mutable)
              requires p_Ref_mutable_ind(k, I32, 0, 1)
              ensures p_Param_ind(G(k), RI, 0, 1)
              ensures extend_Param(G(k), RI, 0, 1) == old(G(extend_Ref_mutable(k, I32, 0, 1)))
            {
              merge_param_ind_Ref_mutable$lt$Int_i32$gt$_0_1(k)
            }

            method split_param(k: s_Ref_mutable)
              requires p_Param_ind(G(k), RI, 0, 1)
              ensures p_Ref_mutable_ind(k, I32, 0, 1)
              ensures G(extend_Ref_mutable(k, I32, 0, 1)) == old(extend_Param(G(k), RI, 0, 1))
            {
              split_param_ind_Ref_mutable$lt$Int_i32$gt$_0_1(k)
            }
            "#,
        );
    }

    /// `N<'b, 'a>` as a type parameter, with `'b` behind `'a`.
    #[test]
    fn nested_reference_as_param() {
        assert_merge_split(
            "struct N<'b, 'a>(&'a mut &'b mut i32);
             type X<'b, 'a> = N<'b, 'a>;",
            &[(0, 2, MergeTarget::Own), (0, 2, MergeTarget::Param)],
            r#"
            define NT s_N_type()
            define G(k) make_generic_s_N(k)
            // The key of the predicate of 'b: `k` with what 'a reads grafted in.
            define KB(k) extend_N(k, 1, 2)

            method merge(k: s_N)
              requires p_N_ind(k, 1, 2) && p_N_ind(KB(k), 0, 1)
              ensures p_Param_ind(G(k), NT, 0, 2)
              ensures extend_Param(G(k), NT, 0, 2) == old(G(extend_N(KB(k), 0, 1)))
            {
              merge_ind_N_0_2(k)
              make_generic_ind_N(k, 0, 2)
            }

            method split(k: s_N)
              requires p_Param_ind(G(k), NT, 0, 2)
              ensures p_N_ind(k, 1, 2) && p_N_ind(KB(k), 0, 1)
              ensures G(extend_N(KB(k), 0, 1)) == old(extend_Param(G(k), NT, 0, 2))
            {
              make_concrete_ind_N(k, 0, 2)
              split_ind_N_0_2(k)
            }

            method merge_param(k: s_N)
              requires p_N_ind(k, 1, 2) && p_N_ind(KB(k), 0, 1)
              ensures p_Param_ind(G(k), NT, 0, 2)
              ensures extend_Param(G(k), NT, 0, 2) == old(G(extend_N(KB(k), 0, 1)))
            {
              merge_param_ind_N_0_2(k)
            }

            method split_param(k: s_N)
              requires p_Param_ind(G(k), NT, 0, 2)
              ensures p_N_ind(k, 1, 2) && p_N_ind(KB(k), 0, 1)
              ensures G(extend_N(KB(k), 0, 1)) == old(extend_Param(G(k), NT, 0, 2))
            {
              split_param_ind_N_0_2(k)
            }
            "#,
        );
    }

    /// `A<'a, 'b>` as a type parameter, with unrelated `'a` and `'b`.
    #[test]
    fn unrelated_positions_as_param() {
        assert_merge_split(
            "struct A<'a, 'b>(&'a mut i32, &'b mut i32);
             type X<'a, 'b> = A<'a, 'b>;",
            &[(0, 2, MergeTarget::Own), (0, 2, MergeTarget::Param)],
            r#"
            define AT s_A_type()
            define G(k) make_generic_s_A(k)

            method merge(k: s_A)
              requires p_A_ind(k, 0, 1) && p_A_ind(k, 1, 2)
              ensures p_Param_ind(G(k), AT, 0, 2)
              ensures extend_Param(G(k), AT, 0, 2).make_concrete_s_A.s_A_0 == old(extend_A(k, 0, 1).s_A_0)
              ensures extend_Param(G(k), AT, 0, 2).make_concrete_s_A.s_A_1 == old(extend_A(k, 1, 2).s_A_1)
            {
              merge_ind_A_0_2(k)
              make_generic_ind_A(k, 0, 2)
            }

            method split(k: s_A)
              requires p_Param_ind(G(k), AT, 0, 2)
              ensures p_A_ind(k, 0, 1) && p_A_ind(k, 1, 2)
              ensures extend_A(k, 0, 1).s_A_0 == old(extend_Param(G(k), AT, 0, 2)).make_concrete_s_A.s_A_0
              ensures extend_A(k, 1, 2).s_A_1 == old(extend_Param(G(k), AT, 0, 2)).make_concrete_s_A.s_A_1
            {
              make_concrete_ind_A(k, 0, 2)
              split_ind_A_0_2(k)
            }

            method merge_param(k: s_A)
              requires p_A_ind(k, 0, 1) && p_A_ind(k, 1, 2)
              ensures p_Param_ind(G(k), AT, 0, 2)
              ensures extend_Param(G(k), AT, 0, 2).make_concrete_s_A.s_A_0 == old(extend_A(k, 0, 1).s_A_0)
              ensures extend_Param(G(k), AT, 0, 2).make_concrete_s_A.s_A_1 == old(extend_A(k, 1, 2).s_A_1)
            {
              merge_param_ind_A_0_2(k)
            }

            method split_param(k: s_A)
              requires p_Param_ind(G(k), AT, 0, 2)
              ensures p_A_ind(k, 0, 1) && p_A_ind(k, 1, 2)
              ensures extend_A(k, 0, 1).s_A_0 == old(extend_Param(G(k), AT, 0, 2)).make_concrete_s_A.s_A_0
              ensures extend_A(k, 1, 2).s_A_1 == old(extend_Param(G(k), AT, 0, 2)).make_concrete_s_A.s_A_1
            {
              split_param_ind_A_0_2(k)
            }
            "#,
        );
    }

    /// `&'a mut T` as a type parameter, in a context generic in `T`.
    #[test]
    fn generic_reference_as_param() {
        assert_merge_split(
            "type X<'a, T> = &'a mut T;",
            &[(0, 2, MergeTarget::Own), (0, 2, MergeTarget::Param)],
            r#"
            define RT(T) s_Ref_mutable_type(T)
            define G(k, T) make_generic_s_Ref_mutable(k, T)
            // The key of the predicate of the positions of `T`: `k` with what 'a
            // reads grafted in.
            define KT(k, T) extend_Ref_mutable(k, T, 0, 1)

            method merge(k: s_Ref_mutable, T: Type)
              requires p_Ref_mutable_ind(k, T, 0, 1)
              requires p_Ref_mutable_ind(KT(k, T), T, 1, 1 + arity(T))
              ensures p_Param_ind(G(k, T), RT(T), 0, 1 + arity(T))
              ensures extend_Param(G(k, T), RT(T), 0, 1 + arity(T)) ==
                old(G(extend_Ref_mutable(KT(k, T), T, 1, 1 + arity(T)), T))
            {
              merge_ind_Ref_mutable$lt$P0$gt$_0_2(k, T)
              make_generic_ind_Ref_mutable(k, T, 0, 1 + arity(T))
            }

            method split(k: s_Ref_mutable, T: Type)
              requires p_Param_ind(G(k, T), RT(T), 0, 1 + arity(T))
              ensures p_Ref_mutable_ind(k, T, 0, 1)
              ensures p_Ref_mutable_ind(KT(k, T), T, 1, 1 + arity(T))
              ensures G(extend_Ref_mutable(KT(k, T), T, 1, 1 + arity(T)), T) ==
                old(extend_Param(G(k, T), RT(T), 0, 1 + arity(T)))
            {
              make_concrete_ind_Ref_mutable(k, T, 0, 1 + arity(T))
              split_ind_Ref_mutable$lt$P0$gt$_0_2(k, T)
            }

            method merge_param(k: s_Ref_mutable, T: Type)
              requires p_Ref_mutable_ind(k, T, 0, 1)
              requires p_Ref_mutable_ind(KT(k, T), T, 1, 1 + arity(T))
              ensures p_Param_ind(G(k, T), RT(T), 0, 1 + arity(T))
              ensures extend_Param(G(k, T), RT(T), 0, 1 + arity(T)) ==
                old(G(extend_Ref_mutable(KT(k, T), T, 1, 1 + arity(T)), T))
            {
              merge_param_ind_Ref_mutable$lt$P0$gt$_0_2(k, T)
            }

            method split_param(k: s_Ref_mutable, T: Type)
              requires p_Param_ind(G(k, T), RT(T), 0, 1 + arity(T))
              ensures p_Ref_mutable_ind(k, T, 0, 1)
              ensures p_Ref_mutable_ind(KT(k, T), T, 1, 1 + arity(T))
              ensures G(extend_Ref_mutable(KT(k, T), T, 1, 1 + arity(T)), T) ==
                old(extend_Param(G(k, T), RT(T), 0, 1 + arity(T)))
            {
              split_param_ind_Ref_mutable$lt$P0$gt$_0_2(k, T)
            }
            "#,
        );
    }

    /// `&'a mut &'b mut T` as `&'c mut U`: merges `'b` and `T`.
    #[test]
    fn generic_nested_reference_as_reference_to_param() {
        assert_merge_split(
            "type X<'a, 'b, T> = &'a mut &'b mut T;",
            &[(1, 3, MergeTarget::Own)],
            r#"
            define RT(T) s_Ref_mutable_type(T)
            // The keys of the predicates of 'b and of the positions of `T`.
            define KB(k, T) extend_Ref_mutable(k, RT(T), 0, 1)
            define KT(k, T) extend_Ref_mutable(KB(k, T), RT(T), 1, 2)

            method merge(k: s_Ref_mutable, T: Type)
              requires p_Ref_mutable_ind(k, RT(T), 0, 1)
              requires p_Ref_mutable_ind(KB(k, T), RT(T), 1, 2)
              requires p_Ref_mutable_ind(KT(k, T), RT(T), 2, 2 + arity(T))
              ensures p_Ref_mutable_ind(k, RT(T), 0, 1)
              ensures p_Ref_mutable_ind(KB(k, T), RT(T), 1, 2 + arity(T))
              ensures extend_Ref_mutable(KB(k, T), RT(T), 1, 2 + arity(T)) ==
                old(extend_Ref_mutable(KT(k, T), RT(T), 2, 2 + arity(T)))
            {
              merge_ind_Ref_mutable$lt$Ref_mutable$lt$P0$gt$$gt$_1_3(KB(k, T), T)
            }

            method split(k: s_Ref_mutable, T: Type)
              requires p_Ref_mutable_ind(k, RT(T), 0, 1)
              requires p_Ref_mutable_ind(KB(k, T), RT(T), 1, 2 + arity(T))
              ensures p_Ref_mutable_ind(k, RT(T), 0, 1)
              ensures p_Ref_mutable_ind(KB(k, T), RT(T), 1, 2)
              ensures p_Ref_mutable_ind(KT(k, T), RT(T), 2, 2 + arity(T))
              ensures extend_Ref_mutable(KT(k, T), RT(T), 2, 2 + arity(T)) ==
                old(extend_Ref_mutable(KB(k, T), RT(T), 1, 2 + arity(T)))
            {
              split_ind_Ref_mutable$lt$Ref_mutable$lt$P0$gt$$gt$_1_3(KB(k, T), T)
            }
            "#,
        );
    }

    /// `(T, &'a mut &'b mut i32)`: merges `'a` and `'b`, after `T`.
    #[test]
    fn block_after_type_param() {
        assert_merge_split(
            "type X<'a, 'b, T> = (T, &'a mut &'b mut i32);",
            &[(1, 3, MergeTarget::Own)],
            r#"
            define RRI s_Ref_mutable_type(s_Ref_mutable_type(s_Int_i32_type()))
            // The key of the predicate of 'b: `k` with what 'a reads grafted in.
            define KB(k, T) extend_2_Tuple(k, T, RRI, arity(T), arity(T) + 1)

            method merge(k: s_2_Tuple, T: Type)
              requires p_2_Tuple_ind(k, T, RRI, arity(T), arity(T) + 1)
              requires p_2_Tuple_ind(KB(k, T), T, RRI, arity(T) + 1, arity(T) + 2)
              ensures p_2_Tuple_ind(k, T, RRI, arity(T), arity(T) + 2)
              ensures extend_2_Tuple(k, T, RRI, arity(T), arity(T) + 2) ==
                old(extend_2_Tuple(KB(k, T), T, RRI, arity(T) + 1, arity(T) + 2))
            {
              merge_ind_2_Tuple$lt$P0$com$$sp$Ref_mutable$lt$Ref_mutable$lt$Int_i32$gt$$gt$$gt$_1_3(k, T)
            }

            method split(k: s_2_Tuple, T: Type)
              requires p_2_Tuple_ind(k, T, RRI, arity(T), arity(T) + 2)
              ensures p_2_Tuple_ind(k, T, RRI, arity(T), arity(T) + 1)
              ensures p_2_Tuple_ind(KB(k, T), T, RRI, arity(T) + 1, arity(T) + 2)
              ensures extend_2_Tuple(KB(k, T), T, RRI, arity(T) + 1, arity(T) + 2) ==
                old(extend_2_Tuple(k, T, RRI, arity(T), arity(T) + 2))
            {
              split_ind_2_Tuple$lt$P0$com$$sp$Ref_mutable$lt$Ref_mutable$lt$Int_i32$gt$$gt$$gt$_1_3(k, T)
            }
            "#,
        );
    }

    /// `Option<&'a mut Box<List<'a, 'b>>>` as `Option<T>`.
    #[test]
    fn recursive_type_in_option() {
        assert_merge_split(
            "struct List<'a, 'b> {
                 x: &'a mut i32,
                 y: &'b mut i32,
                 next: Option<Box<List<'a, 'b>>>,
             }
             type X<'a, 'b> = Option<&'a mut Box<List<'a, 'b>>>;",
            &[(0, 3, MergeTarget::Own)],
            r#"
            define RBL s_Ref_mutable_type(s_Box_type(s_List_type(), s_Global_type()))
            // The key of the predicates of `List`'s positions: `k` with what the
            // reference's 'a reads grafted in.
            define KA(k) extend_Option(k, RBL, 0, 1)
            // What the positions of `List` read, grafted together.
            define LIST(k) graft_Option(extend_Option(KA(k), RBL, 1, 2), RBL, extend_Option(KA(k), RBL, 2, 3), 2, 3)

            method merge(k: s_Option)
              requires p_Option_ind(k, RBL, 0, 1)
              requires p_Option_ind(KA(k), RBL, 1, 2) && p_Option_ind(KA(k), RBL, 2, 3)
              ensures p_Option_ind(k, RBL, 0, 3)
              ensures extend_Option(k, RBL, 0, 3) == old(LIST(k))
            {
              merge_ind_Option$lt$Ref_mutable$lt$Box$lt$List$com$$sp$Global$gt$$gt$$gt$_0_3(k)
            }

            method split(k: s_Option)
              requires p_Option_ind(k, RBL, 0, 3)
              ensures p_Option_ind(k, RBL, 0, 1)
              ensures p_Option_ind(KA(k), RBL, 1, 2) && p_Option_ind(KA(k), RBL, 2, 3)
              ensures LIST(k) == old(extend_Option(k, RBL, 0, 3))
            {
              split_ind_Option$lt$Ref_mutable$lt$Box$lt$List$com$$sp$Global$gt$$gt$$gt$_0_3(k)
            }
            "#,
        );
    }
    /// `&'a mut i32` as `&'a mut U` with `U = i32`: the block of `U` is empty,
    /// after `'a`; its predicate is created from nothing and dropped again.
    #[test]
    fn empty_block() {
        assert_merge_split(
            "type X<'a> = &'a mut i32;",
            &[(1, 1, MergeTarget::Own)],
            r#"
            define I32 s_Int_i32_type()

            method merge(k: s_Ref_mutable)
              ensures p_Ref_mutable_ind(k, I32, 1, 1 + arity(I32))
              ensures extend_Ref_mutable(k, I32, 1, 1 + arity(I32)) == k
            {
              merge_ind_Ref_mutable$lt$Int_i32$gt$_1_1(k)
            }

            method split(k: s_Ref_mutable)
              requires p_Ref_mutable_ind(k, I32, 1, 1 + arity(I32))
            {
              split_ind_Ref_mutable$lt$Int_i32$gt$_1_1(k)
            }
            "#,
        );
    }

    /// `i32` as a type parameter: it has no positions, so the parameter's
    /// predicate is empty, created from nothing and dropped again.
    #[test]
    fn empty_block_as_param() {
        assert_merge_split(
            "type X = i32;",
            &[(0, 0, MergeTarget::Param)],
            r#"
            define I32 s_Int_i32_type()
            define G(k) make_generic_s_Int_i32(k)

            method merge(k: s_Int_i32)
              ensures p_Param_ind(G(k), I32, 0, arity(I32))
              ensures extend_Param(G(k), I32, 0, arity(I32)) == G(k)
            {
              merge_param_ind_Int_i32_0_0(k)
            }

            method split(k: s_Int_i32)
              requires p_Param_ind(G(k), I32, 0, arity(I32))
            {
              split_param_ind_Int_i32_0_0(k)
            }
            "#,
        );
    }
}
