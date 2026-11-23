//! The versions of locals and the `let`s that bind them.

use super::*;

/// A version of a local: one of the values the local takes in the `let`
/// chain.
#[derive(Debug, Clone, Copy)]
pub(super) struct Version<'vir> {
    /// Distinguishes the versions of a local. Version 0 of an input is the
    /// value supplied at instantiation (see `ExprInput::inputs`).
    pub(super) index: usize,
    /// Where the version is created, which only serves its name.
    pub(super) location: mir::Location,
    /// The variable bound to the version, or `None` if the local is declared
    /// (by `StorageLive`) but not yet assigned.
    pub(super) decl: Option<vir::LocalDeclSnap<'vir>>,
}

/// The part of a variable's name that identifies a version of a local or a
/// merge: the index, which makes it unique, after the location that creates
/// it unless short names are asked for (`short_viper_names`).
fn name_suffix(vcx: &vir::VirCtxt<'_>, location: mir::Location, index: usize) -> impl fmt::Display {
    let short = vcx.ident_style == vir::IdentStyle::ItemName;
    fmt::from_fn(move |f| {
        if short {
            write!(f, "{index}")
        } else {
            write!(
                f,
                "{:?}_s{}_i{}",
                location.block, location.statement_index, index
            )
        }
    })
}

/// The current version of each local.
pub(super) type Versions<'vir> = FxHashMap<mir::Local, Version<'vir>>;

/// A part of the `let` chain that encodes a body: the bindings that a region
/// of the body adds, in order, and the version of each local at the end of
/// the region. The walk over a region extends the fragment it starts with
/// (see `Enc::encode_cfg`); a region that cannot complete has none.
#[derive(Debug, Default)]
pub(super) struct Fragment<'vir> {
    pub(super) binds: Vec<Bind<'vir>>,
    pub(super) versions: Versions<'vir>,
}

/// A binding of the `let` chain: of a version of a local, or of the pair that
/// merges the branches of a conditional (see `Enc::merge`).
#[derive(Debug)]
pub(super) enum Bind<'vir> {
    Local(mir::Local, Version<'vir>, ExprRet<'vir>),
    Phi(vir::LocalDeclPair<'vir>, ExprRetAny<'vir, vir::Pair>),
}

impl<'vir> Fragment<'vir> {
    /// The empty fragment of a branch that starts at `versions`.
    pub(super) fn starting_at(versions: &Versions<'vir>) -> Self {
        Self {
            binds: Vec::new(),
            versions: versions.clone(),
        }
    }
}

/// The versions of locals that the encoded expression reads, directly or
/// through the value of a version it reads. Only the bindings of these
/// versions are emitted, so that a binding nothing reads emits nothing its
/// value reads either. This is required, not only an optimisation: every
/// input has a binding of version 0, but only the inputs read are supplied
/// (see `MirPureEncOutput::inputs`). The pair of a merge is always bound,
/// and reads the versions it merges.
#[derive(Default)]
pub(super) struct Uses {
    /// The versions read. Closed under `pending`: once a version is read, so
    /// are the versions its value reads.
    pub(super) read: FxHashSet<(mir::Local, usize)>,
    /// For each assigned version not read yet, the versions its value reads.
    pending: FxHashMap<(mir::Local, usize), Vec<(mir::Local, usize)>>,
    /// While an assignment is encoded, the versions its value reads so far.
    assignment: Option<Vec<(mir::Local, usize)>>,
}

impl Uses {
    /// Records a read of version `index` of `local`: by the value of the
    /// assignment being encoded, if any, or else by the expression itself.
    pub(super) fn read(&mut self, local: mir::Local, index: usize) {
        if let Some(reads) = &mut self.assignment {
            reads.push((local, index));
        } else if self.read.insert((local, index))
            && let Some(reads) = self.pending.remove(&(local, index))
        {
            for (local, index) in reads {
                self.read(local, index);
            }
        }
    }

