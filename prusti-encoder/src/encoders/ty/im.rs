//! Interior mutability, address-keyed (the PLDI27 design).
//!
//! An `UnsafeCell`'s contents are an interior location: its predicate holds
//! only the place component, and its snapshot has a slot for the contents.
//! Shallow snapshots (what predicates hold) have [`ImUnsetEnc`] in the slot,
//! deep snapshots (what specifications and `#[pure_unstable]` functions see)
//! have the contents.
//!
//! An `#[im_opaque]` struct has a key, the address of a value of it, and an
//! atom, the predicate `p_im_<S>(addr, T..)`: it holds the atoms of the
//! interior-mutable values stored inline in the struct (folded) and the
//! `#[im_perm]` amounts of the contents of its `UnsafeCell` fields. With an
//! `#[im_dependent]`, it has a second key, whose atom `p_im_<S>_dep` holds the
//! contents of its field in full, granted (scaled) by the amount.

use prusti_rustc_interface::{abi, span::def_id::DefId};
use task_encoder::{EncodeFullError, TaskEncoder, TaskEncoderDependencies};
use vir::{CallableIdn, CastType};

use crate::encoders::{
    FunctionCallEnc, TyUseImpureEnc, TyUsePureEnc,
    mir_fn::CallTaskDescription,
    ty::{
        RustTyDecomposition, TySpecifics,
        generics::{GArgsTyEnc, GParams, GenericParamsEnc},
    },
};

/// Whether the struct `did` is `#[im_opaque]`.
pub(crate) fn is_im_opaque(did: DefId) -> bool {
    has_marker(did, "im_opaque")
}

/// Whether the struct `did` reaches another type's atom (`#[im_shares]`).
pub(crate) fn is_im_sharer(did: DefId) -> bool {
    has_marker(did, "im_shares")
}

fn has_marker(did: DefId, name: &str) -> bool {
    vir::with_vcx(|vcx| {
        let attrs = prusti_interface::environment::EnvQuery::new(vcx.tcx()).get_attributes(did);
        prusti_interface::utils::has_prusti_attr(attrs, name)
    })
}

/// The kind of an annotation method (see `prusti_specs::im_type_attribute`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImAmountKind {
    Perm,
    Dependent,
    Holds,
    Shares { dependent: bool },
}

/// The annotation methods of the struct `did`: their kind, the field they
/// are about (if any), and the method.
pub(crate) fn im_amounts(did: DefId) -> Vec<(ImAmountKind, Option<abi::FieldIdx>, DefId)> {
    vir::with_vcx(|vcx| {
        let tcx = vcx.tcx();
        let env = prusti_interface::environment::EnvQuery::new(tcx);
        let fields = &tcx.adt_def(did).non_enum_variant().fields;
        let field_idx = |name: &str| {
            fields
                .iter_enumerated()
                .find(|(_, f)| f.name.as_str() == name)
                .map(|(idx, _)| idx)
                .unwrap_or_else(|| panic!("an interior-mutability annotation names no field `{name}`"))
        };
        let mut out = Vec::new();
        for impl_did in tcx.inherent_impls(did) {
            for item in tcx.associated_item_def_ids(*impl_did) {
                let Some(kind) =
                    prusti_interface::utils::read_prusti_attr("im_amount", env.get_attributes(*item))
                else {
                    continue;
                };
                let mut words = kind.split_whitespace();
                let (kind, field) = match (words.next(), words.next()) {
                    (Some("perm"), Some(f)) => (ImAmountKind::Perm, Some(field_idx(f))),
                    (Some("dependent"), Some(f)) => (ImAmountKind::Dependent, Some(field_idx(f))),
                    (Some("holds"), Some(f)) => (ImAmountKind::Holds, Some(field_idx(f))),
                    (Some("shares"), None) => (ImAmountKind::Shares { dependent: false }, None),
                    (Some("shares"), Some("dependent")) => {
                        (ImAmountKind::Shares { dependent: true }, None)
                    }
                    _ => panic!("malformed interior-mutability annotation `{kind}`"),
                };
                out.push((kind, field, *item));
            }
        }
        out
    })
}

