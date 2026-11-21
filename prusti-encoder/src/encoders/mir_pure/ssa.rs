//! The versions of locals and the `let`s that bind them.

use super::*;

impl<'vir: 'enc, 'enc> Enc<'vir, 'enc> {
    pub(super) fn mk_local_ex(&mut self, version: Version<'vir>) -> ExprRet<'vir> {
        self.vcx.mk_local_ex(version.initialised.unwrap())
    }

    pub(super) fn mk_phi(&self, version: Version<'vir>) -> &'vir str {
        vir::vir_format!(self.vcx, "_{}_phi_{}", self.encoding_depth, version)
    }

    pub(super) fn bump_version(
        &mut self,
        update: &mut Update<'vir>,
        local: mir::Local,
        expr: ExprRet<'vir>,
        location: mir::Location,
    ) {
        if self.version_ctr[local] == 0 {
            // Version 0 signifies "inputs into the expression", as checked in
            // `do_encode_full` to figure out which locals should be encoded
            // and provided by the caller/user of the encoded expression. In
            // most cases, version 0 is not assigned to for non-input locals,
            // because a preceding `StorageLive` bumps the version before the
            // assignment takes place, and so any later reads of that local use
            // at least version 1. However, there are some operations, such as
            // checked binary operations, that do not cause the compiler to
            // emit a `StorageLive` for their target place. This workaround
            // checks for such a case and bumps the version twice.
            //
            // see: https://github.com/rust-lang/rust/issues/67400
            self.bump_version_no_assign(local, location);
        }

        let new_version = self.bump_version_no_assign(local, location);
        update.assign(self.vcx, self.encoding_depth, local, new_version, expr);
    }

    pub(super) fn bump_version_init(
        &mut self,
        update: &mut Update<'vir>,
        local: mir::Local,
        expr: ExprRet<'vir>,
        location: mir::Location,
    ) {
        let new_version = self.bump_version_no_assign(local, location);
        update.assign(self.vcx, self.encoding_depth, local, new_version, expr);
    }

    pub(super) fn bump_version_no_assign(
        &mut self,
        local: mir::Local,
        location: mir::Location,
    ) -> Version<'vir> {
        let index = self.version_ctr[local];
        self.version_ctr[local] += 1;
        Version {
            index,
            location,
            initialised: None,
        }
    }

    pub(super) fn reify_binds<T: CompType>(
        &self,
        update: Update<'vir>,
        expr: ExprRetAny<'vir, T>,
    ) -> ExprRetAny<'vir, T> {
        update.binds.iter().rfold(expr, |expr, bind| match bind {
            UpdateBind::Local(local, version, val) => {
                // skip bindings which were not used
                // TODO: this might optimise away some function calls which
                //   can act as triggers for quantifiers
                if !self.uses.is_read(*local, version.index) {
                    expr
                } else {
                    let decl = version.initialised.unwrap();
                    self.vcx.mk_let_expr(decl, val, expr)
                }
            }
            UpdateBind::Phi(phi, val) => self.vcx.mk_let_expr(*phi, val, expr),
        })
    }

    pub(super) fn reify_branch(
        &mut self,
        tuple_ref: &PairUse<'vir>,
        mod_locals: &[mir::Local],
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
        update: Option<Update<'vir>>,
    ) -> ExprRetAny<'vir, vir::Pair> {
        update
            .map(|update| {
                let tuple_args = mod_locals
                    .iter()
                    .map(|local| {
                        let version = update
                            .versions
                            .get(local)
                            .copied()
                            .unwrap_or_else(|| curr_ver[local]);
                        self.uses.read(*local, version.index);
                        self.mk_local_ex(version)
                    })
                    .collect::<Vec<_>>();
                self.reify_binds(update, tuple_ref.mk_cons(&tuple_args))
            })
            .unwrap_or_else(|| {
                // A branch whose update is `None` cannot complete (it ends in
                // `unreachable` or a diverging call, or is the failing side
                // of an `Assert`). It merges the unreachable snapshots of the
                // locals' types, which assume `false` where they are
                // evaluated; with no locals to merge, nothing is assumed.
                let unreachable = mod_locals
                    .iter()
                    .map(|local| {
                        self.ty_use(self.body.local_decls[*local].ty)
                            .unreachable_to_snap()
                    })
                    .collect::<Vec<_>>();
                tuple_ref.mk_cons(&unreachable)
            })
    }

    /// Merges the branches of a conditional at `location`, which `curr_ver`
    /// enters: each of `branches` is taken if its condition holds and none
    /// before it does, `otherwise` if no condition holds. A branch is the
    /// update of the locals it changes, or `None` if it cannot complete. The
    /// changed locals' values, chosen by the conditions, are bound as one
    /// pair, and the locals continue as its elements in `curr_ver`. Returns
    /// `None` if no branch completes.
    pub(super) fn merge(
        &mut self,
        curr_ver: &mut FxHashMap<mir::Local, Version<'vir>>,
        location: mir::Location,
        branches: Vec<(ExprRetAny<'vir, vir::Bool>, Option<Update<'vir>>)>,
        otherwise: Option<Update<'vir>>,
    ) -> Option<Update<'vir>> {
        let updates = || {
            branches
                .iter()
                .map(|(_, update)| update)
                .chain([&otherwise])
        };
        if updates().all(Option::is_none) {
            return None;
        }

        // The merged locals are those changed by some branch that were
        // defined before the conditional.
        let mut mod_locals = updates()
            .flatten()
            .flat_map(|update| update.versions.keys())
            .filter(|local| curr_ver.contains_key(local))
            .copied()
            .collect::<Vec<_>>();
        mod_locals.sort();
        mod_locals.dedup();
        let mod_tys = mod_locals
            .iter()
            .map(|l| self.get_ty_for_local(*l).as_dyn())
            .collect();
        let tuple_ref = self.deps.require_dep::<PairUseEnc>(mod_tys).unwrap();

        let otherwise = self.reify_branch(&tuple_ref, &mod_locals, curr_ver, otherwise);
        let phi_expr = branches
            .into_iter()
            .fold(otherwise, |expr, (cond, update)| {
                let branch = self.reify_branch(&tuple_ref, &mod_locals, curr_ver, update);
                self.vcx.mk_ternary_expr(cond, branch, expr)
            });

        let phi_idx = Version {
            index: self.phi_ctr,
            location,
            initialised: None,
        };
        let phi = self.vcx.mk_local_decl(self.mk_phi(phi_idx), tuple_ref.ty);
        self.phi_ctr += 1;
        let mut phi_update = Update::new();
        phi_update.binds.push(UpdateBind::Phi(phi, phi_expr));
        // TODO: maybe this is unnecessary, we could instead use tuple
        //   access directly instead of the locals going forward?
        for (elem_idx, local) in mod_locals.iter().enumerate() {
            let expr = tuple_ref
                .mk_elem(self.vcx.mk_local_ex(phi), elem_idx)
                .downcast_ty();
            self.bump_version(&mut phi_update, *local, expr, location);
            curr_ver.insert(*local, phi_update.versions[local]);
        }
        Some(phi_update)
    }
}
