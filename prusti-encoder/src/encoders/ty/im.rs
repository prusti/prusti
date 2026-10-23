//! Interior mutability, address-keyed (the PLDI27 design).
//!
//! An `UnsafeCell`'s contents are an interior location: its predicate holds
//! only the place component, and its snapshot has a slot for the contents.
//! Shallow snapshots (what predicates hold) have [`ImUnsetEnc`] in the slot,
//! deep snapshots (what specifications and `#[pure_unstable]` functions see)
//! have the contents.

use prusti_rustc_interface::{abi, span::def_id::DefId};
use task_encoder::{EncodeFullError, TaskEncoder, TaskEncoderDependencies};
use vir::{CallableIdn, CastType};

use crate::encoders::{
    TyUseImpureEnc, TyUsePureEnc,
    ty::{
        RustTyDecomposition, TySpecifics,
        generics::{GParams, GenericParamsEnc},
    },
};

/// Whether the struct `did` is `#[im_opaque]`.
pub(crate) fn is_im_opaque(did: DefId) -> bool {
    vir::with_vcx(|vcx| {
        let attrs = prusti_interface::environment::EnvQuery::new(vcx.tcx()).get_attributes(did);
        prusti_interface::utils::read_prusti_attr("im_opaque", attrs).is_some()
    })
}

/// A permission amount in an `#[im_perm]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ImAmount {
    Write,
}

/// The `#[im_perm(FIELD, AMOUNT)]`s of an `#[im_opaque]` struct.
fn im_perms(did: DefId) -> Vec<(abi::FieldIdx, ImAmount)> {
    vir::with_vcx(|vcx| {
        let tcx = vcx.tcx();
        let attrs = prusti_interface::environment::EnvQuery::new(tcx).get_attributes(did);
        let fields = &tcx.adt_def(did).non_enum_variant().fields;
        prusti_interface::utils::read_prusti_attrs("im_perm", attrs)
            .map(|args| {
                let (field, amount) = args
                    .split_once(',')
                    .unwrap_or_else(|| panic!("malformed `#[im_perm({args})]`"));
                let field = field.trim();
                let (idx, _) = fields
                    .iter_enumerated()
                    .find(|(_, f)| f.name.as_str() == field)
                    .unwrap_or_else(|| panic!("`#[im_perm]` names no field `{field}`"));
                let amount: String = amount.split_whitespace().collect();
                let amount = match amount.as_str() {
                    "Real::WRITE" => ImAmount::Write,
                    other => todo!("`#[im_perm]` amount `{other}`"),
                };
                (idx, amount)
            })
            .collect()
    })
}

/// The atom of an `#[im_opaque]` struct: `p_im_<S>(self, T..)`, holding its
/// `#[im_perm]`s of the contents of its `UnsafeCell` fields.
pub(crate) struct ImOpaqueEnc;

pub(crate) type ImAtomIdn<'vir> = vir::PredicateIdn<'vir, (vir::Ref, vir::ManyTyVal, vir::ManyCSnap)>;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ImAtomRef<'vir>(ImAtomIdn<'vir>);
impl<'vir> task_encoder::OutputRefAny for ImAtomRef<'vir> {}

impl TaskEncoder for ImOpaqueEnc {
    task_encoder::encoder_cache!(ImOpaqueEnc);
    const ENCODER_NAME: &'static str = "interior mutability opaque type encoder";
    type TaskDescription<'vir> = DefId;
    type OutputRef<'vir> = ImAtomRef<'vir>;
    type OutputFullLocal<'vir> = vir::Predicate<'vir>;

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
            let name = vir::vir_format_identifier!(
                vcx,
                "p_im_{}",
                vir::ViperIdent::from_def_id(vcx, *did)
            );
            let idn: ImAtomIdn<'vir> = vir::PredicateIdn::new(
                name,
                (vir::TYPE_REF, generics.ty_args(), generics.const_args()),
            );
            deps.emit_output_ref(*did, ImAtomRef(idn))?;

            let self_decl = vcx.mk_local_decl("self", vir::TYPE_REF);
            let self_ex = vcx.mk_local_ex(self_decl);
            let identity = vcx.tcx().type_of(*did).instantiate_identity().skip_normalization();
            let ty = RustTyDecomposition::from_ty(identity, params);
            let body = contents_perms(deps, ty, self_ex)?
                .into_iter()
                .map(|(pred, _)| pred)
                .collect::<Vec<_>>();
            let pred = vcx.mk_predicate(
                idn,
                (self_decl, generics.ty_decls(), generics.const_decls()),
                Some(vcx.mk_conj(&body)),
            );
            Ok((pred, ()))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for pred in Self::all_outputs_local_no_errors(program) {
            program.add_predicate(pred);
        }
    }
}

