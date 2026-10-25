//! Interior mutability, address-keyed (the PLDI27 design).
//!
//! An `UnsafeCell`'s contents are an interior location: its predicate holds
//! only the place component, and its snapshot has a slot for the contents.
//! Shallow snapshots (what predicates hold) have [`ImUnsetEnc`] in the slot,
//! deep snapshots (what specifications and `#[pure]` functions see)
//! have the contents.
//!
//! An `#[im_opaque]` struct has a key, the address of a value of it, and an
//! atom, the predicate `p_im_<S>(addr, T..)`: it holds the atoms of the
//! interior-mutable values stored inline in the struct (folded) and the
//! `#[im_perm]` amounts of the contents of its `UnsafeCell` fields. With an
//! `#[im_dependent]`, it has a second key, whose atom `p_im_<S>_dep` holds the
//! contents of its field in full, granted (scaled) by the amount.

use prusti_rustc_interface::{
    abi,
    middle::ty,
    span::{def_id::DefId, symbol},
};
use rustc_hash::FxHashSet;
use task_encoder::{EncodeFullError, TaskEncoder, TaskEncoderDependencies};
use vir::{CallableIdn, CastType};

use crate::encoders::{
    FunctionCallEnc, LocalDef, TyUseImpureEnc, TyUsePureEnc,
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

/// The type of the shared value of an `#[im_shares]` struct `did` (a shared
/// reference), in the struct's own generics.
pub(crate) fn shares_slot_ty<'tcx>(did: DefId) -> Option<ty::Ty<'tcx>> {
    if !is_im_sharer(did) {
        return None;
    }
    let (_, _, method) = im_amounts(did)
        .into_iter()
        .find(|(kind, ..)| matches!(kind, ImAmountKind::Shares { .. }))?;
    vir::with_vcx(|vcx| {
        // The annotation method's impl has the struct's generics.
        let sig = vcx.tcx().instantiate_bound_regions_with_erased(
            vcx.tcx()
                .fn_sig(method)
                .instantiate_identity()
                .skip_normalization(),
        );
        Some(vcx.tcx().erase_and_anonymize_regions(sig.output()))
    })
}

/// The pointer field of the struct `did` whose pointee (or a field of it) it
/// holds (`#[ptr_holds]`): its index and the held type.
pub(crate) fn ptr_holds_field<'tcx>(did: DefId) -> Option<(abi::FieldIdx, ty::Ty<'tcx>)> {
    ptr_holds(did).map(|(ptr, _, held)| (ptr, held))
}

/// Like [`ptr_holds_field`], with the index of the pointee's field that is
/// held, if not the whole pointee.
pub(crate) fn ptr_holds<'tcx>(
    did: DefId,
) -> Option<(abi::FieldIdx, Option<abi::FieldIdx>, ty::Ty<'tcx>)> {
    vir::with_vcx(|vcx| {
        let attrs = prusti_interface::environment::EnvQuery::new(vcx.tcx()).get_attributes(did);
        let path = prusti_interface::utils::read_prusti_attr("ptr_holds", attrs)?;
        let (name, sub) = match path.split_once('.') {
            Some((name, sub)) => (name.to_string(), Some(sub.to_string())),
            None => (path, None),
        };
        // A malformed annotation is reported, and the struct holds nothing.
        let malformed = |message: String| {
            vcx.emit_early_error(prusti_interface::PrustiError::incorrect(
                format!("invalid `#[ptr_holds]`: {message}"),
                vcx.tcx().def_span(did).into(),
            ));
            None
        };
        let field_of = |adt: ty::AdtDef<'tcx>, name: &str| {
            adt.is_struct()
                .then(|| {
                    adt.non_enum_variant()
                        .fields
                        .iter_enumerated()
                        .find(|(_, f)| f.name.as_str() == name)
                        .map(|(idx, f)| (idx, f.did))
                })
                .flatten()
        };
        let Some((idx, field)) = field_of(vcx.tcx().adt_def(did), &name) else {
            return malformed(format!("no field `{name}`"));
        };
        let ptr_ty = vcx
            .tcx()
            .type_of(field)
            .instantiate_identity()
            .skip_normalization();
        let pointee = match *ptr_ty.kind() {
            ty::TyKind::RawPtr(pointee, _) => pointee,
            ty::TyKind::Adt(adt, args)
                if vcx
                    .tcx()
                    .is_diagnostic_item(symbol::sym::NonNull, adt.did()) =>
            {
                args.type_at(0)
            }
            _ => return malformed(format!("the field `{name}` is not a raw pointer or a `NonNull`")),
        };
        let Some(sub) = sub else {
            return Some((idx, None, pointee));
        };
        let ty::TyKind::Adt(pointee_adt, pointee_args) = *pointee.kind() else {
            return malformed(format!("the pointee `{pointee}` is not a struct"));
        };
        let Some((sub_idx, sub_field)) = field_of(pointee_adt, &sub) else {
            return malformed(format!("the pointee `{pointee}` has no field `{sub}`"));
        };
        let held = vcx
            .tcx()
            .type_of(sub_field)
            .instantiate(vcx.tcx(), pointee_args)
            .skip_normalization();
        Some((idx, Some(sub_idx), held))
    })
}

/// The kind of an annotation method (see `prusti_specs::im_type_attribute`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImAmountKind {
    Perm,
    Dependent,
    Holds,
    Shares { dependent: bool },
    SharesIf,
    Assume,
}

/// The annotation methods of the struct `did`: their kind, the field they
/// are about (if any), and the method.
pub(crate) fn im_amounts(did: DefId) -> Vec<(ImAmountKind, Option<abi::FieldIdx>, DefId)> {
    vir::with_vcx(|vcx| {
        let env = prusti_interface::environment::EnvQuery::new(vcx.tcx());
        let fields = &vcx.tcx().adt_def(did).non_enum_variant().fields;
        let field_idx = |name: &str| {
            fields
                .iter_enumerated()
                .find(|(_, f)| f.name.as_str() == name)
                .map(|(idx, _)| idx)
        };
        // A malformed annotation is reported and ignored.
        let malformed = |message: String| {
            vcx.emit_early_error(prusti_interface::PrustiError::incorrect(
                format!("invalid interior-mutability annotation: {message}"),
                vcx.tcx().def_span(did).into(),
            ));
        };
        let mut out = Vec::new();
        for impl_did in vcx.tcx().inherent_impls(did) {
            for item in vcx.tcx().associated_item_def_ids(*impl_did) {
                let Some(kind) = prusti_interface::utils::read_prusti_attr(
                    "im_amount",
                    env.get_attributes(*item),
                ) else {
                    continue;
                };
                let mut words = kind.split_whitespace();
                let (kind, field) = match (words.next(), words.next()) {
                    (Some("perm"), Some(f)) => (ImAmountKind::Perm, Some(f)),
                    (Some("dependent"), Some(f)) => (ImAmountKind::Dependent, Some(f)),
                    (Some("holds"), Some(f)) => (ImAmountKind::Holds, Some(f)),
                    (Some("shares"), None) => (ImAmountKind::Shares { dependent: false }, None),
                    (Some("shares"), Some("dependent")) => {
                        (ImAmountKind::Shares { dependent: true }, None)
                    }
                    (Some("assume"), None) => (ImAmountKind::Assume, None),
                    (Some("shares_if"), None) => (ImAmountKind::SharesIf, None),
                    _ => {
                        malformed(format!("`{kind}`"));
                        continue;
                    }
                };
                let field = match field.map(|name| (name, field_idx(name))) {
                    Some((name, None)) => {
                        malformed(format!("no field `{name}`"));
                        continue;
                    }
                    field => field.and_then(|(_, idx)| idx),
                };
                out.push((kind, field, *item));
            }
        }
        out
    })
}

/// The atoms of an `#[im_opaque]` struct: the atom of its key, and that of
/// its dependent key, if it has one. Its amounts are heap-dependent functions
/// (Viper allows no wildcard in a permission amount, even nested in an
/// `unfolding`): each `#[im_perm]`/`#[im_dependent]` method's, reading the
/// atoms the atom folds, and the dependent amount reading the atom itself.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ImAtomRef<'vir> {
    atom: ImAtomIdn<'vir>,
    dependent: Option<ImAtomIdn<'vir>>,
    amounts: &'vir [(DefId, ImAmountIdn<'vir>)],
    dependent_amount: Option<ImAmountIdn<'vir>>,
}
impl<'vir> task_encoder::OutputRefAny for ImAtomRef<'vir> {}

impl<'vir> ImAtomRef<'vir> {
    fn amount(&self, method: DefId) -> ImAmountIdn<'vir> {
        self.amounts.iter().find(|(m, _)| *m == method).unwrap().1
    }
}

pub(crate) type ImAtomIdn<'vir> =
    vir::PredicateIdn<'vir, (vir::Ref, vir::ManyTyVal, vir::ManyCSnap)>;
pub(crate) type ImAmountIdn<'vir> =
    vir::FunctionIdn<'vir, (vir::Ref, vir::ManyTyVal, vir::ManyCSnap), vir::Perm>;

pub(crate) struct ImOpaqueEnc;

impl TaskEncoder for ImOpaqueEnc {
    task_encoder::encoder_cache!(ImOpaqueEnc);
    const ENCODER_NAME: &'static str = "interior mutability opaque type encoder";
    type TaskDescription<'vir> = DefId;
    type OutputRef<'vir> = ImAtomRef<'vir>;
    type OutputFullLocal<'vir> = (Vec<vir::Predicate<'vir>>, Vec<vir::Function<'vir>>);

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
            let amount_ident = |suffix: &str| {
                let name = vir::vir_format_identifier!(
                    vcx,
                    "p_im_{}_amount{suffix}",
                    vir::ViperIdent::from_def_id(vcx, *did)
                );
                vir::FunctionIdn::new(
                    name,
                    (vir::TYPE_REF, generics.ty_args(), generics.const_args()),
                    vir::TYPE_PERM,
                )
            };
            let amount_fns = amounts
                .iter()
                .enumerate()
                .filter(|(_, (kind, ..))| {
                    matches!(kind, ImAmountKind::Perm | ImAmountKind::Dependent)
                })
                .map(|(idx, (_, _, method))| (*method, amount_ident(&format!("_{idx}"))))
                .collect::<Vec<_>>();
            let has_dependent = amounts
                .iter()
                .any(|(kind, ..)| *kind == ImAmountKind::Dependent);
            let atoms = ImAtomRef {
                atom: ident(""),
                dependent: has_dependent.then(|| ident("_dep")),
                amounts: vcx.alloc_slice(&amount_fns),
                dependent_amount: has_dependent.then(|| amount_ident("_dep")),
            };
            deps.emit_output_ref(*did, atoms)?;