/// The atoms of an `#[im_opaque]` struct: the atom of its key, and that of
/// its dependent key, if it has one.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ImAtomRef<'vir> {
    atom: ImAtomIdn<'vir>,
    dependent: Option<ImAtomIdn<'vir>>,
}
impl<'vir> task_encoder::OutputRefAny for ImAtomRef<'vir> {}

pub(crate) type ImAtomIdn<'vir> = vir::PredicateIdn<'vir, (vir::Ref, vir::ManyTyVal, vir::ManyCSnap)>;

pub(crate) struct ImOpaqueEnc;

impl TaskEncoder for ImOpaqueEnc {
    task_encoder::encoder_cache!(ImOpaqueEnc);
    const ENCODER_NAME: &'static str = "interior mutability opaque type encoder";
    type TaskDescription<'vir> = DefId;
    type OutputRef<'vir> = ImAtomRef<'vir>;
    type OutputFullLocal<'vir> = Vec<vir::Predicate<'vir>>;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        did: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> task_encoder::EncodeFullResult<'vir, Self> {
        vir::with_vcx(|vcx| {
            let params = GParams::from(*did);
            let generics = deps.require_dep::<GenericParamsEnc>(params)?;
            let amounts = im_amounts(*did);
            let ident = |suffix: &str| {
                let name = vir::vir_format_identifier!(
                    vcx,
                    "p_im_{}{suffix}",
                    vir::ViperIdent::from_def_id(vcx, *did)
                );
                vir::PredicateIdn::new(
                    name,
                    (vir::TYPE_REF, generics.ty_args(), generics.const_args()),
                )
            };
            let atoms = ImAtomRef {
                atom: ident(""),
                dependent: amounts
                    .iter()
                    .any(|(kind, ..)| *kind == ImAmountKind::Dependent)
                    .then(|| ident("_dep")),
            };
            deps.emit_output_ref(*did, atoms)?;

            let self_decl = vcx.mk_local_decl("self", vir::TYPE_REF);
            let self_ex = vcx.mk_local_ex(self_decl);
            let identity = vcx.tcx().type_of(*did).instantiate_identity().skip_normalization();
            let ty = RustTyDecomposition::from_ty(identity, params);
            // The atom does not hold the struct's own predicate, so its
            // definition only reads the contents of the atoms it holds.
            let snap = deps.require_ref::<TyUsePureEnc>(ty)?.arbitrary_to_snap();
            let decls = (self_decl, generics.ty_decls(), generics.const_decls());

            let mut body = Vec::new();
            for entry in folded(deps, ty, self_ex, snap)? {
                body.push(entry.acc());
            }
            let inner = deep_with(deps, ty, self_ex, snap, false)?;
            let mut dep_body = Vec::new();
            for (kind, field, method) in &amounts {
                let contents = contents_of(deps, ty, self_ex, field.unwrap())?;
                match kind {
                    ImAmountKind::Perm => {
                        let amount = call_amount(deps, *method, ty, self_ex, inner)?;
                        body.push(contents.acc(amount));
                    }
                    ImAmountKind::Dependent => dep_body.push(contents.acc(vcx.mk_full_perm())),
                    _ => (),
                }
            }
            let mut preds = vec![vcx.mk_predicate(atoms.atom, decls, Some(vcx.mk_conj(&body)))];
            if let Some(dependent) = atoms.dependent {
                preds.push(vcx.mk_predicate(dependent, decls, Some(vcx.mk_conj(&dep_body))));
            }
            Ok((preds, ()))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for preds in Self::all_outputs_local_no_errors(program) {
            for pred in preds {
                program.add_predicate(pred);
            }
        }
    }
}

/// The contents of the `UnsafeCell` field `field` of an `#[im_opaque]`
/// struct, held through the generic predicate (`p_Param`) at every
/// instantiation, as in the atom's definition.
struct Contents<'vir> {
    addr: vir::ExprRef<'vir>,
    ty: crate::encoders::ty::use_impure::TyUseImpure<'vir>,
}

