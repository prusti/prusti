use crate::encoders::{
    TyUseImpureEnc,
    ty::{
        RustTyDatas,
        data::{StructData, TyData},
        impure::{
            ImpureTyDatas, PredicateBuilder, TyImpureEnc, TyImpureFieldData, TyImpureStructData,
        },
        pure::{
            AdtBuilder, PureTyDatas, TyPureEnc, TyPureFieldData, TyPureFieldRef, TyPureStructData,
        },
        rust_ty::{RustFieldAddress, RustTySpecial},
        use_pure::TyUsePureEnc,
    },
};
use task_encoder::{EncodeFullError, TaskEncoderDependencies};
use vir::{CastType, HasType, PredicateIdn};

pub(crate) fn ty_pure<'vir>(
    task_key: &TyData<'vir, RustTyDatas>,
    data: &StructData<'vir, RustTyDatas>,
    deps: &mut TaskEncoderDependencies<'vir, TyPureEnc>,
    builder: &mut AdtBuilder<'vir>,
) -> Result<StructData<'vir, PureTyDatas>, EncodeFullError<'vir, TyPureEnc>> {
    ty_pure_variant("", None, task_key, data, deps, builder)
}

pub(super) fn ty_pure_variant<'vir>(
    prefix: &str,
    discr: Option<vir::ExprCSnap<'vir>>,
    task_key: &TyData<'vir, RustTyDatas>,
    data: &StructData<'vir, RustTyDatas>,
    deps: &mut TaskEncoderDependencies<'vir, TyPureEnc>,
    builder: &mut AdtBuilder<'vir>,
) -> Result<StructData<'vir, PureTyDatas>, EncodeFullError<'vir, TyPureEnc>> {
    let field_tys = data
        .fields
        .iter()
        .map(|f| {
            let ty = f.decompose(task_key.params);
            Ok(deps.require_ref::<TyUsePureEnc>(ty)?.snapshot)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let field_tys = builder.vcx.alloc_slice(&field_tys);
    let (field_snaps_to_snap, des) = builder.constructor(prefix, field_tys, discr);

    // For a `Box`, its extras and the address of its (dynamic) value field.
    let box_data = if let RustTySpecial::Box = task_key.special {
        Some(super::r#box::mk_pure_box_data(
            task_key, data, deps, builder,
        )?)
    } else {
        None
    };
    let held_address = if let RustTySpecial::Holds(_, ptr) = task_key.special {
        Some(super::r#box::mk_pure_held_address(
            task_key, data, ptr, deps, builder,
        )?)
    } else {
        None
    };

    let ref_self_decl = builder.vcx.mk_local_decl("self", vir::TYPE_REF);
    let ref_self = builder.vcx.mk_local_ex(ref_self_decl);
    let params = builder.params.clone();

    assert_eq!(des.len(), data.fields.len());
    let des = des
        .iter()
        .zip(&data.fields)
        .enumerate()
        .map(|(idx, (read, field))| {
            let ref_to_field_ref = match field.address {
                RustFieldAddress::Constant => {
                    let args = (ref_self_decl.ty(), params.ty_args(), params.const_args());
                    let params = (ref_self_decl, params.ty_decls(), params.const_decls());
                    TyPureFieldRef::Constant(builder.function(
                        &format!("{prefix}field_{idx}"),
                        args,
                        vir::TYPE_REF,
                        params,
                        &[],
                        &[vir::expr! { ((ref_self) == (null)) == ((result: Ref) == (null)) }],
                        None,
                    ))
                }
                RustFieldAddress::Dynamic => TyPureFieldRef::Dynamic(
                    box_data.map_or_else(|| held_address.unwrap(), |(_, address)| address),
                ),
            };
            TyPureFieldData {
                read: read.downcast_ty(),
                ref_to_field_ref,
            }
        })
        .collect::<Vec<_>>();
    Ok(StructData::new(
        TyPureStructData {
            field_snaps_to_snap,
            box_data: box_data.map(|(box_data, _)| box_data),
        },
        des,
    ))
}

pub(crate) fn ty_impure<'vir>(
    task_key: &TyData<'vir, (RustTyDatas, PureTyDatas)>,
    data: &StructData<'vir, (RustTyDatas, PureTyDatas)>,
    deps: &mut TaskEncoderDependencies<'vir, TyImpureEnc>,
    builder: &mut PredicateBuilder<'vir>,
) -> Result<StructData<'vir, ImpureTyDatas>, EncodeFullError<'vir, TyImpureEnc>> {
    if let RustTySpecial::UnsafeCell = task_key.0.special {
        return unsafe_cell_impure(data, deps, builder);
    }
    let (data, _, snap_expr) = ty_impure_variant("", task_key, data, deps, builder, None)?;

    // Ref-to-snap
    builder.mk_snap_function(Some(snap_expr), &[]);
    Ok(data)
}