            let self_decl = vcx.mk_local_decl("self", vir::TYPE_REF);
            let self_ex = vcx.mk_local_ex(self_decl);
            let identity = vcx
                .tcx()
                .type_of(*did)
                .instantiate_identity()
                .skip_normalization();
            let ty = RustTyDecomposition::from_ty(identity, params);
            // The atom does not hold the struct's own predicate, so its
            // definition only reads the contents of the atoms it holds.
            let snap = deps.require_ref::<TyUsePureEnc>(ty)?.arbitrary_to_snap();
            let decls = (self_decl, generics.ty_decls(), generics.const_decls());
            let call = |idn: ImAmountIdn<'vir>| {
                idn.call()(self_ex, generics.ty_exprs(), generics.const_exprs())
            };

            let folded = folded(deps, ty, self_ex, snap)?;
            let mut body = folded.iter().map(FpEntry::acc).collect::<Vec<_>>();
            let reads_folded = folded
                .iter()
                .map(|entry| vcx.mk_predicate_app_expr(entry.app(vcx.mk_wildcard())))
                .collect::<Vec<_>>();
            let inner = deep_with(deps, ty, self_ex, snap, DeepMode::Atom)?;
            let nonneg_post = vcx.alloc_slice(&[vcx
                .mk_bin_op_expr(
                    vir::BinOpKind::CmpGe,
                    vcx.mk_result(vir::TYPE_PERM),
                    vcx.mk_no_perm(),
                )
                .downcast_ty()]);
            let mut functions = Vec::new();
            for (method, idn) in &amount_fns {
                functions.push(vcx.mk_function(
                    *idn,
                    decls,
                    vcx.alloc_slice(&reads_folded),
                    nonneg_post,
                    None,
                    Some(nonneg(call_amount(deps, *method, ty, self_ex, inner)?)),
                ));
            }
            let mut dep_body = Vec::new();
            // A struct has one dependent atom, with one amount.
            let mut dependents = amounts
                .iter()
                .filter(|(kind, ..)| *kind == ImAmountKind::Dependent)
                .map(|(_, _, method)| *method);
            let dependent = dependents.next();
            if dependents.next().is_some() {
                vcx.emit_early_error(prusti_interface::PrustiError::unsupported(
                    "more than one `#[im_dependent]` annotation on a struct",
                    vcx.tcx().def_span(*did).into(),
                ));
            }
            for (kind, field, method) in &amounts {
                // The other annotations hold nothing in the atom.
                if !matches!(kind, ImAmountKind::Perm | ImAmountKind::Dependent) {
                    continue;
                }
                if *kind == ImAmountKind::Dependent && Some(*method) != dependent {
                    continue;
                }
                let contents = contents_of(deps, ty, self_ex, field.unwrap())?;
                match kind {
                    // Clamped again: Silicon does not use the function's
                    // postcondition when unfolding the atom in a function.
                    ImAmountKind::Perm => {
                        body.push(contents.acc(nonneg(call(atoms.amount(*method)))))
                    }
                    ImAmountKind::Dependent => {
                        dep_body.push(contents.acc(vcx.mk_full_perm()));
                        // The amount the dependent atom is granted at, read
                        // through the atom.
                        let own = atoms.atom.call()(
                            self_ex,
                            generics.ty_exprs(),
                            generics.const_exprs(),
                        )(Some(vcx.mk_wildcard()));
                        functions.push(vcx.mk_function(
                            atoms.dependent_amount.unwrap(),
                            decls,
                            vcx.alloc_slice(&[vcx.mk_predicate_app_expr(own)]),
                            nonneg_post,
                            None,
                            Some(vcx.mk_unfolding_expr(own, call(atoms.amount(*method)))),
                        ));
                    }
                    _ => (),
                }
            }
            let mut preds = vec![vcx.mk_predicate(atoms.atom, decls, Some(vcx.mk_conj(&body)))];
            if let Some(dependent) = atoms.dependent {
                preds.push(vcx.mk_predicate(dependent, decls, Some(vcx.mk_conj(&dep_body))));
            }
            Ok(((preds, functions), ()))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for (preds, functions) in Self::all_outputs_local_no_errors(program) {
            for pred in preds {
                program.add_predicate(pred);
            }
            for function in functions {
                program.add_function(function);
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
    // A plain field is itself the interior location.
    if cell_ty.ty.special != crate::encoders::ty::RustTySpecial::UnsafeCell {
        let contents_ty = ty.ty.expect_structlike().fields[field.as_usize()]
            .decompose_context(ty.ty.params, ty.args);
        return Ok(Contents {
            addr: cell_addr,
            ty: deps.require_dep::<TyUseImpureEnc>(contents_ty)?,
        });
    }
    unsafe_cell_contents(deps, cell_ty, cell_addr)
}

/// The contents of the `UnsafeCell` of type `cell_ty` at `cell_addr`.
fn unsafe_cell_contents<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    cell_ty: RustTyDecomposition<'vir>,
    cell_addr: vir::ExprRef<'vir>,
) -> Result<Contents<'vir>, EncodeFullError<'vir, E>> {
    let cell_use = deps.require_dep::<TyUsePureEnc>(cell_ty)?;
    let addr = cell_use.expect_variant_opt(None).fields[0].field_ref(cell_addr);
    let contents_ty =
        cell_ty.ty.expect_structlike().fields[0].decompose_context(cell_ty.ty.params, cell_ty.args);
    Ok(Contents {
        addr,
        ty: deps.require_dep::<TyUseImpureEnc>(contents_ty)?,
    })
}

/// The permission to and the snapshot of the contents of the value of `ty`
/// at `addr`, if it is an `UnsafeCell`: a free-floating permission (see
/// [`unsafe_cell_contract`]), which moves with the cell.
pub(crate) fn unsafe_cell_slot<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
) -> Result<Option<(vir::ExprBool<'vir>, vir::ExprSnap<'vir>)>, EncodeFullError<'vir, E>> {
    if ty.ty.special != crate::encoders::ty::RustTySpecial::UnsafeCell {
        return Ok(None);
    }
    let contents = unsafe_cell_contents(deps, ty, addr)?;
    let full = vir::with_vcx(|vcx| vcx.mk_full_perm());
    Ok(Some((contents.acc(full), contents.snap())))
}

/// The contract of an `UnsafeCell` primitive on the contents, an interior
/// location outside the cell's predicate: `new` returns the permission to
/// them (free-floating, folded into an atom where needed) and `into_inner`
/// takes it. As pre- and postconditions; `args[0]` is the result.
/// (`UnsafeCell::get` only needs an address fact, given by its spec.)
pub(crate) fn unsafe_cell_contract<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    def_id: DefId,
    args: &[LocalDef<'vir>],
) -> Result<(Vec<vir::ExprBool<'vir>>, Vec<vir::ExprBool<'vir>>), EncodeFullError<'vir, E>> {
    let vcx = vir::with_vcx(|vcx| vcx);
    let is_unsafe_cell = |ty: RustTyDecomposition<'vir>| {
        ty.ty.special == crate::encoders::ty::RustTySpecial::UnsafeCell
    };
    let Some(item) = vcx.tcx().opt_associated_item(def_id) else {
        return Ok(Default::default());
    };
    // Only `UnsafeCell`'s own methods.
    let of_unsafe_cell = vcx.tcx().impl_of_assoc(def_id).is_some_and(|impl_did| {
        vcx.tcx()
            .type_of(impl_did)
            .instantiate_identity()
            .skip_normalization()
            .ty_adt_def()
            .is_some_and(|adt| adt.is_unsafe_cell())
    });
    if !of_unsafe_cell {
        return Ok(Default::default());
    }
    let full = vcx.mk_full_perm();
    match item.name().as_str() {
        "new" if args.len() == 2 && is_unsafe_cell(args[0].ty) => {
            let contents = unsafe_cell_contents(deps, args[0].ty, args[0].local_ex)?;
            let value = vcx.mk_old_expr(args[1].impure_snap);
            let post = vcx.mk_conj(&[contents.acc(full), vcx.mk_eq_expr(contents.snap(), value)]);
            Ok((Vec::new(), vec![post]))
        }
        "into_inner" if args.len() == 2 && is_unsafe_cell(args[1].ty) => {
            let contents = unsafe_cell_contents(deps, args[1].ty, args[1].local_ex)?;
            let value = vcx.mk_old_expr(contents.snap());
            let post = vcx.mk_eq_expr(args[0].impure_snap, value);
            Ok((vec![contents.acc(full)], vec![post]))
        }
        _ => Ok(Default::default()),
    }
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
    // `Real` is represented natively as Viper `Perm`.
    Ok(call_annotation(deps, method, ty, addr, snap)?.downcast_ty())
}

/// The `#[im_assume]` fact of a value of type `ty` at `addr`, whose
/// predicate is held: its annotation method on its deep value.
pub(crate) fn guard_assumption<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
) -> Result<Option<vir::ExprBool<'vir>>, EncodeFullError<'vir, E>> {
    if ty.ty.im_annotated.or(ty.ty.im_opaque).is_none() {
        return Ok(None);
    }
    let snap = deps.require_dep::<TyUseImpureEnc>(ty)?.ref_to_snap(addr);
    guard_assumption_of(deps, ty, addr, snap)
}

/// Like [`guard_assumption`], for the value with shallow snapshot `snap`.
fn guard_assumption_of<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
    snap: vir::ExprSnap<'vir>,
) -> Result<Option<vir::ExprBool<'vir>>, EncodeFullError<'vir, E>> {
    let Some(did) = ty.ty.im_annotated.or(ty.ty.im_opaque) else {
        return Ok(None);
    };
    let Some((_, _, method)) = im_amounts(did)
        .into_iter()
        .find(|(kind, ..)| *kind == ImAmountKind::Assume)
    else {
        return Ok(None);
    };
    let deep = deep_snap(deps, ty, addr, snap)?;
    let fact = call_annotation(deps, method, ty, addr, deep)?;
    let bool_ty =
        RustTyDecomposition::from_ty(vir::with_vcx(|vcx| vcx.tcx().types.bool), ty.args.context());
    let bool_use = deps.require_dep::<TyUsePureEnc>(bool_ty)?;
    Ok(Some(
        bool_use
            .expect_primitive()
            .snap_to_prim(fact.downcast_ty())
            .downcast_ty(),
    ))
}

/// The result of the annotation method `method` of the struct `ty` for the
/// value at `addr` with (deep) snapshot `snap`.
fn call_annotation<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    method: DefId,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
    snap: vir::ExprSnap<'vir>,
) -> Result<vir::ExprSnap<'vir>, EncodeFullError<'vir, E>> {
    let self_ref = self_ref_snap(deps, method, ty, addr, snap)?;
    let func = deps.require_dep::<FunctionCallEnc>(CallTaskDescription::new(
        ty.args.context(),
        ty.args.args(),
        method,
    ))?;
    Ok(func.call_pure(vec![self_ref]))
}

/// The amount of its pointee a `#[ptr_holds]` struct `did` holds (the
/// annotation method takes no arguments), in the context of its definition.
pub(crate) fn holds_amount<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    did: DefId,
    params: GParams<'vir>,
) -> Result<vir::ExprPerm<'vir>, EncodeFullError<'vir, E>> {
    let (_, _, method) = im_amounts(did)
        .into_iter()
        .find(|(kind, ..)| *kind == ImAmountKind::Holds)
        .unwrap();
    let func = deps.require_dep::<FunctionCallEnc>(CallTaskDescription::new(
        params,
        params.rust_params(),
        method,
    ))?;
    Ok(nonneg(func.call_pure(Vec::new()).downcast_ty()))
}

/// The amount of its pointee a value of the `#[ptr_holds]` struct `did` of
/// type `ty` holds.
pub(crate) fn holds_amount_of<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    did: DefId,
    ty: RustTyDecomposition<'vir>,
) -> Result<vir::ExprPerm<'vir>, EncodeFullError<'vir, E>> {
    let (_, _, method) = im_amounts(did)
        .into_iter()
        .find(|(kind, ..)| *kind == ImAmountKind::Holds)
        .unwrap();
    let func = deps.require_dep::<FunctionCallEnc>(CallTaskDescription::new(
        ty.args.context(),
        ty.args.args(),
        method,
    ))?;
    Ok(nonneg(func.call_pure(Vec::new()).downcast_ty()))
}