impl<'vir> Contents<'vir> {
    fn acc(&self, amount: vir::ExprPerm<'vir>) -> vir::ExprBool<'vir> {
        vir::with_vcx(|vcx| self.ty.ref_to_pred(vcx, self.addr, Some(amount)))
    }

    fn snap(&self) -> vir::ExprSnap<'vir> {
        self.ty.ref_to_snap(self.addr)
    }
}

fn contents_of<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
    field: abi::FieldIdx,
) -> Result<Contents<'vir>, EncodeFullError<'vir, E>> {
    let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
    let cell_ty = ty.ty.expect_structlike().fields[field.as_usize()].decompose_normalize(ty.args);
    let cell_addr = ty_use.expect_variant_opt(None).fields[field.as_usize()].field_ref(addr);
    let cell_use = deps.require_dep::<TyUsePureEnc>(cell_ty)?;
    let addr = cell_use.expect_variant_opt(None).fields[0].field_ref(cell_addr);
    let contents_ty =
        cell_ty.ty.expect_structlike().fields[0].decompose_context(cell_ty.ty.params, cell_ty.args);
    Ok(Contents {
        addr,
        ty: deps.require_dep::<TyUseImpureEnc>(contents_ty)?,
    })
}

/// The amount computed by the annotation method `method` of the struct `ty`
/// for the value at `addr` with (deep) snapshot `snap`.
fn call_amount<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    method: DefId,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
    snap: vir::ExprSnap<'vir>,
) -> Result<vir::ExprPerm<'vir>, EncodeFullError<'vir, E>> {
    let self_ref = self_ref_snap(deps, method, ty, addr, snap)?;
    let func = deps.require_dep::<FunctionCallEnc>(CallTaskDescription::new(
        ty.args.context(),
        ty.args.args(),
        method,
    ))?;
    let amount = if func.is_pure_unstable() {
        // The value map of the old encoding, unused by deep values.
        let tys = crate::encoders::ty::interior_mut::ImTys::new(deps);
        let empty = vir::with_vcx(|vcx| vcx.mk_map_empty_expr(tys.key.ty, vir::TYPE_PSNAP));
        func.call_pure_unstable(vec![self_ref], empty)
    } else {
        func.call_pure(vec![self_ref])
    };
    // `Real` is represented natively as Viper `Perm`.
    Ok(amount.downcast_ty())
}

/// The `&self` argument of the annotation method `method` of the struct
/// `ty`, for the value at `addr` with snapshot `snap`.
fn self_ref_snap<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    method: DefId,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
    snap: vir::ExprSnap<'vir>,
) -> Result<vir::ExprSnap<'vir>, EncodeFullError<'vir, E>> {
    let self_ty = vir::with_vcx(|vcx| {
        let tcx = vcx.tcx();
        tcx.fn_sig(method)
            .instantiate(tcx, ty.args.args())
            .skip_normalization()
            .skip_binder()
            .inputs()[0]
    });
    let self_ty = RustTyDecomposition::from_ty(self_ty, ty.args.context());
    let metadata_ty = self_ty.ty.ref_data().unwrap().metadata.decompose_normalize(self_ty.args);
    let metadata = match deps.require_dep::<TyUsePureEnc>(metadata_ty)?.zst_to_snap() {
        Some(metadata) => metadata.upcast_ty(),
        None => deps.require_ref::<TyUsePureEnc>(metadata_ty)?.arbitrary_to_snap(),
    };
    Ok(deps
        .require_dep::<TyUsePureEnc>(self_ty)?
        .expect_immref()
        .prim_to_snap(addr, metadata, snap)
        .upcast_ty())
}

/// An atom in an IM footprint.
#[derive(Clone, Copy)]
pub(crate) struct FpEntry<'vir> {
    ty: RustTyDecomposition<'vir>,
    atom: ImAtomIdn<'vir>,
    addr: vir::ExprRef<'vir>,
    tys: &'vir [vir::ExprTyVal<'vir>],
    consts: &'vir [vir::ExprCSnap<'vir>],
    /// The amount of a dependent atom (`None`: the full atom).
    amount: Option<vir::ExprPerm<'vir>>,
    /// Reached through a reference (or a guard): returned to the caller, and
    /// possibly aliased with other such entries.
    pub(crate) reached: bool,
    /// Reached through a shared reference: may alias other shared entries.
    shared: bool,
}

