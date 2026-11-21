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
        let mut init = Fragment::default();
        let v0 = Version::default();
        // TODO: what about locals which never have StorageLive (i.e. always_live)?
        init.versions.insert(mir::RETURN_PLACE, v0);
        for local in 1..local_count {
            let local_ex = self.input(local.into());
            self.bump_version_init(&mut init, local.into(), local_ex, v0.location);
        }

        // The body may unconditionally terminate with `unreachable` (e.g. a
        // `match` on an uninhabited type): its result is the unreachable snapshot.
        let Some(res) = self.encode_cfg(init, start, end)? else {
            let result_ty = self.body.local_decls[result_local].ty;
            return Ok(self.ty_use(result_ty).unreachable_to_snap());
        };
        let ret_version = res.versions.get(&result_local).copied().unwrap_or(v0);
        self.uses.read(result_local, ret_version.index);

        let ex = self.mk_local_ex(ret_version);
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

    /// Extends `fragment` with the region from `curr` up to `join_point`
    /// (not included), or returns `None` if the region cannot complete.
    fn encode_cfg(
        &mut self,
        mut fragment: Fragment<'vir>,
        curr: mir::BasicBlock,
        join_point: mir::BasicBlock,
    ) -> EncodeResult<'vir, Option<Fragment<'vir>>, MirPureEnc> {
        if curr == join_point {
            // We are done with the current fragment of the CFG, the rest is
            // handled in a parent call.
            return Ok(Some(fragment));
        }

        // walk block statements first
        for (statement_index, stmt) in self.body[curr].statements.iter().enumerate() {
            let location = mir::Location {
                block: curr,
                statement_index,
            };
            self.encode_stmt(&mut fragment, stmt, location)?;
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
                self.encode_cfg(fragment, target, join_point)
            }

            mir::TerminatorKind::SwitchInt { discr, targets } => {
                // The `if false` switch of a `ghost!` block or a `closure!`
                // spec marker: continue straight into the live target (the
                // inline ghost body, or the continuation); the other arm is
                // skipped.
                if let Some(live) = self.spec_arms.switches.get(&curr) {
                    return self.encode_cfg(fragment, *live, join_point);
                }

                // encode the discriminant operand
                let discr_expr = self
                    .vcx
                    .with_span(term.source_info.span, |_| {
                        self.encode_operand_snap(discr, &fragment.versions)
                    })?
                    .downcast_ty();
                let discr_ty = discr.ty(self.body, self.vcx.tcx());
                let discr_ty_out = self.ty_use(discr_ty).expect_primitive();

                // Each target is walked up to the join point of the switch,
                // the immediate reverse dominator of `curr`, where its
                // branch is merged with the others.
                let new_join_point = self.rev_doms.immediate_dominator(curr);
                let mut branches = targets
                    .all_targets()
                    .iter()
                    .map(|target| {
                        let branch = Fragment::starting_at(&fragment.versions);
                        self.encode_cfg(branch, *target, new_join_point)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let otherwise = branches.pop().unwrap();
                let branches = targets
                    .iter()
                    .zip(branches)
                    .map(|((cond_val, _target), branch)| {
                        let cond = self.vcx.mk_eq_expr(
                            discr_ty_out.snap_to_prim(discr_expr),
                            discr_ty_out.expr_from_bits(discr_ty, cond_val).lazy(),
                        );
                        (cond, branch)
                    })
                    .collect();
                if !self.merge(&mut fragment, location, branches, otherwise) {
                    return Ok(None);
                }

                // walk `join` -> `end`
                self.encode_cfg(fragment, new_join_point, join_point)
            }

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
                        self.encode_operand_snap(cond, &fragment.versions)
                    })?
                    .downcast_ty::<vir::Bool>();

                // The rest of the region is the branch taken if the condition
                // has the expected value; the other one fails the assertion
                // and does not complete. As the success branch reaches the end
                // of the region, nothing follows the merge.
                let ok_branch = Fragment::starting_at(&fragment.versions);
                let ok_branch = self.encode_cfg(ok_branch, *target, join_point)?;
                let ok_cond = self.vcx.mk_eq_expr(
                    cond_expr,
                    self.vcx
                        .mk_const_expr(vir::ConstData::Bool(*expected))
                        .downcast_ty(),
                );
                let completes =
                    self.merge(&mut fragment, location, vec![(ok_cond, ok_branch)], None);
                Ok(completes.then_some(fragment))
            }

            mir::TerminatorKind::Return => Ok(Some(fragment)),

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
                let curr_ver = &fragment.versions;
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
                        self.encode_intrinsic(intrinsic, arg_tys, args, curr_ver)
                    } else if let Some(builtin) = PrustiBuiltin::new(def_id, self.gargs(arg_tys)) {
                        match self.encode_prusti_builtin(
                            builtin,
                            def_id,
                            self.gargs(arg_tys),
                            args,
                            term.source_info.span,
                            curr_ver,
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
                                    builtin, arg_tys, args, curr, curr_ver,
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
                            .map(|arg| self.encode_operand_snap(&arg.node, curr_ver))
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

                let expr =
                    self.encode_place_update(curr_ver, *destination, expr?, term.source_info.span)?;
                self.bump_version(&mut fragment, destination.local, expr, location);

                // walk rest of CFG
                self.encode_cfg(fragment, target, join_point)
            }

            k => todo!("terminator kind {k:?}"),
        }
    }

    fn encode_stmt(
        &mut self,
        fragment: &mut Fragment<'vir>,
        stmt: &mir::Statement<'vir>,
        location: mir::Location,
    ) -> EncodeResult<'vir, (), MirPureEnc> {
        // Encoded under the statement's span: expressions pick up the ambient
        // span at creation, which makes error positions inside pure/spec
        // bodies point at the originating source (e.g. the failing conjunct
        // of a spec).
        self.vcx.with_span(stmt.source_info.span, |_| {
            match &stmt.kind {
                &mir::StatementKind::StorageLive(local) => {
                    let new_version = self.bump_version_no_assign(local, location);
                    fragment.versions.insert(local, new_version);
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
                    let curr_ver = &fragment.versions;
                    self.uses.start_assignment();
                    let expr = self
                        .encode_rvalue(curr_ver, rvalue, span)
                        .and_then(|expr| self.encode_place_update(curr_ver, *dest, expr, span));
                    let reads = self.uses.end_assignment();
                    self.bump_version(fragment, dest.local, expr?, location);
                    let version = fragment.versions[&dest.local].index;
                    self.uses.assigned(dest.local, version, reads);
                }
                k => todo!("statement kind {k:?}"),
            }
            Ok(())
        })
    }
}