/// `amount`, or none where it is negative: an annotation's amount is used as
/// a permission amount, which Viper requires to be nonnegative.
fn nonneg<'vir>(amount: vir::ExprPerm<'vir>) -> vir::ExprPerm<'vir> {
    vir::with_vcx(|vcx| {
        vcx.mk_ternary_expr(
            vcx.mk_bin_op_expr(vir::BinOpKind::CmpGt, amount, vcx.mk_no_perm())
                .downcast_ty(),
            amount,
            vcx.mk_no_perm(),
        )
    })
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
        vcx.tcx()
            .fn_sig(method)
            .instantiate(vcx.tcx(), ty.args.args())
            .skip_normalization()
            .skip_binder()
            .inputs()[0]
    });
    let self_ty = RustTyDecomposition::from_ty(self_ty, ty.args.context());
    let metadata_ty = self_ty
        .ty
        .ref_data()
        .unwrap()
        .metadata
        .decompose_normalize(self_ty.args);
    let metadata = match deps.require_dep::<TyUsePureEnc>(metadata_ty)?.zst_to_snap() {
        Some(metadata) => metadata.upcast_ty(),
        None => deps
            .require_ref::<TyUsePureEnc>(metadata_ty)?
            .arbitrary_to_snap(),
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
    /// The (shallow) snapshot of the value.
    snap: vir::ExprSnap<'vir>,
    tys: &'vir [vir::ExprTyVal<'vir>],
    consts: &'vir [vir::ExprCSnap<'vir>],
    /// The amount of a dependent atom, read at its key in the current state
    /// (`None`: the full atom).
    amount: Option<ImAmountIdn<'vir>>,
    /// Reached through a reference (or a guard): returned to the caller, and
    /// possibly aliased with other such entries.
    pub(crate) reached: bool,
    /// Reached through a shared reference or a sharing (`#[im_shares]`): may
    /// alias other shared entries. Such an atom is held by every holder, so a
    /// borrow of one of them does not block it; the others are lent to the
    /// borrow's result.
    pub(crate) shared: bool,
    /// Where the atom is reached (a conditional `#[im_shares]`), if not
    /// always.
    condition: Option<vir::ExprBool<'vir>>,
}

impl<'vir> FpEntry<'vir> {
    fn app(&self, perm: vir::ExprPerm<'vir>) -> vir::PredicateApp<'vir> {
        self.atom.call()(self.addr, self.tys, self.consts)(Some(perm))
    }

    fn amount(&self) -> vir::ExprPerm<'vir> {
        vir::with_vcx(|vcx| {
            let amount = self.amount.map_or_else(
                || vcx.mk_full_perm(),
                |amount| amount.call()(self.addr, self.tys, self.consts),
            );
            match self.condition {
                Some(condition) => vcx.mk_ternary_expr(condition, amount, vcx.mk_no_perm()),
                None => amount,
            }
        })
    }

    /// The entry with its key and condition evaluated in the state `label`
    /// (the amount of a dependent atom is that of the current state).
    pub(crate) fn in_state(&self, label: vir::OldLabel<'vir>) -> Self {
        vir::with_vcx(|vcx| Self {
            addr: vcx.mk_old(self.addr, label),
            condition: self.condition.map(|condition| vcx.mk_old(condition, label)),
            ..*self
        })
    }

    /// Where the atom is reached, if not always (its contents can only be
    /// read there).
    pub(crate) fn condition(&self) -> Option<vir::ExprBool<'vir>> {
        self.condition
    }

    /// The entry, reached only where `condition` holds as well.
    pub(crate) fn with_condition(self, condition: Option<vir::ExprBool<'vir>>) -> Self {
        let Some(condition) = condition else {
            return self;
        };
        Self {
            condition: Some(match self.condition {
                Some(inner) => vir::with_vcx(|vcx| vcx.mk_conj(&[condition, inner])),
                None => condition,
            }),
            ..self
        }
    }

    /// The permission to the atom.
    pub(crate) fn acc(&self) -> vir::ExprBool<'vir> {
        vir::with_vcx(|vcx| vcx.mk_predicate_app_expr(self.app(self.amount())))
    }

    /// The snapshots of the contents of the atom's `#[im_perm]`s and of the
    /// atoms it folds, read through the atom (the dependent atom of a key has
    /// none of its own).
    pub(crate) fn contents<E: TaskEncoder>(
        &self,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> Result<Vec<vir::ExprSnap<'vir>>, EncodeFullError<'vir, E>> {
        if self.amount.is_some() {
            return Ok(Vec::new());
        }
        // A generic atom's contents are its deep value.
        if let TySpecifics::Param(_) = self.ty.ty.specifics {
            let param = deps.require_dep::<ParamImEnc>(())?;
            return Ok(vec![param.snap.call()(self.addr, self.snap, self.tys[0])]);
        }
        let did = self.ty.ty.im_opaque.unwrap();
        let vcx = vir::with_vcx(|vcx| vcx);
        let unfold = self.app(vcx.mk_wildcard());
        let atoms = deps.require_ref::<ImOpaqueEnc>(did)?;
        let unset = deps.require_dep::<ImUnsetEnc>(())?;
        let mut out = Vec::new();
        for (kind, field, method) in im_amounts(did) {
            if kind == ImAmountKind::Perm {
                let contents = contents_of(deps, self.ty, self.addr, field.unwrap())?;
                // Where the atom holds none of them, the contents are not
                // readable (as in a deep value).
                let amount = atoms.amount(method).call()(self.addr, self.tys, self.consts);
                let held = vcx
                    .mk_bin_op_expr(vir::BinOpKind::CmpGt, amount, vcx.mk_no_perm())
                    .downcast_ty();
                let field_ty = self.ty.ty.expect_structlike().fields[field.unwrap().as_usize()]
                    .decompose_normalize(self.ty.args);
                let contents_ty =
                    if field_ty.ty.special == crate::encoders::ty::RustTySpecial::UnsafeCell {
                        deps.require_dep::<GArgsTyEnc>(field_ty.args)?.get_ty()[0]
                    } else {
                        deps.require_dep::<crate::encoders::ty::generics::TyExprEnc>(field_ty)?
                    };
                let snap = vcx.mk_ternary_expr(
                    held,
                    contents.snap(),
                    unset.call()(contents_ty).upcast_ty(),
                );
                out.push(vcx.mk_unfolding_expr(unfold, snap));
            }
        }
        // Those of the atoms it folds.
        for folded in folded(deps, self.ty, self.addr, self.snap)? {
            for snap in folded.contents(deps)? {
                out.push(vcx.mk_unfolding_expr(unfold, snap));
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
            eqs.extend(
                self.tys
                    .iter()
                    .zip(other.tys)
                    .map(|(a, b)| vcx.mk_eq_expr(*a, *b)),
            );
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

/// What an `#[im_shares]` struct shares: the referent of the reference its
/// annotation method computes, and whether the dependent atoms are shared.
struct SharedTarget<'vir> {
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
    metadata: vir::ExprSnap<'vir>,
    snap: vir::ExprSnap<'vir>,
    dependent: bool,
    /// Where the struct shares the target (`if = COND`), if not always.
    condition: Option<vir::ExprBool<'vir>>,
}

fn shared_target<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
    snap: vir::ExprSnap<'vir>,
) -> Result<Option<SharedTarget<'vir>>, EncodeFullError<'vir, E>> {
    let Some(did) = ty.ty.im_annotated else {
        return Ok(None);
    };
    let Some((dependent, method)) =
        im_amounts(did)
            .into_iter()
            .find_map(|(kind, _, method)| match kind {
                ImAmountKind::Shares { dependent } => Some((dependent, method)),
                _ => None,
            })
    else {
        return Ok(None);
    };
    let self_ref = self_ref_snap(deps, method, ty, addr, snap)?;
    let func = deps.require_dep::<FunctionCallEnc>(CallTaskDescription::new(
        ty.args.context(),
        ty.args.args(),
        method,
    ))?;
    let target = func.call_pure(vec![self_ref]);
    let target_ty = vir::with_vcx(|vcx| {
        vcx.tcx()
            .fn_sig(method)
            .instantiate(vcx.tcx(), ty.args.args())
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
    let condition = match im_amounts(did)
        .into_iter()
        .find(|(kind, ..)| *kind == ImAmountKind::SharesIf)
    {
        Some((_, _, method)) => {
            let func = deps.require_dep::<FunctionCallEnc>(CallTaskDescription::new(
                ty.args.context(),
                ty.args.args(),
                method,
            ))?;
            // `bool` is represented natively.
            Some(func.call_pure(vec![self_ref]).downcast_ty())
        }
        None => None,
    };
    Ok(Some(SharedTarget {
        ty: data.referent.decompose_normalize(target_ty.args),
        addr: ref_data.addr_access(target_c),
        metadata: ref_data.metadata_access(target_c),
        snap: ref_data.value_access(target_c),
        dependent,
        condition,
    }))
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
            snap,
            tys: args.get_ty(),
            consts: args.get_const(),
            amount: None,
            reached,
            shared,
            condition: None,
        };
        out.push(entry);
        if let Some(dependent) = atoms.dependent
            && with_dependent
        {
            out.push(FpEntry {
                atom: dependent,
                amount: atoms.dependent_amount,
                ..entry
            });
        }
        return Ok(());
    }
    if let Some(shared) = shared_target(deps, ty, addr, snap)? {
        // A value of the struct reaches the shared atoms instead of those of
        // its own fields.
        let start = out.len();
        footprint_into(
            deps,
            shared.ty,
            shared.addr,
            shared.snap,
            true,
            true,
            shared.dependent,
            out,
        )?;
        if let Some(condition) = shared.condition {
            for entry in &mut out[start..] {
                entry.condition = Some(match entry.condition {
                    Some(inner) => vir::with_vcx(|vcx| vcx.mk_conj(&[condition, inner])),
                    None => condition,
                });
            }
        }
        return Ok(());
    }
    match &ty.ty.specifics {
        // The generic atom of the value.
        TySpecifics::Param(_) if param_reaches_im(ty) => {
            let param = deps.require_dep::<ParamImEnc>(())?;
            let ty_expr = deps.require_dep::<crate::encoders::ty::generics::TyExprEnc>(ty)?;
            out.push(FpEntry {
                ty,
                atom: param.atom,
                addr,
                snap,
                tys: vir::with_vcx(|vcx| vcx.alloc_slice(&[ty_expr])),
                consts: &[],
                amount: None,
                reached,
                shared,
                condition: None,
            });
        }
        // The value a box owns, at its pointee address.
        TySpecifics::StructLike(data)
            if ty.ty.special == crate::encoders::ty::RustTySpecial::Box =>
        {
            let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
            let variant = ty_use.expect_variant_opt(None);
            let snap_c = snap.downcast_ty();
            footprint_into(
                deps,
                data.fields.last().unwrap().decompose_normalize(ty.args),
                variant.box_address_access(snap_c),
                variant.fields.last().unwrap().read(snap_c),
                reached,
                shared,
                with_dependent,
                out,
            )?;
        }
        TySpecifics::StructLike(data)
            if data
                .fields
                .iter()
                .all(|f| matches!(f.address, crate::encoders::ty::RustFieldAddress::Constant)) =>
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
        // The atoms of the fields of the variant the value is.
        TySpecifics::EnumLike(data) => {
            let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
            let snap_c = snap.downcast_ty();
            for variant in &data.variants {
                let start = out.len();
                let variant_use = ty_use.expect_variant_opt(Some(variant.data.vid));
                for (idx, field) in variant.inner.fields.iter().enumerate() {
                    let proj = &variant_use.fields[idx];
                    footprint_into(
                        deps,
                        field.decompose_normalize(ty.args),
                        proj.field_ref(addr),
                        proj.read(snap_c),
                        reached,
                        shared,
                        with_dependent,
                        out,
                    )?;
                }
                if out.len() == start {
                    continue;
                }
                let is = is_variant(deps, ty, data, variant.data, snap_c)?;
                for entry in &mut out[start..] {
                    entry.condition = Some(match entry.condition {
                        Some(inner) => vir::with_vcx(|vcx| vcx.mk_conj(&[is, inner])),
                        None => is,
                    });
                }
            }
        }
        // The referent's value is not in the snapshot: it is read from its
        // predicate (held next to the reference) in the current state.
        TySpecifics::MutRef(data) => {
            let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
            let referent = data.referent.decompose_normalize(ty.args);
            let r_addr = ty_use.expect_mutref().deref_access(snap.downcast_ty());
            // Held generically (as the reference type's parameter).
            let generic = data.referent.decompose_context(ty.ty.params, ty.args);
            let caster = deps.require_dep::<crate::GArgsCastEnc<crate::Pure>>(
                data.referent
                    .decompose_compare_normalize(ty.ty.params, ty.args),
            )?;
            let r_snap = caster.cast_to_caller_ctx(
                deps.require_dep::<TyUseImpureEnc>(generic)?
                    .ref_to_snap(r_addr),
            );
            // Reached through a shared reference, it is shared too.
            footprint_into(
                deps,
                referent,
                r_addr,
                r_snap,
                true,
                shared,
                with_dependent,
                out,
            )?;
        }
        _ => (),
    }
    Ok(())
}

/// Whether the value of the enum `ty` with snapshot `snap` is the variant
/// `vid`.
pub(crate) fn variant_is<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    vid: abi::VariantIdx,
    snap: vir::ExprCSnap<'vir>,
) -> Result<vir::ExprBool<'vir>, EncodeFullError<'vir, E>> {
    let TySpecifics::EnumLike(data) = &ty.ty.specifics else {
        unreachable!("not an enum: {ty:?}")
    };
    let variant = data.variants.iter().find(|v| v.data.vid == vid).unwrap();
    is_variant(deps, ty, data, variant.data, snap)
}

/// Whether the value of the enum `ty` with snapshot `snap` is `variant`.
fn is_variant<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    data: &crate::encoders::ty::data::EnumData<'vir, crate::encoders::ty::RustTyDatas>,
    variant: crate::encoders::ty::rust_ty::RustVariantData,
    snap: vir::ExprCSnap<'vir>,
) -> Result<vir::ExprBool<'vir>, EncodeFullError<'vir, E>> {
    let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
    let discr = ty_use.expect_enumlike().snap_to_discr_snap(snap);
    let discr_ty = deps.require_dep::<crate::encoders::ty::pure::TyPureEnc>(
        RustTyDecomposition::from_prim_ty(data.data.discr).ty,
    )?;
    let discr_prim = *discr_ty.expect_primitive();
    let value =
        discr_prim.prim_to_snap(discr_prim.expr_from_bits(data.data.discr, variant.discr_val));
    Ok(vir::with_vcx(|vcx| vcx.mk_eq_expr(discr, value)))
}