impl<'vir> FpEntry<'vir> {
    fn app(&self, perm: vir::ExprPerm<'vir>) -> vir::PredicateApp<'vir> {
        self.atom.call()(self.addr, self.tys, self.consts)(Some(perm))
    }

    fn amount(&self) -> vir::ExprPerm<'vir> {
        self.amount.unwrap_or_else(|| vir::with_vcx(|vcx| vcx.mk_full_perm()))
    }

    /// The permission to the atom.
    pub(crate) fn acc(&self) -> vir::ExprBool<'vir> {
        vir::with_vcx(|vcx| vcx.mk_predicate_app_expr(self.app(self.amount())))
    }

    /// The snapshots of the contents of the atom's `#[im_perm]`s, read through
    /// the atom (the dependent atom of a key has none of its own).
    pub(crate) fn contents<E: TaskEncoder>(
        &self,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> Result<Vec<vir::ExprSnap<'vir>>, EncodeFullError<'vir, E>> {
        if self.amount.is_some() {
            return Ok(Vec::new());
        }
        let did = self.ty.ty.im_opaque.unwrap();
        let unfold = vir::with_vcx(|vcx| self.app(vcx.mk_wildcard()));
        let mut out = Vec::new();
        for (kind, field, _) in im_amounts(did) {
            if kind == ImAmountKind::Perm {
                let snap = contents_of(deps, self.ty, self.addr, field.unwrap())?.snap();
                out.push(vir::with_vcx(|vcx| vcx.mk_unfolding_expr(unfold, snap)));
            }
        }
        Ok(out)
    }

    fn same_key(&self, other: &Self) -> Option<vir::ExprBool<'vir>> {
        if self.atom.name() != other.atom.name() {
            return None;
        }
        vir::with_vcx(|vcx| {
            let mut eqs = vec![vcx.mk_eq_expr(self.addr, other.addr)];
            eqs.extend(self.tys.iter().zip(other.tys).map(|(a, b)| vcx.mk_eq_expr(*a, *b)));
            Some(vcx.mk_conj(&eqs))
        })
    }
}

/// The atoms of the IM footprint of a value of type `ty` at `addr` with
/// (shallow) snapshot `snap`.
pub(crate) fn footprint<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
    snap: vir::ExprSnap<'vir>,
) -> Result<Vec<FpEntry<'vir>>, EncodeFullError<'vir, E>> {
    let mut out = Vec::new();
    footprint_into(deps, ty, addr, snap, false, false, true, &mut out)?;
    Ok(out)
}

