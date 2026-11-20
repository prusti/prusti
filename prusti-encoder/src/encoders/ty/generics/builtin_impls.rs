use prusti_rustc_interface::{
    data_structures::fx::FxIndexMap, hir::attrs::LangItem, middle::ty, span::DUMMY_SP,
};
use task_encoder::{EncodeFullError, EncodeFullResult, TaskEncoder, TaskEncoderDependencies};
use vir::{CastType, FunctionIdn, ViperIdent, vir_format_identifier};

use crate::encoders::ty::{
    RustParamData, RustTy, RustTyDecomposition, RustTySizedness, TySpecifics,
    generics::{GParams, GenericParams, GenericParamsEnc, TyExprEnc, r#trait::TraitEnc},
    lifted::TyConstructorEnc,
};

/// Encodes the compiler's builtin impls of `Sized`, `MetaSized` and
/// `Pointee`. These traits have no impls in the source, so this encoder
/// states per type constructor what `TraitImplConditionEnc` and
/// `TraitImplItemEnc` state for an ordinary impl (that the trait is
/// implemented, and its associated types), each as an axiom triggered on the
/// application at the constructor. The axioms of each trait
/// are emitted together, in the domain `trait_<Trait>_builtin`.
///
/// A sized constructor implements `Sized` and `MetaSized`, conditional on its
/// tail if it has one; `str`, slices and trait objects implement only
/// `MetaSized`, and extern types neither (`PointeeSized` holds of every type,
/// and bounds on it are skipped, see `TraitImplEnc::context_bounds`):
///
/// ```text
/// axiom { Sized_impl(s_Char_type()) }
/// axiom { forall T :: {Sized_impl(s_W_type(T))} Sized_impl(T) ==> Sized_impl(s_W_type(T)) }
/// ```
///
/// A constructor that does not implement a trait (e.g. `str` for `Sized`)
/// has no axiom for it, which leaves it open rather than false.
///
/// The `Pointee` impl's associated type `Metadata` is the pointer metadata
/// type as rustc computes it (`Ty::pointee_metadata_ty_or_projection`): `()`
/// for a constructor that is sized unconditionally or an extern type, `usize`
/// for `str` and slices, `DynMetadata<Self>` for a trait object, and otherwise
/// the metadata type of the constructor's tail. A type parameter has no
/// constructor; for it, the fact is that a sized type has the metadata type
/// `()`, as rustc's fallback impl does:
///
/// ```text
/// axiom { forall T :: {Metadata(s_Slice_type(T))} Metadata(s_Slice_type(T)) == s_UInt_usize_type() }
/// axiom { forall T :: {Metadata(s_W_type(T))} Metadata(s_W_type(T)) == Metadata(T) }
/// axiom { forall T :: {Metadata(T)} Sized_impl(T) ==> Metadata(T) == s_0_Tuple_type() }
/// ```
///
/// These are needed where a metadata type stays a projection, e.g.
/// `<T as Pointee>::Metadata` in a body generic over `T: ?Sized`; they
/// determine it at each instantiation of `T`.
pub struct BuiltinImplEnc;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum BuiltinImplTask<'vir> {
    /// The `Sized` and `MetaSized` impls of a type constructor.
    Sizedness(RustTy<'vir>),
    /// The `Pointee` impl of a type constructor.
    Pointee(RustTy<'vir>),
    /// The `Pointee` impl of the sized type parameters.
    PointeeSized,
}

impl TaskEncoder for BuiltinImplEnc {
    task_encoder::encoder_cache!(BuiltinImplEnc);
    const ENCODER_NAME: &'static str = "builtin impl encoder";

    type TaskDescription<'vir> = BuiltinImplTask<'vir>;
    /// Each axiom with the name of the trait it is about.
    type OutputFullLocal<'vir> = &'vir [(ViperIdent<'vir>, vir::DomainAxiom<'vir>)];

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;
        let axioms = match *task_key {
            BuiltinImplTask::Sizedness(constructor) => Self::sizedness(deps, constructor)?,
            BuiltinImplTask::Pointee(constructor) => vec![Self::pointee(deps, constructor)?],
            BuiltinImplTask::PointeeSized => vec![Self::pointee_sized(deps)?],
        };
        vir::with_vcx(|vcx| Ok((vcx.alloc_slice(&axioms), ())))
    }

    /// Emits the axioms of each trait as one domain.
    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        let mut traits = FxIndexMap::<_, Vec<_>>::default();
        for axioms in Self::all_outputs_local(program) {
            for &(trait_name, axiom) in axioms {
                traits.entry(trait_name).or_default().push(axiom);
            }
        }
        for (trait_name, axioms) in traits {
            vir::with_vcx(|vcx| {
                program.add_domain(vcx.mk_domain(
                    vir_format_identifier!(vcx, "trait_{trait_name}_builtin"),
                    &[],
                    vcx.alloc_slice(&axioms),
                    &[],
                    None,
                ))
            });
        }
    }
}