/// The atoms a value of `ty` with snapshot `snap` reaches other than through
/// shared references: what a borrow of it lends to the borrow's result.
pub(crate) fn lent_footprint<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    snap: vir::ExprSnap<'vir>,
) -> Result<Option<vir::ExprBool<'vir>>, EncodeFullError<'vir, E>> {
    if !reaches_im(ty) {
        return Ok(None);
    }
    let vcx = vir::with_vcx(|vcx| vcx);
    let entries = footprint(deps, ty, vcx.mk_null(), snap)?
        .into_iter()
        .filter(|entry| !entry.shared)
        .collect::<Vec<_>>();
    Ok((!entries.is_empty()).then(|| footprint_assertion(&entries)))
}

/// What holds of an argument of type `ty` with snapshot `snap` when the
/// borrow of it by the result of a call expires:
/// - the atoms it reaches through shared references are at the keys they
///   had in the state `label`: the call returned them at their keys from
///   its pre-state (see `MethodEnc`);
/// - a guard behind a mutable reference is live again, so its
///   `#[im_assume]` fact holds.
pub(crate) fn on_expiry<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    snap: vir::ExprSnap<'vir>,
    label: vir::OldLabel<'vir>,
) -> Result<Option<vir::ExprBool<'vir>>, EncodeFullError<'vir, E>> {
    if !reaches_im(ty) {
        return Ok(None);
    }
    let vcx = vir::with_vcx(|vcx| vcx);
    let entries = footprint(deps, ty, vcx.mk_null(), snap)?;
    let mut conjs = entries
        .iter()
        .filter(|entry| entry.reached && entry.shared && entry.amount.is_none())
        .map(|entry| {
            let same = vcx.mk_eq_expr(entry.addr, vcx.mk_old(entry.addr, label));
            match entry.condition {
                Some(condition) => vcx.mk_conj(&[
                    vcx.mk_eq_expr(condition, vcx.mk_old(condition, label)),
                    vcx.mk_bin_op_expr(vir::BinOpKind::Implies, condition, same)
                        .downcast_ty(),
                ]),
                None => same,
            }
        })
        .collect::<Vec<_>>();
    if let TySpecifics::MutRef(data) = &ty.ty.specifics {
        let referent = data.referent.decompose_normalize(ty.args);
        let ref_use = deps.require_dep::<TyUsePureEnc>(ty)?;
        let r_addr = ref_use.expect_mutref().deref_access(snap.downcast_ty());
        // Held generically (as the reference type's parameter).
        let generic = data.referent.decompose_context(ty.ty.params, ty.args);
        let caster = deps.require_dep::<crate::GArgsCastEnc<crate::Pure>>(
            data.referent
                .decompose_compare_normalize(ty.ty.params, ty.args),
        )?;
        let r_snap = caster.cast_to_caller_ctx(
            deps.require_dep::<TyUseImpureEnc>(generic)?
                .ref_to_snap(r_addr),
        );
        conjs.extend(guard_assumption_of(deps, referent, r_addr, r_snap)?);
    }
    Ok((!conjs.is_empty()).then(|| vcx.mk_conj(&conjs)))
}

/// The assertions releasing the permissions to `entries`, to be exhaled in
/// order: the dependent atoms first, as their amounts read the atoms.
pub(crate) fn footprint_release<'vir>(entries: &[FpEntry<'vir>]) -> Vec<vir::ExprBool<'vir>> {
    let (dependent, atoms): (Vec<_>, Vec<_>) = entries.iter().partition(|e| e.amount.is_some());
    [dependent, atoms]
        .into_iter()
        .filter(|part| !part.is_empty())
        .map(|part| footprint_assertion(&part))
        .collect()
}

/// The reference parameter of the `#[im_open(PARAM)]` function `def_id`
/// (see `prusti_contracts::im_open`), if any and valid (see
/// [`im_open_attr`]).
pub(crate) fn im_open_param(def_id: DefId) -> Option<prusti_rustc_interface::middle::mir::Local> {
    let (param, valid) = im_open_attr(def_id)?;
    valid.then_some(param)
}

/// The parameter named by the `#[im_open(PARAM)]` attribute of `def_id`, if
/// any, and whether it is valid: a shared reference to an `#[im_opaque]`
/// value.
pub(crate) fn im_open_attr(
    def_id: DefId,
) -> Option<(prusti_rustc_interface::middle::mir::Local, bool)> {
    vir::with_vcx(|vcx| {
        let attrs = prusti_interface::environment::EnvQuery::new(vcx.tcx()).get_attributes(def_id);
        let name = prusti_interface::utils::read_prusti_attr("im_open", attrs)?;
        // The attribute macro checked that the parameter exists.
        let idx = vcx
            .tcx()
            .fn_arg_idents(def_id)
            .iter()
            .position(|ident| ident.is_some_and(|ident| ident.name.as_str() == name))
            .unwrap();
        let input = vcx
            .tcx()
            .fn_sig(def_id)
            .instantiate_identity()
            .skip_normalization()
            .skip_binder()
            .inputs()[idx];
        let valid = matches!(*input.kind(), ty::TyKind::Ref(_, referent, ty::Mutability::Not)
            if matches!(*referent.kind(), ty::TyKind::Adt(adt, _) if is_im_opaque(adt.did())));
        Some((prusti_rustc_interface::middle::mir::Local::from_usize(idx + 1), valid))
    })
}

/// The type and address of the `#[im_opaque]` value the shared reference
/// `snap` of type `ty` points to (an `#[im_open]` parameter), if any.
#[allow(clippy::type_complexity)]
pub(crate) fn im_open_referent<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    snap: vir::ExprSnap<'vir>,
) -> Result<Option<(RustTyDecomposition<'vir>, vir::ExprRef<'vir>)>, EncodeFullError<'vir, E>> {
    let TySpecifics::ImmRef(data) = &ty.ty.specifics else {
        return Ok(None);
    };
    let referent = data.referent.decompose_normalize(ty.args);
    if referent.ty.im_opaque.is_none() {
        return Ok(None);
    }
    let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
    let addr = ty_use.expect_immref().addr_access(snap.downcast_ty());
    Ok(Some((referent, addr)))
}

/// The permissions to the atoms of the value of `ty` at `addr`, if it is of
/// an `#[im_opaque]` struct (`im_atoms` in a specification): the atom, then
/// the dependent atom at its amount.
pub(crate) fn atoms_assertion<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
) -> Result<Option<vir::ExprBool<'vir>>, EncodeFullError<'vir, E>> {
    let Some(did) = ty.ty.im_opaque else {
        return Ok(None);
    };
    let atoms = deps.require_ref::<ImOpaqueEnc>(did)?;
    let args = deps.require_dep::<GArgsTyEnc>(ty.args)?;
    let (tys, consts) = (args.get_ty(), args.get_const());
    let vcx = vir::with_vcx(|vcx| vcx);
    let mut conjs = vec![vcx.mk_predicate_app_expr(
        atoms.atom.call()(addr, tys, consts)(Some(vcx.mk_full_perm())),
    )];
    if let Some(dependent) = atoms.dependent {
        let amount = atoms.dependent_amount.unwrap().call()(addr, tys, consts);
        conjs.push(vcx.mk_predicate_app_expr(
            dependent.call()(addr, tys, consts)(Some(amount)),
        ));
    }
    Ok(Some(vcx.mk_conj(&conjs)))
}

