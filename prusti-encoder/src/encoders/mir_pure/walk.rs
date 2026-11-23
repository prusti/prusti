//! The walk over a body's control flow graph.

use super::*;

impl<'vir: 'enc, 'enc> Enc<'vir, 'enc> {
    /// The value of `result_local` at the end of the region from `start` up
    /// to `end` (not included), with `inputs` bound to the values supplied at
    /// instantiation.
    fn encode_region(
        &mut self,
        start: mir::BasicBlock,
        end: mir::BasicBlock,
        inputs: impl Iterator<Item = mir::Local>,
        result_local: mir::Local,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        let mut init = Fragment::default();
        // TODO: a local without `StorageLive` that is first assigned in the
        //   branches of a conditional has no version before it, so it is not
        //   merged (see `Enc::merge`), and a read of it after the join panics.
        let return_version = self.new_version(mir::RETURN_PLACE, mir::Location::START);
        init.versions.insert(mir::RETURN_PLACE, return_version);
        for local in inputs {
            self.bind_input(&mut init, local)?;
        }

        // The body may unconditionally terminate with `unreachable` (e.g. a
        // `match` on an uninhabited type): its result is the unreachable snapshot.
        let Some(res) = self.encode_cfg(init, start, end)? else {
            let result_ty = self.body.local_decls[result_local].ty;
            return Ok(self.ty_use(result_ty)?.unreachable_to_snap());
        };
        let ret_version = res.versions[&result_local];
        self.uses.read(result_local, ret_version.index);

        let ex = self.mk_local_ex(ret_version);
        Ok(self.close_fragment(res, ex))
    }

    pub(super) fn encode_body(&mut self) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        if graph::is_cyclic(&self.body.basic_blocks) {
            return Err(EncodeFullError::unsupported(
                "a loop in pure code".to_string(),
                vec![self.current_span()],
            ));
        }
        self.encode_region(
            mir::START_BLOCK,
            self.rev_doms.end,
            self.body.args_iter(),
            mir::RETURN_PLACE,
        )
    }

    /// The value of the spec block `block`, which builds the spec closure and
    /// calls the builtin with it: the call's result. Every local of the
    /// enclosing body is an input.
    pub(super) fn encode_spec_block(
        &mut self,
        block: mir::BasicBlock,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        let mir::TerminatorKind::Call {
            destination,
            target: Some(target),
            ..
        } = self.body[block].terminator().kind
        else {
            unreachable!("malformed spec-only block: should end in a call terminator");
        };
        assert!(destination.projection.is_empty());
        self.encode_region(
            block,
            target,
            self.body.local_decls.indices().skip(1),
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
        let term = self.body[curr].terminator();
        let location = self.body.terminator_loc(curr);
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

                // Each target is walked up to the join point of the switch,
                // the immediate reverse dominator of `curr`, where its
                // branch is merged with the others. A switch without one, none
                // of whose arms can complete, cannot complete either.
                let Some(new_join_point) = self.rev_doms.dom.immediate_dominator(curr) else {
                    return Ok(None);
                };

                // encode the discriminant operand
                let discr_expr = self
                    .vcx
                    .with_span(term.source_info.span, |_| {
                        self.encode_operand_snap(discr, &fragment.versions)
                    })?
                    .downcast_ty();
                let discr_ty = discr.ty(self.body, self.vcx.tcx());
                let discr_ty_out = self.ty_use(discr_ty)?.expect_primitive();

                let branches = targets
                    .iter()
                    .map(|(cond_val, target)| {
                        let branch = Fragment::starting_at(&fragment.versions);
                        let branch = self.encode_cfg(branch, target, new_join_point)?;
                        let cond = self.vcx.mk_eq_expr(
                            discr_ty_out.snap_to_prim(discr_expr),
                            discr_ty_out.expr_from_bits(discr_ty, cond_val).lazy(),
                        );
                        Ok((cond, branch))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let otherwise = Fragment::starting_at(&fragment.versions);
                let otherwise = self.encode_cfg(otherwise, targets.otherwise(), new_join_point)?;
                let Some(fragment) = self.merge(fragment, location, branches, otherwise)? else {
                    return Ok(None);
                };

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
                self.merge(fragment, location, vec![(ok_cond, ok_branch)], None)
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

                let span = term.source_info.span;
                let curr_ver = &fragment.versions;
                let expr = self
                    .vcx
                    .with_span(span, |_| self.encode_call(curr_ver, func, args, curr, span));
                let expr = self.encode_place_update(curr_ver, *destination, expr?, span)?;
                self.assign(&mut fragment, destination.local, expr, location);

                // walk rest of CFG
                self.encode_cfg(fragment, target, join_point)
            }

            k => Err(EncodeFullError::unsupported(
                format!("terminator `{k:?}` in pure code"),
                vec![term.source_info.span],
            )),
        }
    }

    /// The value of the returning call of `func` with `args` that ends
    /// `block`: of an intrinsic, a Prusti builtin or a pure function.
    fn encode_call(
        &mut self,
        curr_ver: &Versions<'vir>,
        func: &mir::Operand<'vir>,
        args: &[Spanned<mir::Operand<'vir>>],
        block: mir::BasicBlock,
        span: Span,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        let func_ty = func.ty(self.body, self.vcx.tcx());
        let (def_id, arg_tys) = RustSignature::get_def_id_and_caller_substs(func_ty);
        // The bodiless intrinsics of `RustcIntrinsic` are lowered here, as
        // only optimized MIR lowers them (e.g. `ptr_metadata` to
        // `UnOp::PtrMetadata`).
        let intrinsic = self.vcx.tcx().intrinsic(def_id);
        if let Some(intrinsic) = intrinsic.and_then(RustcIntrinsic::from_intrinsic) {
            return self.encode_intrinsic(intrinsic, arg_tys, args, curr_ver);
        }
        let gargs = self.gargs(arg_tys);
        match PrustiBuiltin::new(def_id, gargs) {
            // The pure-only builtins: quantifiers, spec blocks, mode markers.
            Some(PrustiBuiltin::Spec(builtin)) => {
                self.encode_pure_only_builtin(builtin, arg_tys, args, block, curr_ver)
            }
            Some(builtin) => {
                self.encode_prusti_builtin(builtin, def_id, gargs, args, span, curr_ver)
            }
            None if crate::encoders::is_function_pure(def_id, gargs) => {
                let pure_func = self.deps.require_dep_spanned::<FunctionCallEnc>(
                    CallTaskDescription::new(self.context, arg_tys, def_id),
                    span,
                )?;
                let snap_args = args
                    .iter()
                    .map(|arg| self.encode_operand_snap(&arg.node, curr_ver))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(pure_func.call_pure(snap_args))
            }
            None => Err(EncodeFullError::DependencyError(vec![
                DependencyErrorLink::new(
                    MirPureEnc::ENCODER_NAME,
                    format!(
                        "use of impure function `{}` in pure code is not allowed",
                        self.vcx.tcx().def_path_str(def_id)
                    ),
                    vec![span],
                    ErrorKind::Incorrect,
                ),
            ])),
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
                    let new_version = self.new_version(local, location);
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
                    self.assign(fragment, dest.local, expr?, location);
                    let version = fragment.versions[&dest.local].index;
                    self.uses.assigned(dest.local, version, reads);
                }
                k => {
                    return Err(EncodeFullError::unsupported(
                        format!("statement `{k:?}` in pure code"),
                        vec![stmt.source_info.span],
                    ));
                }
            }
            Ok(())
        })
    }
}