/// For an `#[im_opaque]` struct `ty` at `addr`: per `#[im_perm]`, the
/// permission to the contents of the `UnsafeCell` field, and the contents'
/// snapshot (heap-dependent, readable where the permission is held).
fn contents_perms<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
) -> Result<Vec<(vir::ExprBool<'vir>, (abi::FieldIdx, vir::ExprSnap<'vir>))>, EncodeFullError<'vir, E>>
{
    let did = ty.ty.im_opaque.expect("not an `#[im_opaque]` type");
    let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
    let fields = &ty.ty.expect_structlike().fields;
    let mut out = Vec::new();
    for (idx, amount) in im_perms(did) {
        let cell_ty = fields[idx.as_usize()].decompose_normalize(ty.args);
        let cell_addr = ty_use.expect_variant_opt(None).fields[idx.as_usize()].field_ref(addr);
        let cell_use = deps.require_dep::<TyUsePureEnc>(cell_ty)?;
        let contents_addr = cell_use.expect_variant_opt(None).fields[0].field_ref(cell_addr);
        // The contents are held through the generic predicate (`p_Param`) at
        // every instantiation, as in the atom's definition.
        let contents_ty =
            cell_ty.ty.expect_structlike().fields[0].decompose_context(cell_ty.ty.params, cell_ty.args);
        let contents_use = deps.require_dep::<TyUseImpureEnc>(contents_ty)?;
        let perm = vir::with_vcx(|vcx| match amount {
            ImAmount::Write => vcx.mk_full_perm(),
        });
        let pred = vir::with_vcx(|vcx| contents_use.ref_to_pred(vcx, contents_addr, Some(perm)));
        out.push((pred, (idx, contents_use.ref_to_snap(contents_addr))));
    }
    Ok(out)
}

/// An atom in an IM footprint.
#[derive(Clone, Copy)]
pub(crate) struct FpEntry<'vir> {
    ty: RustTyDecomposition<'vir>,
    atom: ImAtomIdn<'vir>,
    addr: vir::ExprRef<'vir>,
    tys: &'vir [vir::ExprTyVal<'vir>],
    consts: &'vir [vir::ExprCSnap<'vir>],
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

    /// The full permission to the atom.
    pub(crate) fn acc(&self) -> vir::ExprBool<'vir> {
        vir::with_vcx(|vcx| vcx.mk_predicate_app_expr(self.app(vcx.mk_full_perm())))
    }

    /// The snapshots of the atom's contents (one per `#[im_perm]`), read
    /// through the atom.
    pub(crate) fn contents<E: TaskEncoder>(
        &self,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> Result<Vec<vir::ExprSnap<'vir>>, EncodeFullError<'vir, E>> {
        let unfold = vir::with_vcx(|vcx| self.app(vcx.mk_wildcard()));
        Ok(contents_perms(deps, self.ty, self.addr)?
            .into_iter()
            .map(|(_, (_, snap))| vir::with_vcx(|vcx| vcx.mk_unfolding_expr(unfold, snap)))
            .collect())
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
    footprint_into(deps, ty, addr, snap, false, false, &mut out)?;
    Ok(out)
}

fn footprint_into<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
    snap: vir::ExprSnap<'vir>,
    reached: bool,
    shared: bool,
    out: &mut Vec<FpEntry<'vir>>,
) -> Result<(), EncodeFullError<'vir, E>> {
    if let Some(did) = ty.ty.im_opaque {
        let atom = deps.require_ref::<ImOpaqueEnc>(did)?.0;
        let args = deps.require_dep::<crate::encoders::ty::generics::GArgsTyEnc>(ty.args)?;
        out.push(FpEntry {
            ty,
            atom,
            addr,
            tys: args.get_ty(),
            consts: args.get_const(),
            reached,
            shared,
        });
        return Ok(());
    }
    match &ty.ty.specifics {
        TySpecifics::StructLike(data) if data.fields.iter().all(|f| {
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
            let mut perm = vcx.mk_full_perm();
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
    let vcx = vir::with_vcx(|vcx| vcx);
    if !reaches_im(ty) {
        return Ok(snap);
    }
    if let Some(did) = ty.ty.im_opaque {
        let atom = deps.require_ref::<ImOpaqueEnc>(did)?.0;
        let args = deps.require_dep::<crate::encoders::ty::generics::GArgsTyEnc>(ty.args)?;
        let contents = contents_perms(deps, ty, addr)?;
        let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
        let variant = ty_use.expect_variant_opt(None);
        let fields = &ty.ty.expect_structlike().fields;
        let snap_c = snap.downcast_ty();
        let mut field_snaps = Vec::with_capacity(fields.len());
        for (idx, field) in fields.iter().enumerate() {
            let proj = &variant.fields[idx];
            let field_ty = field.decompose_normalize(ty.args);
            let field_addr = proj.field_ref(addr);
            let field_snap = proj.read(snap_c);
            let slot = contents
                .iter()
                .find(|(_, (fidx, _))| fidx.as_usize() == idx)
                .map(|(_, (_, s))| *s);
            field_snaps.push(match slot {
                Some(slot) => {
                    let unfold = atom.call()(addr, args.get_ty(), args.get_const())(Some(
                        vcx.mk_wildcard(),
                    ));
                    let slot = vcx.mk_unfolding_expr(unfold, slot);
                    let cell_use = deps.require_dep::<TyUsePureEnc>(field_ty)?;
                    cell_use
                        .expect_variant_opt(None)
                        .field_snaps_to_snap_generic(vec![slot])
                        .upcast_ty()
                }
                None => deep_snap(deps, field_ty, field_addr, field_snap)?,
            });
        }
        return Ok(variant.field_snaps_to_snap(field_snaps).upcast_ty());
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
                field_snaps.push(deep_snap(
                    deps,
                    field.decompose_normalize(ty.args),
                    proj.field_ref(addr),
                    proj.read(snap_c),
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
            let value = deep_snap(deps, referent, r_addr, ref_use.value_access(snap_c))?;
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
        if ty.ty.im_opaque.is_some() {
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