/// Opening (`unfold`) or closing (`fold`) the atoms of the value of `ty` at
/// `addr`, if it is of an `#[im_opaque]` struct: its place is expanded into
/// its fields, or collapsed back. Opened, the dependent atom first, as its
/// amount reads the atom; closed, the other way round. The dependent atom
/// only where its amount is positive.
pub(crate) fn open_or_close<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
    open: bool,
) -> Result<Vec<vir::Stmt<'vir>>, EncodeFullError<'vir, E>> {
    let Some(did) = ty.ty.im_opaque else {
        return Ok(Vec::new());
    };
    let atoms = deps.require_ref::<ImOpaqueEnc>(did)?;
    let args = deps.require_dep::<GArgsTyEnc>(ty.args)?;
    let (tys, consts) = (args.get_ty(), args.get_const());
    let vcx = vir::with_vcx(|vcx| vcx);
    let fold_or_unfold = |app| {
        if open {
            vcx.mk_unfold_stmt(app)
        } else {
            vcx.mk_fold_stmt(app)
        }
    };
    let mut stmts = vec![fold_or_unfold(
        atoms.atom.call()(addr, tys, consts)(Some(vcx.mk_full_perm())),
    )];
    if let Some(dependent) = atoms.dependent {
        let amount = atoms.dependent_amount.unwrap().call()(addr, tys, consts);
        let positive = vcx
            .mk_bin_op_expr(vir::BinOpKind::CmpGt, amount, vcx.mk_no_perm())
            .downcast_ty();
        let stmt = fold_or_unfold(dependent.call()(addr, tys, consts)(Some(amount)));
        let stmt = vcx.mk_if_stmt(positive, vcx.alloc_slice(&[stmt]), &[]);
        if open {
            stmts.insert(0, stmt);
        } else {
            stmts.push(stmt);
        }
    }
    Ok(stmts)
}

/// The permissions to the atoms of the places a loop invariant keeps, granted
/// once per key: a place and a reference to it reach the same atoms.
pub(crate) fn loop_footprint_assertion<'vir>(entries: &[FpEntry<'vir>]) -> vir::ExprBool<'vir> {
    let merged = entries
        .iter()
        .map(|entry| FpEntry {
            shared: true,
            ..*entry
        })
        .collect::<Vec<_>>();
    footprint_assertion(&merged)
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
                let Some(same) = entry.same_key(earlier) else {
                    continue;
                };
                if entry.shared && earlier.shared {
                    // Granted by the earlier entry, where that one is.
                    let granted = match earlier.condition {
                        Some(condition) => vcx.mk_conj(&[same, condition]),
                        None => same,
                    };
                    perm = vcx.mk_ternary_expr(granted, vcx.mk_no_perm(), perm);
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

/// The re-read of a guard at `addr` (a `#[ptr_holds]` sharer, whose
/// predicate is held) against the shared atom (which is held): where the
/// pointee it holds is the contents of an `#[im_perm]` of the atom, and
/// the atom holds some of them, the two are the same. Viper derives this
/// when both predicates are unfolded, but not across a call that returned
/// the atom (the paper's stability of a token's value).
pub(crate) fn guard_reread<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
) -> Result<Option<vir::ExprBool<'vir>>, EncodeFullError<'vir, E>> {
    let vcx = vir::with_vcx(|vcx| vcx);
    if !matches!(ty.ty.special, crate::encoders::ty::RustTySpecial::Holds(..)) {
        return Ok(None);
    }
    let guard = deps.require_dep::<TyUseImpureEnc>(ty)?;
    let Some(shared) = shared_target(deps, ty, addr, guard.ref_to_snap(addr))? else {
        return Ok(None);
    };
    let Some(did) = shared.ty.ty.im_opaque else {
        return Ok(None);
    };
    let atoms = deps.require_ref::<ImOpaqueEnc>(did)?;
    let args = deps.require_dep::<GArgsTyEnc>(shared.ty.args)?;
    let held_addr = guard
        .expect_structlike()
        .fields
        .last()
        .unwrap()
        .field_ref(addr);
    let held_ty = ty
        .ty
        .expect_structlike()
        .fields
        .last()
        .unwrap()
        .decompose_context(ty.ty.params, ty.args);
    let held = deps
        .require_dep::<TyUseImpureEnc>(held_ty)?
        .ref_to_snap(held_addr);
    let mut conjs = Vec::new();
    for (kind, field, method) in im_amounts(did) {
        if kind != ImAmountKind::Perm {
            continue;
        }
        let contents = contents_of(deps, shared.ty, shared.addr, field.unwrap())?;
        let amount = atoms.amount(method).call()(shared.addr, args.get_ty(), args.get_const());
        let same = vcx.mk_conj(&[
            vcx.mk_eq_expr(held_addr, contents.addr),
            vcx.mk_bin_op_expr(vir::BinOpKind::CmpGt, amount, vcx.mk_no_perm())
                .downcast_ty(),
        ]);
        conjs.push(
            vcx.mk_bin_op_expr(
                vir::BinOpKind::Implies,
                same,
                vcx.mk_eq_expr(held, contents.snap()),
            )
            .downcast_ty(),
        );
    }
    if conjs.is_empty() {
        return Ok(None);
    }
    let atom =
        atoms.atom.call()(shared.addr, args.get_ty(), args.get_const())(Some(vcx.mk_wildcard()));
    Ok(Some(vcx.mk_unfolding_expr(
        guard.ref_to_pred_app(addr, Some(vcx.mk_wildcard())),
        vcx.mk_unfolding_expr(atom, vcx.mk_conj(&conjs)),
    )))
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
    deep_with(deps, ty, addr, snap, DeepMode::Call)
}

/// The deep value of the reference `snap` of type `ty` to an `#[im_opaque]`
/// value whose atom is open (an `#[im_open]` function's parameter): read
/// from the atom's contents.
pub(crate) fn opened_deep_snap<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    snap: vir::ExprSnap<'vir>,
) -> Result<vir::ExprSnap<'vir>, EncodeFullError<'vir, E>> {
    let TySpecifics::ImmRef(data) = &ty.ty.specifics else {
        unreachable!("`#[im_open]` takes a shared reference")
    };
    let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
    let snap_c = snap.downcast_ty();
    let referent = data.referent.decompose_normalize(ty.args);
    let ref_use = ty_use.expect_immref();
    let r_addr = ref_use.addr_access(snap_c);
    let value = deep_with(
        deps,
        referent,
        r_addr,
        ref_use.value_access(snap_c),
        DeepMode::Opened,
    )?;
    Ok(ref_use
        .prim_to_snap(r_addr, ref_use.metadata_access(snap_c), value)
        .upcast_ty())
}

/// How [`deep_with`] reads the atoms of `#[im_opaque]` and `#[im_shares]`
/// values.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DeepMode {
    /// Through their deep functions ([`DeepFnEnc`]): a deep value contains no
    /// `unfolding`, which Silicon cannot match in a wand (a pledge).
    Call,
    /// By unfolding the atoms, as in the deep functions' bodies.
    Inline,
    /// Inside the definition of the atom of the outermost `#[im_opaque]`
    /// value, where only the atoms it folds can be read.
    Atom,
    /// The outermost `#[im_opaque]` value's atom is open (an `#[im_open]`
    /// function): its contents are read directly, nested values through
    /// their deep functions.
    Opened,
}