    /// Starts recording the reads of an assignment's value.
    pub(super) fn start_assignment(&mut self) {
        debug_assert!(self.assignment.is_none(), "assignments do not nest");
        self.assignment = Some(Vec::new());
    }

    /// Stops recording the reads of an assignment's value, returning them.
    pub(super) fn end_assignment(&mut self) -> Vec<(mir::Local, usize)> {
        self.assignment.take().unwrap()
    }

    /// Records that version `index` of `local` is assigned a value that
    /// reads `reads`; they count as read once that version is.
    pub(super) fn assigned(
        &mut self,
        local: mir::Local,
        index: usize,
        reads: Vec<(mir::Local, usize)>,
    ) {
        self.pending.insert((local, index), reads);
    }

    fn is_read(&self, local: mir::Local, index: usize) -> bool {
        self.read.contains(&(local, index))
    }
}

impl<'vir: 'enc, 'enc> Enc<'vir, 'enc> {
    pub(super) fn mk_local_ex(&mut self, version: Version<'vir>) -> ExprRet<'vir> {
        self.vcx.mk_local_ex(version.decl.unwrap())
    }

    /// A new version of `local`, created at `location`, without a value.
    pub(super) fn new_version(
        &mut self,
        local: mir::Local,
        location: mir::Location,
    ) -> Version<'vir> {
        let index = self.version_ctr[local];
        self.version_ctr[local] += 1;
        Version {
            index,
            location,
            decl: None,
        }
    }

    /// Extends `fragment` with a new version of `local`, created at
    /// `location`, bound to `expr`.
    pub(super) fn assign(
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
            self.new_version(local, location);
        }

        let version = self.new_version(local, location);
        self.bind(fragment, local, version, expr);
    }

    /// Extends `fragment` with version 0 of the input `local`, bound to the
    /// value supplied at instantiation.
    pub(super) fn bind_input(
        &mut self,
        fragment: &mut Fragment<'vir>,
        local: mir::Local,
    ) -> EncodeResult<'vir, (), MirPureEnc> {
        let expr = self.input(local)?;
        let version = self.new_version(local, mir::Location::START);
        self.bind(fragment, local, version, expr);
        Ok(())
    }

    fn bind(
        &mut self,
        fragment: &mut Fragment<'vir>,
        local: mir::Local,
        mut version: Version<'vir>,
        expr: ExprRet<'vir>,
    ) {
        let name = vir::vir_format!(
            self.vcx,
            "_{}_{}s_{}",
            self.encoding_depth,
            local.as_usize(),
            name_suffix(self.vcx, version.location, version.index)
        );
        version.decl = Some(self.vcx.mk_local_decl(name, expr.ty()));
        fragment.binds.push(Bind::Local(local, version, expr));
        fragment.versions.insert(local, version);
    }

    /// Wraps the bindings of `fragment` around `expr`. Whether a binding is
    /// read must be final by then: true at the end of the body, and for the
    /// fragment of a branch, whose versions only its merge's pair reads.
    pub(super) fn close_fragment<T: CompType>(
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
                    let decl = version.decl.unwrap();
                    self.vcx.mk_let_expr(decl, val, expr)
                }
            }
            Bind::Phi(phi, val) => self.vcx.mk_let_expr(*phi, val, expr),
        })
    }

    /// The pair of the values of `mod_locals` at the end of `branch`.
    pub(super) fn branch_pair(
        &mut self,
        tuple_ref: &PairUse<'vir>,
        mod_locals: &[mir::Local],
        branch: Option<Fragment<'vir>>,
    ) -> EncodeResult<'vir, ExprRetAny<'vir, vir::Pair>, MirPureEnc> {
        let Some(branch) = branch else {
            // A branch that cannot complete (it ends in `unreachable` or a
            // diverging call, or is the failing side of an `Assert`) merges
            // the unreachable snapshots of the locals' types, whose
            // precondition `false` requires the branch to be unreachable;
            // with no locals to merge, nothing is checked.
            let unreachable = mod_locals
                .iter()
                .map(|local| {
                    let ty = self.ty_use(self.body.local_decls[*local].ty)?;
                    Ok(ty.unreachable_to_snap())
                })
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(tuple_ref.mk_cons(&unreachable));
        };
        let tuple_args = mod_locals
            .iter()
            .map(|local| {
                let version = branch.versions[local];
                self.uses.read(*local, version.index);
                self.mk_local_ex(version)
            })
            .collect::<Vec<_>>();
        Ok(self.close_fragment(branch, tuple_ref.mk_cons(&tuple_args)))
    }

    /// Merges the branches of a conditional at `location` into `fragment`,
    /// the fragment before the conditional: each of `branches` is taken if
    /// its condition holds, `otherwise` if none holds; the conditions exclude
    /// each other. A branch is the fragment of its region, or `None` if
    /// it cannot complete. The values of the locals that some branch changes,
    /// chosen by the conditions, are bound as one pair, and the locals
    /// continue as its elements. Returns the extended fragment, or `None` if
    /// no branch completes.
    pub(super) fn merge(
        &mut self,
        mut fragment: Fragment<'vir>,
        location: mir::Location,
        branches: Vec<(ExprRetAny<'vir, vir::Bool>, Option<Fragment<'vir>>)>,
        otherwise: Option<Fragment<'vir>>,
    ) -> EncodeResult<'vir, Option<Fragment<'vir>>, MirPureEnc> {
        let completing = || {
            branches
                .iter()
                .map(|(_, branch)| branch)
                .chain([&otherwise])
                .flatten()
        };
        if completing().next().is_none() {
            return Ok(None);
        }

        // The merged locals are those that some branch changes and that were
        // defined before the conditional, except one that some completing
        // branch leaves without a value (declared by `StorageLive` but not
        // assigned): MIR reads no local after a join that some path to it
        // leaves uninitialised.
        let mod_locals = completing()
            .flat_map(|branch| &branch.versions)
            .filter(|(local, version)| {
                fragment
                    .versions
                    .get(local)
                    .is_some_and(|before| before.index != version.index)
            })
            .map(|(local, _)| *local)
            .sorted()
            .dedup()
            .filter(|local| completing().all(|branch| branch.versions[local].decl.is_some()))
            .collect::<Vec<_>>();
        let mod_tys = mod_locals
            .iter()
            .map(|l| Ok(self.get_ty_for_local(*l)?.as_dyn()))
            .collect::<Result<_, _>>()?;
        let tuple_ref = self.deps.require_dep::<PairUseEnc>(mod_tys)?;

        let mut phi_expr = self.branch_pair(&tuple_ref, &mod_locals, otherwise)?;
        for (cond, branch) in branches {
            let branch = self.branch_pair(&tuple_ref, &mod_locals, branch)?;
            phi_expr = self.vcx.mk_ternary_expr(cond, branch, phi_expr);
        }

        let phi_name = vir::vir_format!(
            self.vcx,
            "_{}_phi_{}",
            self.encoding_depth,
            name_suffix(self.vcx, location, self.phi_ctr)
        );
        let phi = self.vcx.mk_local_decl(phi_name, tuple_ref.ty);
        self.phi_ctr += 1;
        fragment.binds.push(Bind::Phi(phi, phi_expr));
        // TODO: maybe this is unnecessary, we could instead use tuple
        //   access directly instead of the locals going forward?
        for (elem_idx, local) in mod_locals.iter().enumerate() {
            let expr = tuple_ref
                .mk_elem(self.vcx.mk_local_ex(phi), elem_idx)
                .downcast_ty();
            self.assign(&mut fragment, *local, expr, location);
        }
        Ok(Some(fragment))
    }
}