/// An `UnsafeCell<T>`'s predicate is only its place component (abstract): the
/// contents are an interior location. Its shallow snapshot has `im_unset(T)`
/// in the slot.
fn unsafe_cell_impure<'vir>(
    data: &StructData<'vir, (RustTyDatas, PureTyDatas)>,
    deps: &mut TaskEncoderDependencies<'vir, TyImpureEnc>,
    builder: &mut PredicateBuilder<'vir>,
) -> Result<StructData<'vir, ImpureTyDatas>, EncodeFullError<'vir, TyImpureEnc>> {
    let unset = deps.require_dep::<crate::encoders::ty::im::ImUnsetEnc>(())?;
    let field_accessors = data
        .fields
        .iter()
        .map(|field| TyImpureFieldData {
            ref_to_field_ref: match field.1.ref_to_field_ref {
                TyPureFieldRef::Constant(ref_to_field_ref) => ref_to_field_ref,
                TyPureFieldRef::Dynamic(_) => unreachable!("an `UnsafeCell` has no dynamic field"),
            },
        })
        .collect::<Vec<_>>();
    builder.mk_predicate("", None);
    let vcx = builder.vcx;
    let shallow =
        data.1.field_snaps_to_snap.call()(
            &[unset.call()(builder.params.ty_exprs()[0]).upcast_ty()],
        );
    let post = vcx.mk_eq_expr(vcx.mk_result(builder.csnap_type()), shallow);
    let wildcard = vcx.mk_wildcard();
    builder.mk_snap_function_amount(None, &[post], Some(wildcard));
    // The contents are not part of the predicate: the `Uninit` token stands
    // for the cell alone, and has no field tokens to repack into.
    let ref_self_decl = builder.ref_self_decl();
    let ref_self = vcx.mk_local_ex(ref_self_decl);
    let params = builder.params.clone();
    let self_uninit = builder.uninit_pred_expr(ref_self);
    let uninit_args = (ref_self_decl.ty(), params.ty_args(), params.const_args());
    let [method_uninit_expand, method_uninit_collapse] =
        ["$uninit_expand", "$uninit_collapse"].map(|name| {
            builder.inner.method(
                name,
                uninit_args,
                &[],
                (ref_self_decl, params.ty_decls(), params.const_decls()),
                &[self_uninit],
                &[self_uninit],
            )
        });
    Ok(StructData::new(
        TyImpureStructData {
            box_data: None,
            method_uninit_expand,
            method_uninit_collapse,
        },
        field_accessors,
    ))
}

pub(super) type ImpureVariant<'vir> = (
    StructData<'vir, ImpureTyDatas>,
    PredicateIdn<'vir, (vir::Ref, vir::ManyTyVal, vir::ManyCSnap)>,
    vir::ExprCSnap<'vir>,
);