fn deep_with<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
    snap: vir::ExprSnap<'vir>,
    mode: DeepMode,
) -> Result<vir::ExprSnap<'vir>, EncodeFullError<'vir, E>> {
    let vcx = vir::with_vcx(|vcx| vcx);
    if !reaches_im(ty) {
        return Ok(snap);
    }
    if mode == DeepMode::Call
        && let Some(did) = ty.ty.im_opaque.or(ty.ty.im_annotated)
    {
        let deep = deps.require_dep::<DeepFnEnc>(did)?;
        let args = deps.require_dep::<GArgsTyEnc>(ty.args)?;
        return Ok(deep.call()(addr, snap, args.get_ty(), args.get_const()));
    }
    let own = mode != DeepMode::Atom;
    let nested = match mode {
        DeepMode::Call | DeepMode::Opened => DeepMode::Call,
        DeepMode::Inline | DeepMode::Atom => DeepMode::Inline,
    };
    if let Some(did) = ty.ty.im_opaque {
        let atoms = deps.require_ref::<ImOpaqueEnc>(did)?;
        let args = deps.require_dep::<GArgsTyEnc>(ty.args)?;
        let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
        let variant = ty_use.expect_variant_opt(None);
        let fields = &ty.ty.expect_structlike().fields;
        let snap_c = snap.downcast_ty();
        let amounts = im_amounts(did);
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
            field_snaps.push(match perm {
                Some((_, _, method)) if own => {
                    let contents = contents_of(deps, ty, addr, abi::FieldIdx::from_usize(idx))?;
                    let amount =
                        atoms.amount(*method).call()(addr, args.get_ty(), args.get_const());
                    let held = vcx
                        .mk_bin_op_expr(vir::BinOpKind::CmpGt, amount, vcx.mk_no_perm())
                        .downcast_ty();
                    // A plain field is a slot itself, as in the shallow
                    // snapshot where the atom holds none of it.
                    if field_ty.ty.special != crate::encoders::ty::RustTySpecial::UnsafeCell {
                        field_snaps.push(vcx.mk_ternary_expr(
                            held,
                            proj.generic_to_caller(contents.snap()),
                            field_snap,
                        ));
                        continue;
                    }
                    let unset = deps.require_dep::<ImUnsetEnc>(())?;
                    let cell_ty_args = deps.require_dep::<GArgsTyEnc>(field_ty.args)?;
                    let slot = vcx.mk_ternary_expr(
                        held,
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
                _ => deep_with(deps, field_ty, proj.field_ref(addr), field_snap, nested)?,
            });
        }
        let value = variant.field_snaps_to_snap(field_snaps).upcast_ty();
        if !own || mode == DeepMode::Opened {
            return Ok(value);
        }
        let unfold =
            atoms.atom.call()(addr, args.get_ty(), args.get_const())(Some(vcx.mk_wildcard()));
        return Ok(vcx.mk_unfolding_expr(unfold, value));
    }
    let shared = shared_target(deps, ty, addr, snap)?;
    match &ty.ty.specifics {
        // Read through the value's generic atom.
        TySpecifics::Param(_) => {
            let param = deps.require_dep::<ParamImEnc>(())?;
            let ty_expr = deps.require_dep::<crate::encoders::ty::generics::TyExprEnc>(ty)?;
            Ok(param.snap.call()(addr, snap, ty_expr))
        }
        TySpecifics::StructLike(data) if !data.fields.is_empty() => {
            let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
            let variant = ty_use.expect_variant_opt(None);
            let snap_c = snap.downcast_ty();
            let mut field_snaps = Vec::with_capacity(data.fields.len());
            for (idx, field) in data.fields.iter().enumerate() {
                let proj = &variant.fields[idx];
                field_snaps.push(match field.address {
                    crate::encoders::ty::RustFieldAddress::Constant => deep_with(
                        deps,
                        field.decompose_normalize(ty.args),
                        proj.field_ref(addr),
                        proj.read(snap_c),
                        nested,
                    )?,
                    // The value a box owns, deep.
                    crate::encoders::ty::RustFieldAddress::Dynamic
                        if ty.ty.special == crate::encoders::ty::RustTySpecial::Box =>
                    {
                        deep_with(
                            deps,
                            field.decompose_normalize(ty.args),
                            variant.box_address_access(snap_c),
                            proj.read(snap_c),
                            nested,
                        )?
                    }
                    // A held pointee (of a `#[ptr_holds]`) is shallow.
                    crate::encoders::ty::RustFieldAddress::Dynamic => proj.read(snap_c),
                    // The shared value, deep.
                    crate::encoders::ty::RustFieldAddress::Slot => {
                        let shared = shared.as_ref().unwrap();
                        let deep = deep_with(deps, shared.ty, shared.addr, shared.snap, nested)?;
                        let slot_ty = field.decompose_normalize(ty.args);
                        deps.require_dep::<TyUsePureEnc>(slot_ty)?
                            .expect_immref()
                            .prim_to_snap(shared.addr, shared.metadata, deep)
                            .upcast_ty()
                    }
                });
            }
            let mut value = variant.field_snaps_to_snap(field_snaps).upcast_ty();
            // The interior-mutable values a sharer's fields reach are inside
            // the shared atoms.
            if let Some(shared) = shared {
                let mut entries = Vec::new();
                footprint_into(
                    deps,
                    shared.ty,
                    shared.addr,
                    shared.snap,
                    true,
                    true,
                    false,
                    &mut entries,
                )?;
                for entry in entries.iter().rev() {
                    value = vcx.mk_unfolding_expr(entry.app(vcx.mk_wildcard()), value);
                }
                // Where nothing is shared, the value is shallow.
                if let Some(condition) = shared.condition {
                    value = vcx.mk_ternary_expr(condition, value, snap);
                }
            }
            Ok(value)
        }
        TySpecifics::ImmRef(data) => {
            let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
            let snap_c = snap.downcast_ty();
            let referent = data.referent.decompose_normalize(ty.args);
            let ref_use = ty_use.expect_immref();
            let r_addr = ref_use.addr_access(snap_c);
            let value = deep_with(deps, referent, r_addr, ref_use.value_access(snap_c), nested)?;
            Ok(ref_use
                .prim_to_snap(r_addr, ref_use.metadata_access(snap_c), value)
                .upcast_ty())
        }
        // The variant the value is, with deep fields.
        TySpecifics::EnumLike(data) => {
            let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
            let snap_c = snap.downcast_ty();
            let mut value = snap;
            for variant in &data.variants {
                let fields = &variant.inner.fields;
                if !fields
                    .iter()
                    .any(|f| reaches_im(f.decompose_normalize(ty.args)))
                {
                    continue;
                }
                let variant_use = ty_use.expect_variant_opt(Some(variant.data.vid));
                let mut field_snaps = Vec::with_capacity(fields.len());
                for (idx, field) in fields.iter().enumerate() {
                    let proj = &variant_use.fields[idx];
                    field_snaps.push(deep_with(
                        deps,
                        field.decompose_normalize(ty.args),
                        proj.field_ref(addr),
                        proj.read(snap_c),
                        nested,
                    )?);
                }
                let deep = variant_use.field_snaps_to_snap(field_snaps).upcast_ty();
                let is = is_variant(deps, ty, data, variant.data, snap_c)?;
                value = vcx.mk_ternary_expr(is, deep, value);
            }
            Ok(value)
        }
        _ => Ok(snap),
    }
}

/// A type in `ty` whose contents reach interior-mutable values
/// (`RefCell<Cell<_>>`, `Rc<RefCell<_>>`, `[Cell<_>; N]`), if any. The
/// encoding does not support these yet: the contents are held generically
/// (or, for arrays and slices, element-wise without footprints and deep
/// values), without the atoms they would reach.
pub(crate) fn nested_im<'tcx>(ty: ty::Ty<'tcx>, params: GParams<'tcx>) -> Option<ty::Ty<'tcx>> {
    ty.walk()
        .filter_map(|arg| arg.as_type())
        .find(|inner| match *inner.kind() {
            ty::TyKind::Adt(adt, args)
                if (is_im_opaque(adt.did()) || is_im_sharer(adt.did()) || adt.is_unsafe_cell())
                    && args.types().any(|arg| {
                        reaches_concrete_im(RustTyDecomposition::from_ty(arg, params))
                    }) =>
            {
                true
            }
            ty::TyKind::Adt(adt, _) if unsupported_im_fields(adt.did(), *inner, params) => true,
            _ => reaches_im_array(RustTyDecomposition::from_ty(*inner, params)),
        })
}

/// Whether the struct `did` (as `ty`) has fields whose interior-mutable values
/// the encoding cannot follow yet: those an `#[im_opaque]` struct's atom folds
/// that are not inline at fixed addresses (e.g. inside an enum or a box), or
/// those a `#[ptr_holds]` struct holds inline besides its pointer.
fn unsupported_im_fields<'tcx>(did: DefId, ty: ty::Ty<'tcx>, params: GParams<'tcx>) -> bool {
    fn fixed(ty: RustTyDecomposition<'_>) -> bool {
        if ty.ty.im_opaque.is_some() || !reaches_concrete_im(ty) {
            return true;
        }
        match &ty.ty.specifics {
            TySpecifics::StructLike(data)
                if ty.ty.special != crate::encoders::ty::RustTySpecial::Box
                    && data.fields.iter().all(|f| {
                        matches!(f.address, crate::encoders::ty::RustFieldAddress::Constant)
                    }) =>
            {
                data.fields
                    .iter()
                    .all(|f| fixed(f.decompose_normalize(ty.args)))
            }
            _ => false,
        }
    }
    fn inline(ty: RustTyDecomposition<'_>) -> bool {
        if ty.ty.im_opaque.is_some() || ty.ty.im_annotated.is_some() {
            return true;
        }
        match &ty.ty.specifics {
            TySpecifics::StructLike(data)
                if ty.ty.special != crate::encoders::ty::RustTySpecial::Box =>
            {
                data.fields
                    .iter()
                    .any(|f| inline(f.decompose_normalize(ty.args)))
            }
            TySpecifics::EnumLike(data) => data
                .variants
                .iter()
                .flat_map(|variant| variant.inner.fields.iter())
                .any(|f| inline(f.decompose_normalize(ty.args))),
            _ => false,
        }
    }
    let decomp = RustTyDecomposition::from_ty(ty, params);
    let TySpecifics::StructLike(data) = &decomp.ty.specifics else {
        return false;
    };
    // The struct's own fields (not the pointee a `#[ptr_holds]` adds).
    let count = vir::with_vcx(|vcx| vcx.tcx().adt_def(did).non_enum_variant().fields.len());
    let fields = || {
        data.fields
            .iter()
            .take(count)
            .enumerate()
            .map(|(idx, f)| (abi::FieldIdx::from_usize(idx), f.decompose_normalize(decomp.args)))
    };
    if is_im_opaque(did) {
        let annotated = im_amounts(did)
            .into_iter()
            .filter_map(|(_, field, _)| field)
            .collect::<Vec<_>>();
        return fields().any(|(idx, field)| !annotated.contains(&idx) && !fixed(field));
    }
    if let Some((ptr, _)) = ptr_holds_field(did) {
        return fields().any(|(idx, field)| idx != ptr && inline(field));
    }
    false
}

/// Whether an unsize coercion from `src` to `dst` hides interior-mutable
/// values behind a trait object, whose footprint the encoding cannot follow
/// (not supported yet).
pub(crate) fn hides_im<'tcx>(src: ty::Ty<'tcx>, dst: ty::Ty<'tcx>, params: GParams<'tcx>) -> bool {
    dst.walk()
        .filter_map(|arg| arg.as_type())
        .any(|ty| matches!(ty.kind(), ty::TyKind::Dynamic(..)))
        && reaches_im(RustTyDecomposition::from_ty(src, params))
}

/// Whether a value of the caller's type `ty`, passed as a value of a callee's
/// declared type `declared`, may reach interior-mutable values hidden behind an
/// associated or opaque type of `declared`, which carries no generic atom (not
/// supported yet).
pub(crate) fn alias_hides_im<'tcx>(declared: ty::Ty<'tcx>, ty: RustTyDecomposition<'tcx>) -> bool {
    declared
        .walk()
        .filter_map(|arg| arg.as_type())
        .any(|inner| matches!(inner.kind(), ty::TyKind::Alias(..)))
        && reaches_im(ty)
}

/// Whether a value of `ty` holds, inline or behind references, an array or
/// slice of values that reach `#[im_opaque]` ones.
fn reaches_im_array(ty: RustTyDecomposition<'_>) -> bool {
    fn go<'tcx>(ty: RustTyDecomposition<'tcx>, seen: &mut Vec<RustTyDecomposition<'tcx>>) -> bool {
        if seen.contains(&ty) {
            return false;
        }
        seen.push(ty);
        match &ty.ty.specifics {
            TySpecifics::ArrayLike(data) => {
                let elem = data.data.decompose_normalize(ty.args);
                reaches_concrete_im(elem) || go(elem, seen)
            }
            TySpecifics::StructLike(data) => data
                .fields
                .iter()
                .any(|f| go(f.decompose_normalize(ty.args), seen)),
            TySpecifics::ImmRef(data) | TySpecifics::MutRef(data) => {
                go(data.referent.decompose_normalize(ty.args), seen)
            }
            TySpecifics::EnumLike(data) => data
                .variants
                .iter()
                .flat_map(|variant| variant.inner.fields.iter())
                .any(|f| go(f.decompose_normalize(ty.args), seen)),
            _ => false,
        }
    }
    go(ty, &mut Vec::new())
}

/// Whether a value of `ty` reaches an `#[im_opaque]` value, i.e. whether its
/// deep snapshot differs from its shallow one. A value of a type parameter
/// may, if the crate uses interior mutability at all.
pub(crate) fn reaches_im(ty: RustTyDecomposition<'_>) -> bool {
    reaches_im_with(ty, program_uses_im())
}

/// Whether a value of `ty` reaches an `#[im_opaque]` value whatever its type
/// parameters are instantiated with.
fn reaches_concrete_im(ty: RustTyDecomposition<'_>) -> bool {
    reaches_im_with(ty, false)
}

fn reaches_im_with(ty: RustTyDecomposition<'_>, params: bool) -> bool {
    fn go<'tcx>(
        ty: RustTyDecomposition<'tcx>,
        params: bool,
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
            TySpecifics::Param(_) => params && param_reaches_im(ty),
            TySpecifics::StructLike(data) => data
                .fields
                .iter()
                .any(|f| go(f.decompose_normalize(ty.args), params, seen)),
            TySpecifics::ImmRef(data) | TySpecifics::MutRef(data) => {
                go(data.referent.decompose_normalize(ty.args), params, seen)
            }
            TySpecifics::EnumLike(data) => data
                .variants
                .iter()
                .flat_map(|variant| variant.inner.fields.iter())
                .any(|f| go(f.decompose_normalize(ty.args), params, seen)),
            _ => false,
        }
    }
    go(ty, params, &mut Vec::new())
}

/// The deep snapshot of a value of the `#[im_opaque]` or `#[im_shares]`
/// struct `did`, as a function of its address and shallow snapshot that reads
/// its footprint.
pub(crate) struct DeepFnEnc;

pub(crate) type DeepFn<'vir> =
    vir::FunctionIdn<'vir, (vir::Ref, vir::Snap, vir::ManyTyVal, vir::ManyCSnap), vir::Snap>;

