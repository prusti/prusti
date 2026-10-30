//! Encoding for raw pointer types (`*const T` / `*mut T`).
//!
//! Modelled like a reference (see [`super::immref`]) but conservatively: the
//! snapshot carries the pointer address and the pointer metadata. the pointee
//! is still required for dereferences in pure code (such as specifications
//! of methods) but it may be null.

use crate::encoders::{
    TyUseImpureEnc, TyUsePureEnc,
    ty::{
        RustRaw, RustTyDatas,
        data::TyData,
        generics::{ParamTypEnc, TyExprEnc},
        impure::{PredicateBuilder, TyImpureEnc, TyImpureRaw, TyImpureRawData},
        optional::OptionalEnc,
        pure::{AdtBuilder, PureTyDatas, TyPureEnc, TyPureRaw, TyPureRawData},
    },
};
use task_encoder::{EncodeFullError, TaskEncoderDependencies};
use vir::CastType;

pub(crate) fn ty_pure<'vir>(
    task_key: &TyData<'vir, RustTyDatas>,
    data: &RustRaw<'vir>,
    deps: &mut TaskEncoderDependencies<'vir, TyPureEnc>,
    builder: &mut AdtBuilder<'vir>,
) -> Result<TyPureRaw<'vir>, EncodeFullError<'vir, TyPureEnc>> {
    let ty = data.metadata.decompose(task_key.params);
    let metadata = deps.require_ref::<TyUsePureEnc>(ty)?.snapshot.downcast_ty();
    // In contrast to the metadata, we do not model null pointers using an empty
    // tuple. Raw pointers could point to empty tuples and dereferencing such
    // a raw pointer is different from dereferencing a null pointer.
    let optional = deps.require_dep::<OptionalEnc>(())?;

    let (field_snaps_to_snap, field_access) =
        builder.constructor("", (vir::TYPE_REF, metadata, optional.snapshot), None);

    // A function rather than a destructor: unlike a destructor applied to the
    // wrong variant, its precondition makes reading an absent value a
    // verification error at the call site.
    let self_type = builder.self_type();
    let self_decl = builder.vcx.mk_local_decl("self", self_type);
    let optional_field: vir::AdtDestructor<'vir, vir::CSnap, vir::CSnap> =
        field_access[2].downcast_ty();
    let optional_value = optional_field.call()(builder.vcx.mk_local_ex(self_decl));
    let value_access = builder.function::<vir::CSnap, vir::PSnap>(
        "value",
        self_type,
        vir::TYPE_PSNAP,
        (self_decl,),
        &[optional.is_some(optional_value)],
        &[],
        Some(optional.value_access.call()(optional_value)),
    );
    let is_some = builder.function::<vir::CSnap, vir::Bool>(
        "is_some",
        self_type,
        vir::TYPE_BOOL,
        (self_decl,),
        &[],
        &[],
        Some(optional.is_some(optional_value)),
    );

    Ok(TyPureRawData {
        prim_to_snap: field_snaps_to_snap,
        address_access: field_access[0].downcast_ty(),
        metadata_access: field_access[1].downcast_ty(),
        value_none: optional.none,
        value_some: optional.some,
        value_access,
        is_some,
    })
}

pub(crate) fn ty_impure<'vir>(
    task_key: &TyData<'vir, (RustTyDatas, PureTyDatas)>,
    data: &(&RustRaw<'vir>, &TyPureRaw<'vir>),
    deps: &mut TaskEncoderDependencies<'vir, TyImpureEnc>,
    builder: &mut PredicateBuilder<'vir>,
) -> Result<TyImpureRaw<'vir>, EncodeFullError<'vir, TyImpureEnc>> {
    let metadata_type = data.0.metadata.decompose(task_key.0.params);
    deps.require_dep::<TyUseImpureEnc>(metadata_type)?;
    let optional = deps.require_dep::<OptionalEnc>(())?;

    let ref_self_decl = builder.ref_self_decl();
    let ref_self = builder.vcx.mk_local_ex(ref_self_decl);
    let vcx = builder.vcx;

    let metadata_snap = deps
        .require_ref::<TyUsePureEnc>(metadata_type)?
        .snapshot
        .downcast_ty();

    // fields: the address, the pointer metadata and the optional value, which
    // is all a raw pointer's snapshot holds. The optional value must be stored:
    // the snap function has to return what was assigned, otherwise assigning a
    // pointer whose value is present assumes `some == none`.
    let addr_field = builder.field("addr", vir::TYPE_REF);
    let metadata_field = builder.field("metadata", metadata_snap);
    let value_field = builder.field("value", optional.snapshot);

    let metadata_ty = deps.require_dep::<TyExprEnc>(metadata_type)?;
    let typ = deps.require_dep::<ParamTypEnc>(())?.typ;
    let metadata_typ = |metadata| vcx.mk_eq_expr(typ(metadata), metadata_ty);

    // main predicate
    builder.mk_predicate(
        "",
        Some(vcx.mk_conj(&[
            vir::expr! { acc((ref_self).[addr_field]) },
            vir::expr! { acc((ref_self).[metadata_field]) },
            vir::expr! { acc((ref_self).[value_field]) },
            metadata_typ(vir::expr! { [metadata_field](ref_self) }),
        ])),
    );

    // Ref-to-snap
    builder.mk_snap_function(
        Some(data.1.prim_to_snap.call()(
            vir::expr! { [addr_field](ref_self) },
            vir::expr! { [metadata_field](ref_self) },
            vir::expr! { [value_field](ref_self) },
        )),
        &[metadata_typ(data.1.metadata_access.call()(
            vcx.mk_result(builder.csnap_type()),
        ))],
    );

    Ok(TyImpureRawData {})
}