/// The atoms an `#[im_opaque]` struct's atom folds: those of the fields
/// without an amount annotation.
fn folded<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
    snap: vir::ExprSnap<'vir>,
) -> Result<Vec<FpEntry<'vir>>, EncodeFullError<'vir, E>> {
    let did = ty.ty.im_opaque.unwrap();
    let annotated = im_amounts(did)
        .into_iter()
        .filter_map(|(_, field, _)| field)
        .collect::<Vec<_>>();
    let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
    let snap_c = snap.downcast_ty();
    let mut out = Vec::new();
    for (idx, field) in ty.ty.expect_structlike().fields.iter().enumerate() {
        if annotated.contains(&abi::FieldIdx::from_usize(idx)) {
            continue;
        }
        let proj = &ty_use.expect_variant_opt(None).fields[idx];
        footprint_into(
            deps,
            field.decompose_normalize(ty.args),
            proj.field_ref(addr),
            proj.read(snap_c),
            false,
            false,
            true,
            &mut out,
        )?;
    }
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn footprint_into<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
    snap: vir::ExprSnap<'vir>,
    reached: bool,
    shared: bool,
    with_dependent: bool,
    out: &mut Vec<FpEntry<'vir>>,
) -> Result<(), EncodeFullError<'vir, E>> {
    if let Some(did) = ty.ty.im_opaque {
        let atoms = deps.require_ref::<ImOpaqueEnc>(did)?;
        let args = deps.require_dep::<GArgsTyEnc>(ty.args)?;
        let entry = FpEntry {
            ty,
            atom: atoms.atom,
            addr,
            tys: args.get_ty(),
            consts: args.get_const(),
            amount: None,
            reached,
            shared,
        };
        out.push(entry);
        if let Some(dependent) = atoms.dependent
            && with_dependent
        {
            let (_, _, method) = im_amounts(did)
                .into_iter()
                .find(|(kind, ..)| *kind == ImAmountKind::Dependent)
                .unwrap();
            let deep = deep_with(deps, ty, addr, snap, true)?;
            out.push(FpEntry {
                atom: dependent,
                amount: Some(call_amount(deps, method, ty, addr, deep)?),
                ..entry
            });
        }
        return Ok(());
    }
    if let Some(did) = ty.ty.im_annotated {
        let shares = im_amounts(did)
            .into_iter()
            .find_map(|(kind, _, method)| match kind {
                ImAmountKind::Shares { dependent } => Some((dependent, method)),
                _ => None,
            });
        if let Some((dependent, method)) = shares {
            // A value of the struct reaches the shared atoms instead of those
            // of its own fields.
            let self_ref = self_ref_snap(deps, method, ty, addr, snap)?;
            let func = deps.require_dep::<FunctionCallEnc>(CallTaskDescription::new(
                ty.args.context(),
                ty.args.args(),
                method,
            ))?;
            let target = func.call_pure(vec![self_ref]);
            let target_ty = vir::with_vcx(|vcx| {
                let tcx = vcx.tcx();
                tcx.fn_sig(method)
                    .instantiate(tcx, ty.args.args())
                    .skip_normalization()
                    .skip_binder()
                    .output()
            });
            let target_ty = RustTyDecomposition::from_ty(target_ty, ty.args.context());
            let ref_use = deps.require_dep::<TyUsePureEnc>(target_ty)?;
            let target_c = target.downcast_ty();
            let ref_data = ref_use.expect_immref();
            let TySpecifics::ImmRef(data) = &target_ty.ty.specifics else {
                unreachable!("`#[im_shares]` returns a shared reference")
            };
            footprint_into(
                deps,
                data.referent.decompose_normalize(target_ty.args),
                ref_data.addr_access(target_c),
                ref_data.value_access(target_c),
                true,
                true,
                dependent,
                out,
            )?;
            return Ok(());
        }
    }
    match &ty.ty.specifics {
        TySpecifics::StructLike(data)
            if data.fields.iter().all(|f| {
                matches!(f.address, crate::encoders::ty::RustFieldAddress::Constant)
            }) =>
        {
            let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
            let snap_c = snap.downcast_ty();
            for (idx, field) in data.fields.iter().enumerate() {
                let field_ty = field.decompose_normalize(ty.args);
                let proj = &ty_use.expect_variant_opt(None).fields[idx];
                footprint_into(
                    deps,
                    field_ty,
                    proj.field_ref(addr),
                    proj.read(snap_c),
                    reached,
                    shared,
                    with_dependent,
                    out,
                )?;
            }
        }
        TySpecifics::ImmRef(data) => {
            let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
            let snap_c = snap.downcast_ty();
            let referent = data.referent.decompose_normalize(ty.args);
            let ref_use = ty_use.expect_immref();
            footprint_into(
                deps,
                referent,
                ref_use.addr_access(snap_c),
                ref_use.value_access(snap_c),
                true,
                true,
                with_dependent,
                out,
            )?;
        }
        _ => (),
    }
    Ok(())
}