impl TaskEncoder for DeepFnEnc {
    task_encoder::encoder_cache!(DeepFnEnc);
    const ENCODER_NAME: &'static str = "interior mutability deep snapshot encoder";
    type TaskDescription<'vir> = DefId;
    type OutputFullDependency<'vir> = DeepFn<'vir>;
    type OutputFullLocal<'vir> = vir::Function<'vir>;

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
            let identity = vcx
                .tcx()
                .type_of(*did)
                .instantiate_identity()
                .skip_normalization();
            let ty = RustTyDecomposition::from_ty(identity, params);
            let snap_ty = deps.require_ref::<TyUsePureEnc>(ty)?.snapshot;
            let deep: DeepFn<'vir> = vir::FunctionIdn::new(
                vir::vir_format_identifier!(
                    vcx,
                    "deep_{}",
                    vir::ViperIdent::from_def_id(vcx, *did)
                ),
                (
                    vir::TYPE_REF,
                    snap_ty,
                    generics.ty_args(),
                    generics.const_args(),
                ),
                snap_ty,
            );
            deps.emit_output_ref(*did, ())?;

            let addr_decl = vcx.mk_local_decl("addr", vir::TYPE_REF);
            let snap_decl = vcx.mk_local_decl("snap", snap_ty);
            let addr = vcx.mk_local_ex(addr_decl);
            let snap = vcx.mk_local_ex(snap_decl);
            let mut entries = Vec::new();
            footprint_into(deps, ty, addr, snap, false, false, false, &mut entries)?;
            // The deep value reads no dependent atom.
            let pres = entries
                .iter()
                .filter(|entry| entry.amount.is_none())
                .map(|entry| {
                    let acc = vcx.mk_predicate_app_expr(entry.app(vcx.mk_wildcard()));
                    match entry.condition {
                        Some(condition) => vcx
                            .mk_bin_op_expr(vir::BinOpKind::Implies, condition, acc)
                            .downcast_ty(),
                        None => acc,
                    }
                })
                .collect::<Vec<_>>();
            let body = deep_with(deps, ty, addr, snap, DeepMode::Inline)?;
            Ok((
                vcx.mk_function(
                    deep,
                    (
                        addr_decl,
                        snap_decl,
                        generics.ty_decls(),
                        generics.const_decls(),
                    ),
                    vcx.alloc_slice(&pres),
                    &[],
                    None,
                    Some(body),
                ),
                deep,
            ))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for function in Self::all_outputs_local_no_errors(program) {
            program.add_function(function);
        }
    }
}

/// The inverse of the address of the field `field` of the struct `did`, for
/// `#[container_of]`. Field addresses are offsets, so the two are mutually
/// inverse: `container_of(field_addr(a, T..), T..) == a` and
/// `field_addr(container_of(l, T..), T..) == l`.
pub(crate) struct ContainerOfEnc;

pub(crate) type ContainerOf<'vir> =
    vir::FunctionIdn<'vir, (vir::Ref, vir::ManyTyVal, vir::ManyCSnap), vir::Ref>;

impl TaskEncoder for ContainerOfEnc {
    task_encoder::encoder_cache!(ContainerOfEnc);
    const ENCODER_NAME: &'static str = "container-of encoder";
    type TaskDescription<'vir> = (DefId, abi::FieldIdx);
    type OutputFullDependency<'vir> = ContainerOf<'vir>;
    type OutputFullLocal<'vir> = vir::Domain<'vir>;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut task_encoder::TaskEncoderDependencies<'vir, Self>,
    ) -> task_encoder::EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;
        vir::with_vcx(|vcx| {
            let (did, field) = *task_key;
            let params = GParams::from(did);
            let generics = deps.require_dep::<GenericParamsEnc>(params)?;
            let name = vir::vir_format_identifier!(
                vcx,
                "container_of_{}_{}",
                vir::ViperIdent::from_def_id(vcx, did),
                field.as_usize()
            );
            let inverse: ContainerOf<'vir> = vir::FunctionIdn::new(
                name,
                (vir::TYPE_REF, generics.ty_args(), generics.const_args()),
                vir::TYPE_REF,
            );
            let identity = vcx
                .tcx()
                .type_of(did)
                .instantiate_identity()
                .skip_normalization();
            let ty = RustTyDecomposition::from_ty(identity, params);
            let self_decl = vcx.mk_local_decl("self", vir::TYPE_REF);
            let self_ex = vcx.mk_local_ex(self_decl);
            let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
            let field_use = ty_use.expect_variant_opt(None).fields[field.as_usize()];
            let field_addr = field_use.field_ref(self_ex);
            let lhs = inverse.call()(field_addr, generics.ty_exprs(), generics.const_exprs());
            let left = vir::expr! {
                forall [self_decl], ..[generics.ty_decls()], ..[generics.const_decls()] :: {[field_addr]}
                    [vcx.mk_eq_expr(lhs, self_ex)]
            };
            let container = inverse.call()(self_ex, generics.ty_exprs(), generics.const_exprs());
            let right = vir::expr! {
                forall [self_decl], ..[generics.ty_decls()], ..[generics.const_decls()] :: {[container]}
                    [vcx.mk_eq_expr(field_use.field_ref(container), self_ex)]
            };
            let domain = vcx.mk_domain(
                vir::vir_format_identifier!(vcx, "def_{}", name),
                &[],
                vcx.alloc_slice(&[
                    vcx.mk_domain_axiom(vir::vir_format_identifier!(vcx, "ax_{}_left", name), left),
                    vcx.mk_domain_axiom(
                        vir::vir_format_identifier!(vcx, "ax_{}_right", name),
                        right,
                    ),
                ]),
                vcx.alloc_slice(&[vcx.mk_domain_function(inverse, false, None)]),
                None,
            );
            Ok((domain, inverse))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for domain in Self::all_outputs_local_no_errors(program) {
            program.add_domain(domain);
        }
    }
}

/// Whether the crate's bodies mention a type that holds an interior-mutable
/// one. Only then can a type parameter be instantiated with one, and generic
/// code holds the footprints of the values of its type parameters (see
/// [`ParamImEnc`]).
pub(crate) fn program_uses_im() -> bool {
    thread_local! {
        static USES_IM: std::cell::LazyCell<bool> = std::cell::LazyCell::new(|| {
            vir::with_vcx(|vcx| {
                let mut seen = FxHashSet::default();
                vcx.tcx().hir_body_owners().any(|owner| {
                    vcx.tcx()
                        .typeck(owner)
                        .node_types()
                        .items_in_stable_order()
                        .into_iter()
                        .any(|(_, ty)| holds_im(vcx.tcx(), *ty, &mut seen))
                })
            })
        });
    }
    USES_IM.with(|uses| **uses)
}

/// Whether `ty` mentions a type that holds an interior-mutable one (in its
/// fields, transitively).
fn holds_im<'tcx>(
    tcx: ty::TyCtxt<'tcx>,
    ty: ty::Ty<'tcx>,
    seen: &mut FxHashSet<ty::Ty<'tcx>>,
) -> bool {
    ty.walk().filter_map(|arg| arg.as_type()).any(|inner| {
        let ty::TyKind::Adt(adt, args) = *inner.kind() else {
            return false;
        };
        if is_im_opaque(adt.did()) || is_im_sharer(adt.did()) {
            return true;
        }
        seen.insert(inner)
            && adt
                .all_fields()
                .any(|field| holds_im(tcx, field.ty(tcx, args).skip_normalization(), seen))
    })
}

/// The type parameters of `def_id` that are the contents of interior-mutable
/// values, which reach none themselves (see `nested_im`), as a set of
/// parameter indices: all those of an interior-mutable type, and those that
/// occur directly as arguments of an interior-mutable type (or of an
/// `UnsafeCell`) in the self type of a method's impl or in a function's
/// inputs (e.g. `T` of `Cell<T>::get` or of `refcell_value<T>(&RefCell<T>)`).
pub(crate) fn im_contents_params(def_id: DefId) -> u64 {
    vir::with_vcx(|vcx| {
        let def_id = vcx.tcx().typeck_root_def_id(def_id);
        let is_im = |did: DefId| is_im_opaque(did) || is_im_sharer(did);
        let kind = vcx.tcx().def_kind(def_id);
        if matches!(
            kind,
            prusti_rustc_interface::hir::def::DefKind::Struct
                | prusti_rustc_interface::hir::def::DefKind::Enum
        ) {
            return if is_im(def_id) { u64::MAX } else { 0 };
        }
        let mut contents = 0;
        let mut add = |ty: ty::Ty<'_>| {
            for inner in ty.walk().filter_map(|arg| arg.as_type()) {
                let ty::TyKind::Adt(adt, args) = *inner.kind() else {
                    continue;
                };
                if !adt.is_unsafe_cell() && !is_im(adt.did()) {
                    continue;
                }
                for arg in args.types() {
                    if let ty::TyKind::Param(param) = *arg.kind()
                        && param.index < 64
                    {
                        contents |= 1 << param.index;
                    }
                }
            }
        };
        if let Some(impl_did) = vcx.tcx().impl_of_assoc(def_id) {
            add(vcx.tcx().type_of(impl_did).instantiate_identity().skip_normalization());
        }
        if matches!(
            kind,
            prusti_rustc_interface::hir::def::DefKind::Fn
                | prusti_rustc_interface::hir::def::DefKind::AssocFn
        ) {
            for input in vcx
                .tcx()
                .fn_sig(def_id)
                .instantiate_identity()
                .skip_normalization()
                .skip_binder()
                .inputs()
            {
                add(*input);
            }
        }
        contents
    })
}

/// Whether `ty` is a type parameter of `def_id` that is the contents of
/// interior-mutable values (see [`im_contents_params`]).
pub(crate) fn is_im_contents(def_id: DefId, ty: ty::Ty<'_>) -> bool {
    match *ty.kind() {
        ty::TyKind::Param(param) => {
            param.index < 64 && im_contents_params(def_id) & (1 << param.index) != 0
        }
        _ => false,
    }
}

/// Whether a value of the type parameter `ty` may reach interior-mutable
/// values: if the crate uses interior mutability at all, unless it is the
/// contents of one (see [`im_contents_params`]). Trait objects and
/// projections, decomposed like type parameters, do not carry generic atoms.
fn param_reaches_im(ty: RustTyDecomposition<'_>) -> bool {
    let Some(ty::TyKind::Param(param)) = ty
        .args
        .args()
        .first()
        .and_then(|arg| arg.as_type())
        .map(|ty| *ty.kind())
    else {
        return false;
    };
    program_uses_im()
        && (param.index >= 64 || ty.args.context().im_contents() & (1 << param.index) == 0)
}

/// The atom of a value of a type parameter `T`, holding the footprint of
/// whatever `T` is instantiated with, and the deep value read through it:
/// `predicate p_im_Param(self: Ref, T: Type)` and
/// `function im_snap_Param(self: Ref, s: s_Param, T: Type): s_Param`, both
/// abstract. At a call instantiating `T`, the caller converts the concrete
/// atoms into the generic one and back (see [`to_generic`]).
pub(crate) struct ParamImEnc;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ParamIm<'vir> {
    pub(crate) atom: ImAtomIdn<'vir>,
    pub(crate) snap: vir::FunctionIdn<'vir, (vir::Ref, vir::Snap, vir::TyVal), vir::Snap>,
}

