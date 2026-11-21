//! The versions of locals and the `let`s that bind them.

use super::*;

impl<'vir: 'enc, 'enc> Enc<'vir, 'enc> {
    pub(super) fn mk_local_ex(
        &mut self,
        _local: mir::Local,
        version: Version<'vir>,
    ) -> ExprRet<'vir> {
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
        // check that `local` and `expr` type correspond
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
        // check that `local` and `expr` type correspond
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
                        let version = update.versions.get(local).copied().unwrap_or_else(|| {
                            // TODO: remove (debug)
                            if !curr_ver.contains_key(local) {
                                tracing::error!("unknown version of local! {}", local.as_usize());
                                return Version {
                                    index: 0xff,
                                    ..Default::default()
                                };
                            }
                            curr_ver[local]
                        });

                        self.uses.read(*local, version.index);
                        self.mk_local_ex(*local, version)
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
}