/// The permissions to `entries`. Owned entries are distinct from every other
/// entry (assumed on inhale); entries reached through shared references are
/// granted once per key.
pub(crate) fn footprint_assertion<'vir>(entries: &[FpEntry<'vir>]) -> vir::ExprBool<'vir> {
    vir::with_vcx(|vcx| {
        let mut conjs = Vec::new();
        let mut distinct = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            let mut perm = entry.amount();
            for earlier in &entries[..i] {
                let Some(same) = entry.same_key(earlier) else { continue };
                if entry.shared && earlier.shared {
                    perm = vcx.mk_ternary_expr(same, vcx.mk_no_perm(), perm);
                } else {
                    distinct.push(
                        vcx.mk_unary_op_expr(vir::UnOpKind::Not, same.upcast_ty())
                            .downcast_ty::<vir::Bool>(),
                    );
                }
            }
            conjs.push(vcx.mk_predicate_app_expr(entry.app(perm)));
        }
        if !distinct.is_empty() {
            conjs.insert(
                0,
                vcx.mk_inhale_exhale_expr(vcx.mk_conj(&distinct), vcx.mk_bool::<true>()),
            );
        }
        vcx.mk_conj(&conjs)
    })
}

/// The deep snapshot of a value of type `ty` at `addr` with shallow snapshot
/// `snap`: the contents of the interior locations it reaches fill the
/// `UnsafeCell` slots. Reads the atoms (needs their permissions).
pub(crate) fn deep_snap<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
    snap: vir::ExprSnap<'vir>,
) -> Result<vir::ExprSnap<'vir>, EncodeFullError<'vir, E>> {
    deep_with(deps, ty, addr, snap, true)
}

/// Like [`deep_snap`]; for an `#[im_opaque]` `ty`, `own` says whether its own
/// atom is held (and read) or the read happens inside its definition, where
/// only the atoms it folds can be read.
fn deep_with<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
    snap: vir::ExprSnap<'vir>,
    own: bool,
) -> Result<vir::ExprSnap<'vir>, EncodeFullError<'vir, E>> {
    let vcx = vir::with_vcx(|vcx| vcx);
    if !reaches_im(ty) {
        return Ok(snap);
    }
    if let Some(did) = ty.ty.im_opaque {
        let atoms = deps.require_ref::<ImOpaqueEnc>(did)?;
        let args = deps.require_dep::<GArgsTyEnc>(ty.args)?;
        let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
        let variant = ty_use.expect_variant_opt(None);
        let fields = &ty.ty.expect_structlike().fields;
        let snap_c = snap.downcast_ty();
        let amounts = im_amounts(did);
        // The amounts read the folded atoms only.
        let inner = if own {
            Some(deep_with(deps, ty, addr, snap, false)?)
        } else {
            None
        };
        let mut field_snaps = Vec::with_capacity(fields.len());
        for (idx, field) in fields.iter().enumerate() {
            let proj = &variant.fields[idx];
            let field_ty = field.decompose_normalize(ty.args);
            let field_snap = proj.read(snap_c);
            let perm = amounts.iter().find(|(kind, field, _)| {
                *kind == ImAmountKind::Perm && field.map(|f| f.as_usize()) == Some(idx)
            });
            let annotated = amounts
                .iter()
                .any(|(_, field, _)| field.map(|f| f.as_usize()) == Some(idx));
            field_snaps.push(match (perm, inner) {
                (Some((_, _, method)), Some(inner)) => {
                    let contents = contents_of(deps, ty, addr, abi::FieldIdx::from_usize(idx))?;
                    let amount = call_amount(deps, *method, ty, addr, inner)?;
                    let unset = deps.require_dep::<ImUnsetEnc>(())?;
                    let cell_ty_args = deps.require_dep::<GArgsTyEnc>(field_ty.args)?;
                    let slot = vcx.mk_ternary_expr(
                        vcx.mk_bin_op_expr(vir::BinOpKind::CmpGt, amount, vcx.mk_no_perm())
                            .downcast_ty(),
                        contents.snap(),
                        unset.call()(cell_ty_args.get_ty()[0]).upcast_ty(),
                    );
                    let cell_use = deps.require_dep::<TyUsePureEnc>(field_ty)?;
                    cell_use
                        .expect_variant_opt(None)
                        .field_snaps_to_snap_generic(vec![slot])
                        .upcast_ty()
                }
                _ if annotated => field_snap,
                _ => deep_with(deps, field_ty, proj.field_ref(addr), field_snap, true)?,
            });
        }
        let value = variant.field_snaps_to_snap(field_snaps).upcast_ty();
        if !own {
            return Ok(value);
        }
        let unfold = atoms.atom.call()(addr, args.get_ty(), args.get_const())(Some(
            vcx.mk_wildcard(),
        ));
        return Ok(vcx.mk_unfolding_expr(unfold, value));
    }
    match &ty.ty.specifics {
        TySpecifics::StructLike(data)
            if !data.fields.is_empty()
                && data.fields.iter().all(|f| {
                    matches!(f.address, crate::encoders::ty::RustFieldAddress::Constant)
                }) =>
        {
            let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
            let variant = ty_use.expect_variant_opt(None);
            let snap_c = snap.downcast_ty();
            let mut field_snaps = Vec::with_capacity(data.fields.len());
            for (idx, field) in data.fields.iter().enumerate() {
                let proj = &variant.fields[idx];
                field_snaps.push(deep_with(
                    deps,
                    field.decompose_normalize(ty.args),
                    proj.field_ref(addr),
                    proj.read(snap_c),
                    true,
                )?);
            }
            Ok(variant.field_snaps_to_snap(field_snaps).upcast_ty())
        }
        TySpecifics::ImmRef(data) => {
            let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
            let snap_c = snap.downcast_ty();
            let referent = data.referent.decompose_normalize(ty.args);
            let ref_use = ty_use.expect_immref();
            let r_addr = ref_use.addr_access(snap_c);
            let value = deep_with(deps, referent, r_addr, ref_use.value_access(snap_c), true)?;
            Ok(ref_use
                .prim_to_snap(r_addr, ref_use.metadata_access(snap_c), value)
                .upcast_ty())
        }
        _ => Ok(snap),
    }
}

