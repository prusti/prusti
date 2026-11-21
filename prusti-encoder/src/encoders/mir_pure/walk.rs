//! The walk over a body's control flow graph.

use super::*;

impl<'vir: 'enc, 'enc> Enc<'vir, 'enc> {
    fn encode_common(
        &mut self,
        start: mir::BasicBlock,
        end: mir::BasicBlock,
        local_count: usize,
        result_local: mir::Local,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        let mut init = Update::new();
        let v0 = Version::default();
        // TODO: what about locals which never have StorageLive (i.e. always_live)?
        init.versions.insert(mir::RETURN_PLACE, v0);
        for local in 1..local_count {
            let local_ex = self.vcx.mk_lazy_expr(
                vir::vir_format!(self.vcx, "pure in _{local}"),
                self.get_ty_for_local(local.into()),
                Box::new(move |_vcx, lctx: ExprInput<'vir>| lctx.1[&local.into()].kind),
            );
            // check that `local` and `expr` type correspond
            self.bump_version_init(&mut init, local.into(), local_ex, v0.location);
        }

        let update = self.encode_cfg(&init.versions, start, end)?;

        // The body may unconditionally terminate with `unreachable` (e.g. a
        // `match` on an uninhabited type): its result is the unreachable snapshot.
        let Some(res) = init.merge(update) else {
            let result_ty = self.body.local_decls[result_local].ty;
            return Ok(self.ty_use(result_ty).unreachable_to_snap());
        };
        let ret_version = res.versions.get(&result_local).copied().unwrap_or(v0);
        self.uses.read(result_local, ret_version.index);

        let ex = self.mk_local_ex(result_local, ret_version);
        Ok(self.reify_binds(res, ex))
    }

    pub(super) fn encode_body(&mut self) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        assert!(
            !graph::is_cyclic(&self.body.basic_blocks),
            "MIR pure encoding does not support loops"
        );
        self.encode_common(
            mir::START_BLOCK,
            self.rev_doms.end,
            self.body.arg_count + 1,
            mir::RETURN_PLACE,
        )
    }

    pub(super) fn encode_spec_block(
        &mut self,
        block: mir::BasicBlock,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        let Some(mir::TerminatorKind::Call { destination, .. }) = self.body.basic_blocks[block]
            .terminator
            .as_ref()
            .map(|t| &t.kind)
        else {
            unreachable!("malformed spec-only block: should end in a call terminator");
        };
        assert!(destination.projection.is_empty());
        self.encode_common(
            block,
            self.body.basic_blocks.successors(block).next().unwrap(),
            self.body.local_decls.len(),
            destination.local,
        )
    }

    fn encode_cfg(
        &mut self,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
        curr: mir::BasicBlock,
        join_point: mir::BasicBlock,
    ) -> EncodeResult<'vir, Option<Update<'vir>>, MirPureEnc> {
        if curr == join_point {
            // We are done with the current fragment of the CFG, the rest is
            // handled in a parent call.
            return Ok(Some(Update::new()));
        }

        // walk block statements first
        let mut new_curr_ver = curr_ver.clone();
        let mut stmt_update = Update::new();
        for (statement_index, stmt) in self.body[curr].statements.iter().enumerate() {
            let location = mir::Location {
                block: curr,
                statement_index,
            };
            let newer = self.encode_stmt(&new_curr_ver, stmt, location)?;
            newer.add_to_map(&mut new_curr_ver);
            stmt_update = stmt_update.merge_inner(newer);
        }

        // then walk terminator. Its operands and call are encoded under its
        // span, like statements in `encode_stmt`; the walks of its successors
        // are not, as they encode other blocks.
        let term = self.body[curr].terminator.as_ref().unwrap();
        let location = mir::Location {
            block: curr,
            statement_index: self.body[curr].statements.len(),
        };
        match &term.kind {
            &mir::TerminatorKind::Goto { target }
            | &mir::TerminatorKind::FalseEdge {
                real_target: target,
                ..
            }
            | &mir::TerminatorKind::Drop { target, .. } => {
                let rest_update = self.encode_cfg(&new_curr_ver, target, join_point)?;
                Ok(stmt_update.merge(rest_update))
            }

            mir::TerminatorKind::SwitchInt { discr, targets } => {
                // The `if false` switch of a `ghost!` block or a `closure!`
                // spec marker: continue straight into the live target (the
                // inline ghost body, or the continuation); the other arm is
                // skipped.
                if let Some(live) = self.spec_arms.switches.get(&curr) {
                    let rest_update = self.encode_cfg(&new_curr_ver, *live, join_point)?;
                    return Ok(stmt_update.merge(rest_update));
                }

                // encode the discriminant operand
                let discr_expr = self
                    .vcx
                    .with_span(term.source_info.span, |_| {
                        self.encode_operand_snap(discr, &new_curr_ver)
                    })?
                    .downcast_ty();
                let discr_ty = discr.ty(self.body, self.vcx.tcx());
                let discr_ty_out = self.ty_use(discr_ty).expect_primitive();

                // walk `curr` -> `targets[i]` -> `join` for each target. The
                // join point the bb which is an immediate reverse dominator of
                // the branch point.
                // TODO: indexvec?
                let new_join_point = self.rev_doms.immediate_dominator(curr);
                let mut updates = targets
                    .all_targets()
                    .iter()
                    .map(|target| self.encode_cfg(&new_curr_ver, *target, new_join_point))
                    .collect::<Result<Vec<_>, _>>()?;

                // find locals updated in any of the results, which were also
                // defined before the branch
                let mut mod_locals = updates
                    .iter()
                    .flat_map(|update| update.iter().flat_map(|v| v.versions.keys()))
                    .filter(|local| new_curr_ver.contains_key(local))
                    .copied()
                    .collect::<Vec<_>>();
                mod_locals.sort();
                mod_locals.dedup();
                let mod_tys = mod_locals
                    .iter()
                    .map(|l| self.get_ty_for_local(*l).as_dyn())
                    .collect();

                // for each branch, create a Viper tuple of the updated locals
                let tuple_ref = self.deps.require_dep::<PairUseEnc>(mod_tys).unwrap();
                let otherwise_update = updates.pop().unwrap();
                let phi_expr = targets.iter().zip(updates).fold(
                    self.reify_branch(&tuple_ref, &mod_locals, &new_curr_ver, otherwise_update),
                    |expr, ((cond_val, _target), branch_update)| {
                        self.vcx.mk_ternary_expr(
                            self.vcx.mk_eq_expr(
                                discr_ty_out.snap_to_prim(discr_expr),
                                discr_ty_out.expr_from_bits(discr_ty, cond_val).lazy(),
                            ),
                            self.reify_branch(
                                &tuple_ref,
                                &mod_locals,
                                &new_curr_ver,
                                branch_update,
                            ),
                            expr,
                        )
                    },
                );

                // assign tuple into a `phi` variable
                let phi_idx = Version {
                    index: self.phi_ctr,
                    location,
                    initialised: None,
                };
                let phi = self.vcx.mk_local_decl(self.mk_phi(phi_idx), tuple_ref.ty);
                self.phi_ctr += 1;
                let mut phi_update = Update::new();
                phi_update.binds.push(UpdateBind::Phi(phi, phi_expr));

                // update locals by destructuring `phi` variable
                // TODO: maybe this is unnecessary, we could instead use tuple
                //   access directly instead of the locals going forward?
                for (elem_idx, local) in mod_locals.iter().enumerate() {
                    let expr = tuple_ref
                        .mk_elem(self.vcx.mk_local_ex(phi), elem_idx)
                        .downcast_ty();
                    self.bump_version(&mut phi_update, *local, expr, location);
                    new_curr_ver.insert(*local, phi_update.versions[local]);
                }

                // walk `join` -> `end`
                let end_update = self.encode_cfg(&new_curr_ver, new_join_point, join_point)?;
                Ok(stmt_update.merge(phi_update.merge(end_update)))
            }

            // TODO: there is some code duplication between here and SwitchInt
            mir::TerminatorKind::Assert {
                cond,
                expected,
                target,
                ..
            } => {
                // encode the condition operand
                let cond_ty = cond.ty(self.body, self.vcx.tcx());
                assert_eq!(*cond_ty.kind(), TyKind::Bool);
                let cond_expr = self
                    .vcx
                    .with_span(term.source_info.span, |_| {
                        self.encode_operand_snap(cond, &new_curr_ver)
                    })?
                    .downcast_ty::<vir::Bool>();

                // if cond == expected: walk the rest of the CFG
                let ok_update = self.encode_cfg(&new_curr_ver, *target, join_point)?;

                // find locals updated in the "ok" branch, which were also
                // defined before the branch
                // TODO: is the unwrap here ok? can ok_update also be None?
                let mut mod_locals = ok_update
                    .as_ref()
                    .unwrap()
                    .versions
                    .keys()
                    .filter(|local| new_curr_ver.contains_key(local))
                    .copied()
                    .collect::<Vec<_>>();
                mod_locals.sort();
                mod_locals.dedup();
                let mod_tys = mod_locals
                    .iter()
                    .map(|l| self.get_ty_for_local(*l).as_dyn())
                    .collect();

                // create a Viper tuple of the updated locals
                let tuple_ref = self.deps.require_dep::<PairUseEnc>(mod_tys).unwrap();
                let phi_expr = self.vcx.mk_ternary_expr(
                    self.vcx.mk_eq_expr(
                        cond_expr,
                        self.vcx
                            .mk_const_expr(vir::ConstData::Bool(*expected))
                            .downcast_ty(),
                    ),
                    self.reify_branch(&tuple_ref, &mod_locals, &new_curr_ver, ok_update),
                    self.reify_branch(&tuple_ref, &mod_locals, &new_curr_ver, None),
                );

                // assign tuple into a `phi` variable
                let phi_idx = Version {
                    index: self.phi_ctr,
                    location,
                    initialised: None,
                };
                let phi = self.vcx.mk_local_decl(self.mk_phi(phi_idx), tuple_ref.ty);
                self.phi_ctr += 1;
                let mut phi_update = Update::new();
                phi_update.binds.push(UpdateBind::Phi(phi, phi_expr));

                // update locals by destructuring `phi` variable
                // TODO: maybe this is unnecessary, we could instead use tuple
                //   access directly instead of the locals going forward?
                for (elem_idx, local) in mod_locals.iter().enumerate() {
                    let expr = tuple_ref
                        .mk_elem(self.vcx.mk_local_ex(phi), elem_idx)
                        .downcast_ty();
                    self.bump_version(&mut phi_update, *local, expr, location);
                    new_curr_ver.insert(*local, phi_update.versions[local]);
                }

                Ok(stmt_update.merge(Some(phi_update)))
            }

            mir::TerminatorKind::Return => Ok(Some(stmt_update)),

            mir::TerminatorKind::Unreachable => {
                // will result in generating an unreachable phi merge
                Ok(None)
            }

            mir::TerminatorKind::Call {
                func,
                args,
                destination,
                target,
                ..
            } => {
                let Some(target) = *target else {
                    // target is none if the call diverges (i.e. returns !), in
                    // this case treat it just like an unreachable terminator.
                    return Ok(None);
                };

                let func_ty = func.ty(self.body, self.vcx.tcx());
                let (def_id, arg_tys) = RustSignature::get_def_id_and_caller_substs(func_ty);
                let expr = self.vcx.with_span(term.source_info.span, |_| {
                    // Diverging calls were handled above; every other call
                    // must be to an intrinsic, a Prusti builtin or a pure
                    // function.
                    let is_pure = crate::encoders::is_function_pure(def_id, self.gargs(arg_tys));

                    // The bodiless `ptr_metadata` intrinsic is only lowered to
                    // `UnOp::PtrMetadata` in optimized MIR; do the lowering here.
                    let intrinsic = self.vcx.tcx().intrinsic(def_id);
                    let intrinsic = intrinsic.and_then(RustcIntrinsic::from_intrinsic);
                    if let Some(intrinsic) = intrinsic {
                        self.encode_intrinsic(intrinsic, arg_tys, args, &new_curr_ver)
                    } else if let Some(builtin) = PrustiBuiltin::new(def_id, self.gargs(arg_tys)) {
                        match self.encode_prusti_builtin(
                            builtin,
                            def_id,
                            self.gargs(arg_tys),
                            args,
                            term.source_info.span,
                            &new_curr_ver,
                        )? {
                            Some(expr) => Ok(expr),
                            // The pure-only builtins (quantifiers, spec blocks,
                            // mode markers).
                            None => {
                                let PrustiBuiltin::Spec(builtin) = builtin else {
                                    // Operand-based builtins are handled by
                                    // `PureRvalueEnc::encode_prusti_builtin` before this is reached.
                                    unreachable!(
                                        "operand-based builtin in the pure-only handler: {builtin:?}"
                                    )
                                };
                                self.encode_pure_only_builtin(
                                    builtin,
                                    arg_tys,
                                    args,
                                    curr,
                                    &new_curr_ver,
                                )
                            }
                        }
                    } else if is_pure {
                        let pure_func = self.deps.require_dep_spanned::<FunctionCallEnc>(
                            CallTaskDescription::new(self.context, arg_tys, def_id),
                            term.source_info.span,
                        )?;
                        let snap_args = args
                            .iter()
                            .map(|arg| self.encode_operand_snap(&arg.node, &new_curr_ver))
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok(pure_func.call_pure(snap_args))
                    } else {
                        Err(EncodeFullError::DependencyError(vec![
                            DependencyErrorLink::new(
                                MirPureEnc::ENCODER_NAME,
                                format!(
                                    "use of impure function `{}` in pure code is not allowed",
                                    self.vcx.tcx().def_path_str(def_id)
                                ),
                                vec![term.source_info.span],
                                ErrorKind::Incorrect,
                            ),
                        ]))
                    }
                });

                let expr = self.encode_place_update(
                    &new_curr_ver,
                    *destination,
                    expr?,
                    term.source_info.span,
                )?;
                let mut term_update = Update::new();
                self.bump_version(&mut term_update, destination.local, expr, location);
                term_update.add_to_map(&mut new_curr_ver);

                // walk rest of CFG
                let end_update = self.encode_cfg(&new_curr_ver, target, join_point)?;

                Ok(stmt_update.merge_inner(term_update).merge(end_update))
            }

            k => todo!("terminator kind {k:?}"),
        }
    }

    fn encode_stmt(
        &mut self,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
        stmt: &mir::Statement<'vir>,
        location: mir::Location,
    ) -> EncodeResult<'vir, Update<'vir>, MirPureEnc> {
        // Encoded under the statement's span: expressions pick up the ambient
        // span at creation, which makes error positions inside pure/spec
        // bodies point at the originating source (e.g. the failing conjunct
        // of a spec).
        self.vcx.with_span(stmt.source_info.span, |_| {
            let mut update = Update::new();
            match &stmt.kind {
                &mir::StatementKind::StorageLive(local) => {
                    let new_version = self.bump_version_no_assign(local, location);
                    update.versions.insert(local, new_version);
                }
                mir::StatementKind::StorageDead(..)
                | mir::StatementKind::FakeRead(..)
                | mir::StatementKind::AscribeUserType(..)
                | mir::StatementKind::PlaceMention(..)
                | mir::StatementKind::Coverage(..)
                | mir::StatementKind::ConstEvalCounter
                | mir::StatementKind::Nop
                | mir::StatementKind::BackwardIncompatibleDropHint { .. } => {} // nop
                mir::StatementKind::Assign(box (dest, rvalue)) => {
                    let span = stmt.source_info.span;
                    self.uses.start_assignment();
                    let expr = self
                        .encode_rvalue(curr_ver, rvalue, span)
                        .and_then(|expr| self.encode_place_update(curr_ver, *dest, expr, span));
                    let reads = self.uses.end_assignment();
                    self.bump_version(&mut update, dest.local, expr?, location);
                    let version = update.versions[&dest.local].index;
                    self.uses.assigned(dest.local, version, reads);
                }
                k => todo!("statement kind {k:?}"),
            }
            Ok(update)
        })
    }
}