impl TaskEncoder for ParamImEnc {
    task_encoder::encoder_cache!(ParamImEnc);
    const ENCODER_NAME: &'static str = "interior mutability type parameter atom encoder";
    type TaskDescription<'vir> = ();
    type OutputFullDependency<'vir> = ParamIm<'vir>;
    type OutputFullLocal<'vir> = (vir::Predicate<'vir>, vir::Function<'vir>);

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        _task_key: &Self::TaskKey<'vir>,
        deps: &mut task_encoder::TaskEncoderDependencies<'vir, Self>,
    ) -> task_encoder::EncodeFullResult<'vir, Self> {
        deps.emit_output_ref((), ())?;
        vir::with_vcx(|vcx| {
            let s_param = vir::TYPE_PSNAP.upcast_ty();
            let atom: ImAtomIdn<'vir> = vir::PredicateIdn::new(
                vir::vir_format_identifier!(vcx, "p_im_Param"),
                (vir::TYPE_REF, vcx.alloc_slice(&[vir::TYPE_TYVAL]), Default::default()),
            );
            let snap = vir::FunctionIdn::new(
                vir::vir_format_identifier!(vcx, "im_snap_Param"),
                (vir::TYPE_REF, s_param, vir::TYPE_TYVAL),
                s_param,
            );
            let self_decl = vcx.mk_local_decl("self", vir::TYPE_REF);
            let s_decl = vcx.mk_local_decl("s", s_param);
            let ty_decl = vcx.mk_local_decl("T", vir::TYPE_TYVAL);
            let self_ex = vcx.mk_local_ex(self_decl);
            let ty_ex = vcx.mk_local_ex(ty_decl);
            let predicate =
                vcx.mk_predicate(atom, (self_decl, vcx.alloc_slice(&[ty_decl]), &[]), None);
            let held = atom.call()(self_ex, vcx.alloc_slice(&[ty_ex]), &[])(Some(
                vcx.mk_wildcard(),
            ));
            let function = vcx.mk_function(
                snap,
                (self_decl, s_decl, ty_decl),
                vcx.alloc_slice(&[vcx.mk_predicate_app_expr(held)]),
                &[],
                None,
                None,
            );
            Ok(((predicate, function), ParamIm { atom, snap }))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for (predicate, function) in Self::all_outputs_local_no_errors(program) {
            program.add_predicate(predicate);
            program.add_function(function);
        }
    }
}

/// The permission to the generic atom of the value at `addr` of the type
/// `ty_expr`, if `condition`.
pub(crate) fn param_atom_acc<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    addr: vir::ExprRef<'vir>,
    ty_expr: vir::ExprTyVal<'vir>,
) -> Result<vir::ExprBool<'vir>, EncodeFullError<'vir, E>> {
    let param = deps.require_dep::<ParamImEnc>(())?;
    Ok(vir::with_vcx(|vcx| {
        vcx.mk_predicate_app_expr(param.atom.call()(addr, vcx.alloc_slice(&[ty_expr]), &[])(
            Some(vcx.mk_full_perm()),
        ))
    }))
}

/// The deep value read through the generic atom of the value at `addr` of
/// the type `ty_expr` with the generic snapshot `snap`.
pub(crate) fn param_atom_snap<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    addr: vir::ExprRef<'vir>,
    snap: vir::ExprSnap<'vir>,
    ty_expr: vir::ExprTyVal<'vir>,
) -> Result<vir::ExprSnap<'vir>, EncodeFullError<'vir, E>> {
    let param = deps.require_dep::<ParamImEnc>(())?;
    Ok(param.snap.call()(addr, snap, ty_expr))
}

/// A value of one of a callee's type parameters inside an argument or the
/// result of a call: the parameter, its instantiation (in the caller's
/// context), and where the value is.
pub(crate) struct GenericPos<'vir> {
    pub(crate) param: ty::Ty<'vir>,
    pub(crate) ty: RustTyDecomposition<'vir>,
    pub(crate) addr: vir::ExprRef<'vir>,
    pub(crate) snap: vir::ExprSnap<'vir>,
    /// Reached through a reference: its atoms come back after the call.
    pub(crate) reached: bool,
    /// Behind a mutable reference: its value may change in the call.
    pub(crate) mutable: bool,
    /// Inside a field (of the referent, if behind a reference).
    pub(crate) nested: bool,
    /// The instantiation is a type parameter of the caller.
    pub(crate) identity: bool,
    pub(crate) condition: Option<vir::ExprBool<'vir>>,
    /// For the referent of a mutable reference: how to read its snapshot
    /// anew at its address (it is held generically, as the reference type's
    /// parameter).
    pub(crate) reread: Option<(
        RustTyDecomposition<'vir>,
        crate::encoders::ty::generics::GArgCaster<'vir, crate::Pure>,
    )>,
}

impl<'vir> GenericPos<'vir> {
    /// The concrete atoms of the value, as reached from the argument or the
    /// result it is in (through a shared reference: possibly aliased).
    pub(crate) fn footprint<E: TaskEncoder>(
        &self,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> Result<Vec<FpEntry<'vir>>, EncodeFullError<'vir, E>> {
        let mut entries = footprint(deps, self.ty, self.addr, self.snap)?;
        for entry in &mut entries {
            entry.reached |= self.reached;
            entry.shared |= self.reached && !self.mutable;
            *entry = entry.with_condition(self.condition);
        }
        Ok(entries)
    }

    /// The snapshot of the value at `addr`, read anew (see `reread`).
    pub(crate) fn reread_snap<E: TaskEncoder>(
        &self,
        deps: &mut TaskEncoderDependencies<'vir, E>,
        addr: vir::ExprRef<'vir>,
    ) -> Result<Option<vir::ExprSnap<'vir>>, EncodeFullError<'vir, E>> {
        let Some((generic, caster)) = self.reread else {
            return Ok(None);
        };
        Ok(Some(caster.cast_to_caller_ctx(
            deps.require_dep::<TyUseImpureEnc>(generic)?
                .ref_to_snap(addr),
        )))
    }
}

/// The values of the callee's type parameters inside a value of the callee's
/// declared type `declared`, which is the value of the caller's type `ty` at
/// `addr` with snapshot `snap`. The positions mirror those of the callee's
/// footprint (see `footprint_into`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn generic_positions<'vir, E: TaskEncoder>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
    declared: ty::Ty<'vir>,
    ty: RustTyDecomposition<'vir>,
    addr: vir::ExprRef<'vir>,
    snap: vir::ExprSnap<'vir>,
    reached: bool,
    mutable: bool,
    out: &mut Vec<GenericPos<'vir>>,
) -> Result<(), EncodeFullError<'vir, E>> {
    if !program_uses_im() {
        return Ok(());
    }
    let vcx = vir::with_vcx(|vcx| vcx);
    let start = out.len();
    match *declared.kind() {
        ty::TyKind::Param(_) => out.push(GenericPos {
            param: declared,
            ty,
            addr,
            snap,
            reached,
            mutable,
            nested: false,
            identity: matches!(ty.ty.specifics, TySpecifics::Param(_)),
            condition: None,
            reread: None,
        }),
        ty::TyKind::Ref(_, inner, _) => match &ty.ty.specifics {
            TySpecifics::ImmRef(data) => {
                let ref_use = deps.require_dep::<TyUsePureEnc>(ty)?;
                let ref_use = ref_use.expect_immref();
                let snap_c = snap.downcast_ty();
                generic_positions(
                    deps,
                    inner,
                    data.referent.decompose_normalize(ty.args),
                    ref_use.addr_access(snap_c),
                    ref_use.value_access(snap_c),
                    true,
                    false,
                    out,
                )?;
            }
            TySpecifics::MutRef(data) => {
                let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
                let referent = data.referent.decompose_normalize(ty.args);
                let r_addr = ty_use.expect_mutref().deref_access(snap.downcast_ty());
                // As in `footprint_into`.
                let generic = data.referent.decompose_context(ty.ty.params, ty.args);
                let caster = deps.require_dep::<crate::GArgsCastEnc<crate::Pure>>(
                    data.referent
                        .decompose_compare_normalize(ty.ty.params, ty.args),
                )?;
                let r_snap = caster.cast_to_caller_ctx(
                    deps.require_dep::<TyUseImpureEnc>(generic)?
                        .ref_to_snap(r_addr),
                );
                let referent_start = out.len();
                // Behind a shared reference, it is not mutable.
                let mutable = mutable || !reached;
                generic_positions(deps, inner, referent, r_addr, r_snap, true, mutable, out)?;
                for pos in &mut out[referent_start..] {
                    if pos.nested {
                        continue;
                    }
                    // Behind another mutable reference, which the callee
                    // may point elsewhere: as inside a value.
                    if pos.reread.is_some() {
                        pos.nested = true;
                    } else {
                        pos.reread = Some((generic, caster));
                    }
                }
            }
            _ => (),
        },
        ty::TyKind::Adt(adt, dargs) if !is_im_opaque(adt.did()) && !is_im_sharer(adt.did()) => {
            let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
            let snap_c = snap.downcast_ty();
            match &ty.ty.specifics {
                // The value a box owns, at its pointee address.
                TySpecifics::StructLike(data) if adt.is_box() => {
                    let variant = ty_use.expect_variant_opt(None);
                    generic_positions(
                        deps,
                        dargs.type_at(0),
                        data.fields.last().unwrap().decompose_normalize(ty.args),
                        variant.box_address_access(snap_c),
                        variant.fields.last().unwrap().read(snap_c),
                        reached,
                        mutable,
                        out,
                    )?;
                }
                TySpecifics::StructLike(data)
                    if data.fields.iter().all(|f| {
                        matches!(f.address, crate::encoders::ty::RustFieldAddress::Constant)
                    }) =>
                {
                    for (idx, field) in data.fields.iter().enumerate() {
                        let declared_field = adt.non_enum_variant().fields
                            [abi::FieldIdx::from_usize(idx)]
                        .ty(vcx.tcx(), dargs)
                        .skip_normalization();
                        let proj = &ty_use.expect_variant_opt(None).fields[idx];
                        generic_positions(
                            deps,
                            declared_field,
                            field.decompose_normalize(ty.args),
                            proj.field_ref(addr),
                            proj.read(snap_c),
                            reached,
                            mutable,
                            out,
                        )?;
                    }
                }
                TySpecifics::EnumLike(data) => {
                    for variant in &data.variants {
                        let start = out.len();
                        let variant_use = ty_use.expect_variant_opt(Some(variant.data.vid));
                        for (idx, field) in variant.inner.fields.iter().enumerate() {
                            let declared_field = adt.variant(variant.data.vid).fields
                                [abi::FieldIdx::from_usize(idx)]
                            .ty(vcx.tcx(), dargs)
                            .skip_normalization();
                            let proj = &variant_use.fields[idx];
                            generic_positions(
                                deps,
                                declared_field,
                                field.decompose_normalize(ty.args),
                                proj.field_ref(addr),
                                proj.read(snap_c),
                                reached,
                                mutable,
                                out,
                            )?;
                        }
                        if out.len() == start {
                            continue;
                        }
                        let is = is_variant(deps, ty, data, variant.data, snap_c)?;
                        for pos in &mut out[start..] {
                            pos.condition = Some(match pos.condition {
                                Some(inner) => vcx.mk_conj(&[is, inner]),
                                None => is,
                            });
                        }
                    }
                }
                _ => (),
            }
        }
        ty::TyKind::Tuple(declared_fields) => {
            if let TySpecifics::StructLike(data) = &ty.ty.specifics {
                let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
                let snap_c = snap.downcast_ty();
                for (idx, field) in data.fields.iter().enumerate() {
                    let proj = &ty_use.expect_variant_opt(None).fields[idx];
                    generic_positions(
                        deps,
                        declared_fields[idx],
                        field.decompose_normalize(ty.args),
                        proj.field_ref(addr),
                        proj.read(snap_c),
                        reached,
                        mutable,
                        out,
                    )?;
                }
            }
        }
        _ => (),
    }
    if matches!(declared.kind(), ty::TyKind::Adt(..) | ty::TyKind::Tuple(_)) {
        for pos in &mut out[start..] {
            pos.nested = true;
        }
    }
    Ok(())
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
