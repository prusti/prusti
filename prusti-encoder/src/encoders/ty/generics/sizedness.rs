use prusti_rustc_interface::{hir::attrs::LangItem, middle::ty, span::DUMMY_SP};
use task_encoder::{EncodeFullError, EncodeFullResult, TaskEncoder, TaskEncoderDependencies};
use vir::{CastType, FunctionIdn, vir_format_identifier};

use crate::encoders::ty::{
    RustParamData, RustTy, RustTyDecomposition, RustTySizedness, TySpecifics,
    generics::{GParams, GenericParamsEnc, TyExprEnc, r#trait::TraitEnc},
    lifted::TyConstructorEnc,
};

/// Encodes a type constructor's facts about the sizedness traits `Sized` and
/// `MetaSized` (`PointeeSized` holds of every type, and bounds on it are
/// skipped, see `TraitImplEnc::context_bounds`). These are implemented by
/// the compiler rather than by impls, so where an ordinary trait's
/// `impl_fun` is established by its impls' conditions, these are
/// established by the constructors, each with an axiom triggered on the
/// application at the constructor:
///
/// ```text
/// axiom { Sized_impl(s_Char_type()) }
/// axiom { forall T :: {Sized_impl(s_W_type(T))} Sized_impl(T) ==> Sized_impl(s_W_type(T)) }
/// ```
///
/// A constructor that does not implement a trait (e.g. `str` for `Sized`)
/// has no axiom for it, which leaves it open rather than false.
pub struct SizednessEnc;

impl TaskEncoder for SizednessEnc {
    task_encoder::encoder_cache!(SizednessEnc);
    const ENCODER_NAME: &'static str = "sizedness encoder";

    type TaskDescription<'vir> = RustTy<'vir>;
    type OutputFullLocal<'vir> = vir::Domain<'vir>;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;
        vir::with_vcx(|vcx| {
            let lang_items = vcx.tcx().lang_items();
            // For each trait, `None` if the constructor does not implement
            // it, otherwise the type it is conditional on (if any).
            let (sized, meta_sized) = match task_key.sizedness {
                RustTySizedness::None => unreachable!(),
                RustTySizedness::Sized {
                    sized_if,
                    meta_sized_if,
                } => (Some(sized_if), Some(meta_sized_if)),
                RustTySizedness::MetaSized => (None, Some(None)),
                RustTySizedness::PointeeSized => (None, None),
            };
            let facts = [
                ("Sized", lang_items.sized_trait(), sized),
                ("MetaSized", lang_items.meta_sized_trait(), meta_sized),
            ];

            let params = deps.require_dep::<GenericParamsEnc>(task_key.params)?;
            let constructor = deps.require_ref::<TyConstructorEnc>(*task_key)?;
            let ty = constructor.ty_constructor.call()(params.ty_exprs(), params.const_exprs());
            let base_name = task_key.name();

            let mut axioms = Vec::new();
            for (trait_name, trait_did, fact) in facts {
                let (Some(trait_did), Some(condition)) = (trait_did, fact) else {
                    continue;
                };
                let impl_fun = deps.require_ref::<TraitEnc>(trait_did)?.impl_fun;
                let implemented = impl_fun(&[ty], &[]);
                let body = match condition {
                    None => implemented,
                    Some(tail) => {
                        let tail = RustTyDecomposition::from_ty(tail, task_key.params);
                        let tail_implemented = impl_fun(&[params.ty_expr(deps, tail)?], &[]);
                        vir::expr! { (tail_implemented) ==> (implemented) }
                    }
                };
                let decls = params.ty_decls().iter().map(|decl| decl.upcast_ty());
                let decls = decls.chain(params.const_decls().iter().map(|decl| decl.upcast_ty()));
                axioms.push(vcx.mk_domain_axiom(
                    vir_format_identifier!(vcx, "s_{base_name}_{trait_name}"),
                    vcx.mk_forall_expr(
                        vcx.alloc_slice(&decls.collect::<Vec<vir::LocalDeclDyn>>()),
                        vcx.alloc_slice(&[vcx.mk_trigger(&[implemented])]),
                        body,
                    ),
                ));
            }

            let domain = vcx.mk_domain(
                vir_format_identifier!(vcx, "sizedness_{base_name}"),
                &[],
                vcx.alloc_slice(&axioms),
                &[],
                None,
            );
            Ok((domain, ()))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for domain in Self::all_outputs_local(program) {
            program.add_domain(domain);
        }
    }
}

impl SizednessEnc {
    /// Encodes the facts of a type constructor. Called for every constructor
    /// `TyConstructorEnc` encodes.
    pub fn require(constructor: RustTy<'_>) {
        if !matches!(constructor.sizedness, RustTySizedness::None) {
            // TODO: request the facts with the span of the type's definition
            // (or of the field whose type is the tail), so that a failure is
            // not reported without a location. `RustTy` carries neither.
            let _ = Self::encode(constructor, false, DUMMY_SP);
        }
    }
}

/// Encodes the compiler's builtin `Pointee` impl, i.e. the pointer metadata
/// type `Metadata(T)` of a type. Like the sizedness facts, it has no impl to
/// come from. For each type constructor (task `Some`), it states the
/// metadata type as rustc computes it (`Ty::pointee_metadata_ty_or_projection`),
/// triggered on `Metadata` applied to the constructor: `()` for a constructor
/// that is sized unconditionally or an extern type, `usize` for `str` and
/// slices, `DynMetadata<Self>` for a trait object, and otherwise the metadata
/// type of the tail that the constructor's sizedness is conditional on. A
/// type parameter has no constructor; for it (task `None`), the fact is that a
/// sized type has the metadata type `()`, as rustc's fallback impl does:
///
/// ```text
/// axiom { forall T :: {Metadata(s_Slice_type(T))} Metadata(s_Slice_type(T)) == s_UInt_usize_type() }
/// axiom { forall T :: {Metadata(s_W_type(T))} Metadata(s_W_type(T)) == Metadata(T) }
/// axiom { forall T :: {Metadata(T)} Sized_impl(T) ==> Metadata(T) == () }
/// ```
///
/// These are needed where a metadata type stays a projection, e.g.
/// `<T as Pointee>::Metadata` in a body generic over `T: ?Sized`, which is
/// then known at the instantiations of `T`.
pub struct PointeeMetadataEnc;

impl TaskEncoder for PointeeMetadataEnc {
    task_encoder::encoder_cache!(PointeeMetadataEnc);
    const ENCODER_NAME: &'static str = "pointee metadata encoder";

    type TaskDescription<'vir> = Option<RustTy<'vir>>;
    type OutputFullLocal<'vir> = vir::Domain<'vir>;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;
        let domain = match *task_key {
            None => Self::encode_sized(deps)?,
            Some(constructor) => Self::encode_constructor(deps, constructor)?,
        };
        Ok((domain, ()))
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for domain in Self::all_outputs_local(program) {
            program.add_domain(domain);
        }
    }
}

impl PointeeMetadataEnc {
    /// Encodes the metadata type of a type constructor, and the fact for type
    /// parameters, once the `Pointee` trait, whose `Metadata` function they
    /// are stated with, is requested, so that a program without that
    /// function does not get them. Called for every constructor
    /// `TyConstructorEnc` encodes.
    pub fn require(constructor: RustTy<'_>) {
        if matches!(constructor.sizedness, RustTySizedness::None) {
            return;
        }
        let pointee = vir::with_vcx(|vcx| {
            vcx.tcx()
                .require_lang_item(LangItem::PointeeTrait, DUMMY_SP)
        });
        TraitEnc::on_task_requested(pointee, move || {
            // TODO: request the facts with a span (see `SizednessEnc::require`).
            let _ = Self::encode(None, false, DUMMY_SP);
            let _ = Self::encode(Some(constructor), false, DUMMY_SP);
        });
    }

    fn metadata_fun<'vir>(
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> Result<
        FunctionIdn<'vir, (vir::ManyTyVal, vir::ManyCSnap), vir::TyVal>,
        EncodeFullError<'vir, Self>,
    > {
        vir::with_vcx(|vcx| {
            let pointee = vcx
                .tcx()
                .require_lang_item(LangItem::PointeeTrait, DUMMY_SP);
            let metadata = vcx.tcx().require_lang_item(LangItem::Metadata, DUMMY_SP);
            Ok(deps.require_ref::<TraitEnc>(pointee)?.assoc_types[&metadata])
        })
    }

    fn encode_sized<'vir>(
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> Result<vir::Domain<'vir>, EncodeFullError<'vir, Self>> {
        let unit = vir::with_vcx(|vcx| {
            RustTyDecomposition::from_ty(vcx.tcx().types.unit, GParams::empty())
        });
        let unit_ty = deps.require_dep::<TyExprEnc>(unit)?;
        let metadata = Self::metadata_fun(deps)?;
        let sized = vir::with_vcx(|vcx| {
            let sized = vcx.tcx().require_lang_item(LangItem::Sized, DUMMY_SP);
            Ok(deps.require_ref::<TraitEnc>(sized)?.impl_fun)
        })?;
        vir::with_vcx(|vcx| {
            let ty_decl = vcx.mk_local_decl("ty", vir::TYPE_TYVAL);
            let ty = vcx.mk_local_ex(ty_decl);
            let metadata = metadata(&[ty], &[]);
            let sized = sized(&[ty], &[]);
            let axiom = vir::expr! {
                forall [ty_decl] :: { [metadata] }
                    (sized) ==> ((metadata) == (unit_ty))
            };
            Ok(vcx.mk_domain(
                vir::ViperIdent::new("PointeeMetadata"),
                &[],
                vcx.alloc_slice(&[
                    vcx.mk_domain_axiom(vir::ViperIdent::new("pointee_metadata_sized"), axiom)
                ]),
                &[],
                None,
            ))
        })
    }

    fn encode_constructor<'vir>(
        deps: &mut TaskEncoderDependencies<'vir, Self>,
        constructor: RustTy<'vir>,
    ) -> Result<vir::Domain<'vir>, EncodeFullError<'vir, Self>> {
        let params = deps.require_dep::<GenericParamsEnc>(constructor.params)?;
        let ty_constructor = deps
            .require_ref::<TyConstructorEnc>(constructor)?
            .ty_constructor;
        let ty = ty_constructor(params.ty_exprs(), params.const_exprs());
        let metadata_fun = Self::metadata_fun(deps)?;
        vir::with_vcx(|vcx| {
            let tcx = vcx.tcx();
            // The metadata type over the constructor's parameters, `None`
            // for a trait object.
            let metadata = match constructor.sizedness {
                RustTySizedness::None => unreachable!(),
                RustTySizedness::Sized { sized_if, .. } => {
                    Some(sized_if.map_or(tcx.types.unit, |tail| {
                        ty::Ty::new_imm_ptr(tcx, tail).pointee_metadata_ty_or_projection(tcx)
                    }))
                }
                RustTySizedness::MetaSized
                    if matches!(
                        constructor.specifics,
                        TySpecifics::Param(RustParamData::Dyn)
                    ) =>
                {
                    None
                }
                // `str` and slices.
                RustTySizedness::MetaSized => Some(tcx.types.usize),
                // Extern types.
                RustTySizedness::PointeeSized => Some(tcx.types.unit),
            };
            let metadata = match metadata {
                Some(metadata) => params.ty_expr(
                    deps,
                    RustTyDecomposition::from_ty(metadata, constructor.params),
                )?,
                // `DynMetadata<Self>` with the `Dyn` constructor as `Self`:
                // all trait objects share that parameterless constructor, so
                // there is no rustc type to instantiate `DynMetadata` with.
                None => {
                    let did = tcx.require_lang_item(LangItem::DynMetadata, DUMMY_SP);
                    let dyn_metadata = tcx.type_of(did).instantiate_identity();
                    let dyn_metadata = deps.require_ref::<TyConstructorEnc>(
                        RustTyDecomposition::from_ty(dyn_metadata.skip_normalization(), did).ty,
                    )?;
                    dyn_metadata.ty_constructor.call()(vcx.alloc_slice(&[ty]), &[])
                }
            };

            let base_name = constructor.name();
            let metadata_ty = metadata_fun(&[ty], &[]);
            let decls = params.ty_decls().iter().map(|decl| decl.upcast_ty());
            let decls = decls.chain(params.const_decls().iter().map(|decl| decl.upcast_ty()));
            let axiom = vcx.mk_domain_axiom(
                vir_format_identifier!(vcx, "s_{base_name}_Metadata"),
                vcx.mk_forall_expr(
                    vcx.alloc_slice(&decls.collect::<Vec<vir::LocalDeclDyn>>()),
                    vcx.alloc_slice(&[vcx.mk_trigger(&[metadata_ty])]),
                    vcx.mk_eq_expr(metadata_ty, metadata),
                ),
            );
            Ok(vcx.mk_domain(
                vir_format_identifier!(vcx, "pointee_metadata_{base_name}"),
                &[],
                vcx.alloc_slice(&[axiom]),
                &[],
                None,
            ))
        })
    }
}
