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
        fragment: &mut Fragment<'vir>,
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
        fragment.assign(self.vcx, self.encoding_depth, local, new_version, expr);
    }

    pub(super) fn bump_version_init(
        &mut self,
        fragment: &mut Fragment<'vir>,
        local: mir::Local,
        expr: ExprRet<'vir>,
        location: mir::Location,
    ) {
        let new_version = self.bump_version_no_assign(local, location);
        fragment.assign(self.vcx, self.encoding_depth, local, new_version, expr);
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
        fragment: Fragment<'vir>,
        expr: ExprRetAny<'vir, T>,
    ) -> ExprRetAny<'vir, T> {
        fragment.binds.iter().rfold(expr, |expr, bind| match bind {
            Bind::Local(local, version, val) => {
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
            Bind::Phi(phi, val) => self.vcx.mk_let_expr(*phi, val, expr),
        })
    }

    pub(super) fn reify_branch(
        &mut self,
        tuple_ref: &PairUse<'vir>,
        mod_locals: &[mir::Local],
        branch: Option<Fragment<'vir>>,
    ) -> ExprRetAny<'vir, vir::Pair> {
        branch
            .map(|branch| {
                let tuple_args = mod_locals
                    .iter()
                    .map(|local| {
                        let version = branch.versions[local];
                        self.uses.read(*local, version.index);
                        self.mk_local_ex(version)
                    })
                    .collect::<Vec<_>>();
                self.reify_binds(branch, tuple_ref.mk_cons(&tuple_args))
            })
            .unwrap_or_else(|| {
                // A branch that cannot complete (it ends in `unreachable` or
                // a diverging call, or is the failing side of an `Assert`)
                // merges the unreachable snapshots of the locals' types,
                // which assume `false` where they are evaluated; with no
                // locals to merge, nothing is assumed.
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

    /// Merges the branches of a conditional at `location` into `fragment`,
    /// the fragment before the conditional: each of `branches` is taken if
    /// its condition holds, `otherwise` if none holds; the conditions exclude
    /// each other. A branch is the fragment of its region, or `None` if
    /// it cannot complete. The values of the locals that some branch changes,
    /// chosen by the conditions, are bound as one pair, and the locals
    /// continue as its elements. Returns whether some branch completes;
    /// otherwise `fragment` is unchanged.
    pub(super) fn merge(
        &mut self,
        fragment: &mut Fragment<'vir>,
        location: mir::Location,
        branches: Vec<(ExprRetAny<'vir, vir::Bool>, Option<Fragment<'vir>>)>,
        otherwise: Option<Fragment<'vir>>,
    ) -> bool {
        let completing = || {
            branches
                .iter()
                .map(|(_, branch)| branch)
                .chain([&otherwise])
                .flatten()
        };
        if completing().next().is_none() {
            return false;
        }

        // The merged locals are those that some branch changes and that were
        // defined before the conditional.
        let mut mod_locals = completing()
            .flat_map(|branch| &branch.versions)
            .filter(|(local, version)| {
                fragment
                    .versions
                    .get(local)
                    .is_some_and(|before| before.index != version.index)
            })
            .map(|(local, _)| *local)
            .collect::<Vec<_>>();
        mod_locals.sort();
        mod_locals.dedup();
        // A local that some completing branch leaves without a value (declared
        // by `StorageLive` but not assigned) is not merged: MIR reads no local
        // after a join that some path to it leaves uninitialised.
        mod_locals.retain(|local| {
            completing().all(|branch| branch.versions[local].initialised.is_some())
        });
        let mod_tys = mod_locals
            .iter()
            .map(|l| self.get_ty_for_local(*l).as_dyn())
            .collect();
        let tuple_ref = self.deps.require_dep::<PairUseEnc>(mod_tys).unwrap();

        let otherwise = self.reify_branch(&tuple_ref, &mod_locals, otherwise);
        let phi_expr = branches
            .into_iter()
            .fold(otherwise, |expr, (cond, branch)| {
                let branch = self.reify_branch(&tuple_ref, &mod_locals, branch);
                self.vcx.mk_ternary_expr(cond, branch, expr)
            });

        let phi_idx = Version {
            index: self.phi_ctr,
            location,
            initialised: None,
        };
        let phi = self.vcx.mk_local_decl(self.mk_phi(phi_idx), tuple_ref.ty);
        self.phi_ctr += 1;
        fragment.binds.push(Bind::Phi(phi, phi_expr));
        // TODO: maybe this is unnecessary, we could instead use tuple
        //   access directly instead of the locals going forward?
        for (elem_idx, local) in mod_locals.iter().enumerate() {
            let expr = tuple_ref
                .mk_elem(self.vcx.mk_local_ex(phi), elem_idx)
                .downcast_ty();
            self.bump_version(fragment, *local, expr, location);
        }
        true
    }
}