impl BuiltinImplEnc {
    /// Encodes the builtin impls of a type constructor: its sizedness, and,
    /// once the `Pointee` trait is requested, its pointee metadata type. The
    /// latter is stated with that trait's `Metadata` function, so a program
    /// that never uses the trait does not get it. Called for every constructor
    /// `TyConstructorEnc` encodes.
    pub fn require(constructor: RustTy<'_>) {
        if matches!(constructor.sizedness, RustTySizedness::None) {
            return;
        }
        // TODO: request the facts with the span of the type's definition (or
        // of the field whose type is the tail), so that a failure is not
        // reported without a location. `RustTy` carries neither.
        let _ = Self::encode(BuiltinImplTask::Sizedness(constructor), false, DUMMY_SP);
        let pointee = vir::with_vcx(|vcx| {
            vcx.tcx()
                .require_lang_item(LangItem::PointeeTrait, DUMMY_SP)
        });
        TraitEnc::on_task_requested(pointee, move || {
            let _ = Self::encode(BuiltinImplTask::PointeeSized, false, DUMMY_SP);
            let _ = Self::encode(BuiltinImplTask::Pointee(constructor), false, DUMMY_SP);
        });
    }

    fn sizedness<'vir>(
        deps: &mut TaskEncoderDependencies<'vir, Self>,
        constructor: RustTy<'vir>,
    ) -> Result<Vec<(ViperIdent<'vir>, vir::DomainAxiom<'vir>)>, EncodeFullError<'vir, Self>> {
        let facts = ConstructorFacts::new(deps, constructor)?;
        vir::with_vcx(|vcx| {
            // For each trait, `None` if the constructor does not implement
            // it, otherwise the type it is conditional on (if any).
            let (sized, meta_sized) = match constructor.sizedness {
                RustTySizedness::None => unreachable!(),
                RustTySizedness::Sized {
                    sized_if,
                    meta_sized_if,
                } => (Some(sized_if), Some(meta_sized_if)),
                RustTySizedness::MetaSized => (None, Some(None)),
                RustTySizedness::PointeeSized => (None, None),
            };
            let traits = [
                ("Sized", LangItem::Sized, sized),
                ("MetaSized", LangItem::MetaSized, meta_sized),
            ];

            let mut axioms = Vec::new();
            for (fact, lang_item, condition) in traits {
                let Some(condition) = condition else {
                    continue;
                };
                let trait_did = vcx.tcx().require_lang_item(lang_item, DUMMY_SP);
                let trait_ = deps.require_ref::<TraitEnc>(trait_did)?;
                let implemented = (trait_.impl_fun)(&[facts.ty], &[]);
                let body = match condition {
                    None => implemented,
                    Some(tail) => {
                        let tail = RustTyDecomposition::from_ty(tail, constructor.params);
                        let tail_implemented =
                            (trait_.impl_fun)(&[facts.params.ty_expr(deps, tail)?], &[]);
                        vir::expr! { (tail_implemented) ==> (implemented) }
                    }
                };
                let axiom = facts.axiom(vcx, fact, vcx.mk_trigger(&[implemented]), body);
                axioms.push((trait_.trait_name, axiom));
            }
            Ok(axioms)
        })
    }

    fn pointee<'vir>(
        deps: &mut TaskEncoderDependencies<'vir, Self>,
        constructor: RustTy<'vir>,
    ) -> Result<(ViperIdent<'vir>, vir::DomainAxiom<'vir>), EncodeFullError<'vir, Self>> {
        let facts = ConstructorFacts::new(deps, constructor)?;
        let (trait_name, metadata_fun) = pointee_metadata(deps)?;
        vir::with_vcx(|vcx| {
            // The metadata type over the constructor's parameters, `None`
            // for a trait object.
            let metadata = match constructor.sizedness {
                RustTySizedness::None => unreachable!(),
                // `pointee_metadata_ty_or_projection` takes a pointer to the
                // pointee whose metadata type it computes.
                RustTySizedness::Sized { sized_if, .. } => {
                    Some(sized_if.map_or(vcx.tcx().types.unit, |tail| {
                        ty::Ty::new_imm_ptr(vcx.tcx(), tail)
                            .pointee_metadata_ty_or_projection(vcx.tcx())
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
                RustTySizedness::MetaSized => Some(vcx.tcx().types.usize),
                // Extern types.
                RustTySizedness::PointeeSized => Some(vcx.tcx().types.unit),
            };
            let metadata = match metadata {
                Some(metadata) => facts.params.ty_expr(
                    deps,
                    RustTyDecomposition::from_ty(metadata, constructor.params),
                )?,
                // All trait objects are encoded by the one parameterless `Dyn`
                // constructor, so there is no rustc type `dyn Trait` to
                // instantiate `DynMetadata` with; `DynMetadata` is applied to
                // the `Dyn` type directly.
                None => {
                    let did = vcx.tcx().require_lang_item(LangItem::DynMetadata, DUMMY_SP);
                    let dyn_metadata = vcx.tcx().type_of(did).instantiate_identity();
                    let dyn_metadata = deps.require_ref::<TyConstructorEnc>(
                        RustTyDecomposition::from_ty(dyn_metadata.skip_normalization(), did).ty,
                    )?;
                    dyn_metadata.ty_constructor.call()(vcx.alloc_slice(&[facts.ty]), &[])
                }
            };

            let metadata_ty = metadata_fun(&[facts.ty], &[]);
            let axiom = facts.axiom(
                vcx,
                "Metadata",
                vcx.mk_trigger(&[metadata_ty]),
                vcx.mk_eq_expr(metadata_ty, metadata),
            );
            Ok((trait_name, axiom))
        })
    }

    fn pointee_sized<'vir>(
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> Result<(ViperIdent<'vir>, vir::DomainAxiom<'vir>), EncodeFullError<'vir, Self>> {
        let unit = vir::with_vcx(|vcx| {
            RustTyDecomposition::from_ty(vcx.tcx().types.unit, GParams::empty())
        });
        let unit_ty = deps.require_dep::<TyExprEnc>(unit)?;
        let (trait_name, metadata) = pointee_metadata(deps)?;
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
            Ok((
                trait_name,
                vcx.mk_domain_axiom(ViperIdent::new("pointee_metadata_sized"), axiom),
            ))
        })
    }
}