/// Whether a value of `ty` reaches an `#[im_opaque]` value, i.e. whether its
/// deep snapshot differs from its shallow one.
pub(crate) fn reaches_im(ty: RustTyDecomposition<'_>) -> bool {
    fn go<'tcx>(
        ty: RustTyDecomposition<'tcx>,
        seen: &mut Vec<RustTyDecomposition<'tcx>>,
    ) -> bool {
        if ty.ty.im_opaque.is_some() || ty.ty.im_annotated.is_some() {
            return true;
        }
        if seen.contains(&ty) {
            return false;
        }
        seen.push(ty);
        match &ty.ty.specifics {
            TySpecifics::StructLike(data) => data
                .fields
                .iter()
                .any(|f| go(f.decompose_normalize(ty.args), seen)),
            TySpecifics::ImmRef(data) => go(data.referent.decompose_normalize(ty.args), seen),
            _ => false,
        }
    }
    go(ty, &mut Vec::new())
}

/// `im_unset(T): s_Param`, the slot of a shallow `UnsafeCell<T>` snapshot.
pub(crate) struct ImUnsetEnc;

pub(crate) type ImUnset<'vir> = vir::FunctionIdn<'vir, vir::TyVal, vir::PSnap>;

impl TaskEncoder for ImUnsetEnc {
    task_encoder::encoder_cache!(ImUnsetEnc);
    const ENCODER_NAME: &'static str = "interior mutability unset slot encoder";
    type TaskDescription<'vir> = ();
    type OutputFullDependency<'vir> = ImUnset<'vir>;
    type OutputFullLocal<'vir> = vir::Domain<'vir>;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        _task_key: &Self::TaskKey<'vir>,
        deps: &mut task_encoder::TaskEncoderDependencies<'vir, Self>,
    ) -> task_encoder::EncodeFullResult<'vir, Self> {
        deps.emit_output_ref((), ())?;
        vir::with_vcx(|vcx| {
            let unset: ImUnset<'vir> = vir::FunctionIdn::new(
                vir::vir_format_identifier!(vcx, "im_unset"),
                vir::TYPE_TYVAL,
                vir::TYPE_PSNAP,
            );
            let domain = vcx.mk_domain(
                vir::ViperIdent::new("def_im_unset"),
                &[],
                &[],
                vcx.alloc_slice(&[vcx.mk_domain_function(unset, false, None)]),
                None,
            );
            Ok((domain, unset))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for domain in Self::all_outputs_local_no_errors(program) {
            program.add_domain(domain);
        }
    }
}