pub(crate) fn ty_impure_variant<'vir>(
    prefix: &str,
    task_key: &TyData<'vir, (RustTyDatas, PureTyDatas)>,
    data: &StructData<'vir, (RustTyDatas, PureTyDatas)>,
    deps: &mut TaskEncoderDependencies<'vir, TyImpureEnc>,
    builder: &mut PredicateBuilder<'vir>,
    // Held alongside the fields' `Uninit` tokens when unpacked (an enum's
    // discriminant predicate).
    extra_unpacked_resource: Option<vir::ExprBool<'vir>>,
) -> Result<ImpureVariant<'vir>, EncodeFullError<'vir, TyImpureEnc>> {
    let fields = data
        .fields
        .iter()
        .map(|f| {
            let ty = f.0.decompose(task_key.0.params);
            deps.require_dep::<TyUseImpureEnc>(ty)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let ref_self_decl = builder.ref_self_decl();
    let ref_self = builder.vcx.mk_local_ex(ref_self_decl);

    // For a `Box`, its extras and the (heap-dependent) address of its dynamic
    // value field. Since the value field comes after the `Unique` field, its
    // predicate conjunct below is framed.
    let box_data = if let RustTySpecial::Box = task_key.0.special {
        Some(super::r#box::mk_impure_box_data(data, &fields, builder))
    } else {
        None
    };
    // For a `#[ptr_holds]` struct, the (heap-dependent) address of the held
    // pointee and the amount held.
    let held = if let RustTySpecial::Holds(did, ptr) = task_key.0.special {
        let address = super::r#box::mk_impure_held_address(data, &fields, ptr, builder);
        let amount = crate::encoders::ty::im::holds_amount(deps, did, task_key.0.params)?;
        Some((address, amount))
    } else {
        None
    };

    // Ref-to-Ref function for every field
    let field_accessors = data
        .fields
        .iter()
        .map(|field| TyImpureFieldData {
            ref_to_field_ref: match field.1.ref_to_field_ref {
                TyPureFieldRef::Constant(ref_to_field_ref) => ref_to_field_ref,
                TyPureFieldRef::Dynamic(_) => {
                    box_data.map_or_else(|| held.unwrap().0, |(_, address)| address)
                }
            },
        })
        .collect::<Vec<_>>();

    // main variant predicate
    let mut pred_name = String::new();
    if !prefix.is_empty() {
        pred_name = format!("{prefix}owned");
    }
    let pred_expr = builder.vcx.mk_conj(
        &fields
            .iter()
            .zip(&field_accessors)
            .zip(&data.fields)
            .map(
                |((field, TyImpureFieldData { ref_to_field_ref }), field_data)| {
                    let amount = match field_data.0.address {
                        RustFieldAddress::Dynamic => held.map(|(_, amount)| amount),
                        RustFieldAddress::Constant => None,
                    };
                    field.ref_to_pred(
                        builder.vcx,
                        ref_to_field_ref(
                            ref_self,
                            builder.params.ty_exprs(),
                            builder.params.const_exprs(),
                        ),
                        amount,
                    )
                },
            )
            .collect::<Vec<_>>(),
    );
    let pred_owned = builder.mk_predicate(&pred_name, Some(pred_expr));

    // Ref-to-snap
    let mut snap_args: Vec<&'vir vir::ExprGenData<'vir, (), !, vir::Snap>> = fields
        .iter()
        .zip(&field_accessors)
        .map(|(field, TyImpureFieldData { ref_to_field_ref })| {
            field.ref_to_snap(ref_to_field_ref(
                ref_self,
                builder.params.ty_exprs(),
                builder.params.const_exprs(),
            ))
        })
        .collect::<Vec<_>>();
    // A held pointee is read only where its amount is positive (Silicon
    // cannot always tell inside the snapshot function).
    if let Some((_, amount)) = held {
        let vcx = builder.vcx;
        let held_ty = data.fields.last().unwrap().0.decompose(task_key.0.params);
        let arbitrary = deps
            .require_ref::<TyUsePureEnc>(held_ty)?
            .arbitrary_to_snap();
        let last = snap_args.last_mut().unwrap();
        *last = vcx.mk_ternary_expr(
            vcx.mk_bin_op_expr(vir::BinOpKind::CmpGt, amount, vcx.mk_no_perm())
                .downcast_ty(),
            *last,
            arbitrary,
        );
    }
    let variant_snap_expr = data.1.field_snaps_to_snap.call()(snap_args.as_slice());

    // Uninit token repacking methods. A `Box` is unpacked only through its
    // value, so its token also stands for the other fields' predicates.
    let params = builder.params.clone();
    let self_uninit = builder.uninit_pred_expr(ref_self);
    let value_field = if box_data.is_some() {
        fields.len() - 1
    } else {
        0
    };
    // A `#[ptr_holds]` struct's held pointee and a shared slot are no memory
    // of the struct: they have no tokens.
    let mut field_uninits = fields
        .iter()
        .zip(&field_accessors)
        .enumerate()
        .filter(|(idx, _)| {
            let field_data = &data.fields[*idx];
            field_data.0.address != RustFieldAddress::Slot
                && !(held.is_some()
                    && matches!(field_data.1.ref_to_field_ref, TyPureFieldRef::Dynamic(_)))
        })
        .map(|(idx, (field, TyImpureFieldData { ref_to_field_ref }))| {
            let field_ref = ref_to_field_ref(ref_self, params.ty_exprs(), params.const_exprs());
            if idx < value_field {
                field.ref_to_pred(builder.vcx, field_ref, None)
            } else {
                field.uninit_pred(builder.vcx, field_ref)
            }
        })
        .collect::<Vec<_>>();
    field_uninits.extend(extra_unpacked_resource);
    let uninit_args = (ref_self_decl.ty(), params.ty_args(), params.const_args());
    let method_uninit_expand = builder.inner.method(
        &format!("{prefix}$uninit_expand"),
        uninit_args,
        &[],
        (ref_self_decl, params.ty_decls(), params.const_decls()),
        &[self_uninit],
        &field_uninits,
    );
    let method_uninit_collapse = builder.inner.method(
        &format!("{prefix}$uninit_collapse"),
        uninit_args,
        &[],
        (ref_self_decl, params.ty_decls(), params.const_decls()),
        &field_uninits,
        &[self_uninit],
    );

    Ok((
        StructData::new(
            TyImpureStructData {
                box_data: box_data.map(|(box_data, _)| box_data),
                method_uninit_expand,
                method_uninit_collapse,
            },
            field_accessors,
        ),
        pred_owned,
        variant_snap_expr,
    ))
}