/// The function of an associated type: the type for the trait's arguments.
type AssocTypeFun<'vir> = FunctionIdn<'vir, (vir::ManyTyVal, vir::ManyCSnap), vir::TyVal>;

/// The name of the `Pointee` trait and its `Metadata` function.
pub(crate) fn pointee_metadata<'vir, E: TaskEncoder + 'vir + ?Sized>(
    deps: &mut TaskEncoderDependencies<'vir, E>,
) -> Result<(ViperIdent<'vir>, AssocTypeFun<'vir>), EncodeFullError<'vir, E>> {
    vir::with_vcx(|vcx| {
        let pointee = vcx
            .tcx()
            .require_lang_item(LangItem::PointeeTrait, DUMMY_SP);
        let metadata = vcx.tcx().require_lang_item(LangItem::Metadata, DUMMY_SP);
        let pointee = deps.require_ref::<TraitEnc>(pointee)?;
        Ok((pointee.trait_name, pointee.assoc_types[&metadata]))
    })
}

/// A type constructor applied to its own parameters, over which the facts of
/// its builtin impls are quantified.
struct ConstructorFacts<'vir> {
    constructor: RustTy<'vir>,
    params: GenericParams<'vir>,
    /// The constructor applied to `params`.
    ty: vir::ExprTyVal<'vir>,
}

impl<'vir> ConstructorFacts<'vir> {
    fn new(
        deps: &mut TaskEncoderDependencies<'vir, BuiltinImplEnc>,
        constructor: RustTy<'vir>,
    ) -> Result<Self, EncodeFullError<'vir, BuiltinImplEnc>> {
        let params = deps.require_dep::<GenericParamsEnc>(constructor.params)?;
        let ty_constructor = deps
            .require_ref::<TyConstructorEnc>(constructor)?
            .ty_constructor;
        let ty = ty_constructor.call()(params.ty_exprs(), params.const_exprs());
        Ok(Self {
            constructor,
            params,
            ty,
        })
    }

    /// The axiom `s_<constructor>_<fact>`: `body` for all parameters of the
    /// constructor, triggered on `trigger`.
    fn axiom(
        &self,
        vcx: &'vir vir::VirCtxt<'vir>,
        fact: &str,
        trigger: vir::Trigger<'vir>,
        body: vir::ExprBool<'vir>,
    ) -> vir::DomainAxiom<'vir> {
        let base_name = self.constructor.name();
        let decls = self.params.ty_decls().iter().map(|decl| decl.upcast_ty());
        let decls = decls.chain(
            self.params
                .const_decls()
                .iter()
                .map(|decl| decl.upcast_ty()),
        );
        vcx.mk_domain_axiom(
            vir_format_identifier!(vcx, "s_{base_name}_{fact}"),
            vcx.mk_forall_expr(
                vcx.alloc_slice(&decls.collect::<Vec<vir::LocalDeclDyn>>()),
                vcx.alloc_slice(&[trigger]),
                body,
            ),
        )
    }
}
