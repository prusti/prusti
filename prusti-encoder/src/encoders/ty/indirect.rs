use itertools::Itertools;
use pcg::borrow_pcg::{
    GeneralizedLifetime,
    region_projection::{
        ExtractRegionsCtxt, Generalized, LifetimeProjection, LifetimeProjectionIdx, PcgRegion,
    },
};
use prusti_rustc_interface::{
    index::IndexVec,
    middle::ty::{self, TyCtxt},
};
use task_encoder::{EncodeFullResult, TaskEncoder, TaskEncoderDependencies};
use vir::{CastType, Reify};

use crate::encoders::{TyUseImpureEnc, ty::RustTyDecomposition};

use super::{
    data::{StructData, TySpecifics},
    rust_ty::RustTyDatas,
    use_pure::{TyUsePureEnc, TyUsePureField, UsePureTyDatas},
};

#[derive(Copy, Clone)]
pub struct PrustiPcgCtxt;

impl<'tcx> ExtractRegionsCtxt<'tcx, RustTyDecomposition<'tcx>, GeneralizedLifetime<'tcx>>
    for PrustiPcgCtxt
{
    fn extract_regions(
        self,
        data: RustTyDecomposition<'tcx>,
    ) -> IndexVec<LifetimeProjectionIdx<Generalized>, GeneralizedLifetime<'tcx>> {
        data.args
            .args()
            .iter()
            .flat_map(|arg| match arg.as_type() {
                Some(ty) => self.regions_vec(ty),
                None => arg
                    .as_region()
                    .map(|r| GeneralizedLifetime::Region(r.into()))
                    .into_iter()
                    .collect(),
            })
            .unique()
            .collect()
    }
}

pub struct IndirectPredicatesEnc;

type ExprInput<'vir> = vir::ExprSnap<'vir>;
type ExprOutput<'vir> = vir::ExprGenBool<'vir, ExprInput<'vir>, vir::ExprKind<'vir>>;
/// The snapshot of the place whose lifetime projection it is, and a label.
type UnchangedInput<'vir> = (ExprInput<'vir>, vir::CfgBlockLabelData<'vir>);
type UnchangedOutput<'vir> = vir::ExprGenBool<'vir, UnchangedInput<'vir>, vir::ExprKind<'vir>>;

/// The indirect predicates of a lifetime projection, as functions of the
/// snapshot of the place whose lifetime projection it is.
#[derive(Clone, Default)]
pub struct IndirectPredicatesEncOutputRef<'vir> {
    pub predicates: Vec<IndirectPredicate<'vir>>,
}

impl<'vir> IndirectPredicatesEncOutputRef<'vir> {
    pub fn applications(&self) -> impl Iterator<Item = ExprOutput<'vir>> + '_ {
        self.predicates
            .iter()
            .map(|predicate| predicate.application)
    }
}

/// An indirect predicate application, with what it says about the referents
/// it holds.
#[derive(Clone, Copy)]
pub struct IndirectPredicate<'vir> {
    pub application: ExprOutput<'vir>,
    /// That the referents the application holds have the values they had at
    /// the label, under the same case splits as the application. The holder's
    /// snapshot is expected to be a heap read, so the path from the holder to
    /// each referent is also read at the label.
    pub unchanged: UnchangedOutput<'vir>,
}

/// The variants of an enum value, given by their discriminants and the
/// indirect predicates of their fields with the fields' accessors.
type Variants<'vir> = Vec<(
    vir::ExprCSnap<'vir>,
    Vec<(&'vir TyUsePureField<'vir>, IndirectPredicate<'vir>)>,
)>;

/// The case split on the variant of the enum value `self_snap`, of the
/// conjunction of `field` over the fields of each variant. The first variant
/// holds when no other is selected.
fn variant_case_split<'vir, 'tcx>(
    vcx: &'vir vir::VirCtxt<'tcx>,
    snap_to_discr_snap: vir::FunctionIdn<'vir, vir::CSnap, vir::CSnap>,
    self_snap: vir::ExprCSnap<'vir>,
    variants: &Variants<'vir>,
    field: impl Fn(vir::ExprSnap<'vir>, IndirectPredicate<'vir>) -> vir::ExprBool<'vir>,
) -> vir::ExprBool<'vir> {
    let self_discr = snap_to_discr_snap.call()(self_snap);
    let variant_conjs: Vec<_> = variants
        .iter()
        .map(|(discr, fields)| {
            let preds: Vec<_> = fields
                .iter()
                .map(|&(acc, inner)| field(acc.read(self_snap), inner))
                .collect();
            (discr, vcx.mk_conj(&preds))
        })
        .collect();
    let (first, rest) = variant_conjs.split_first().unwrap();
    rest.iter().fold(first.1, |else_, (discr, conj)| {
        vcx.mk_ternary_expr(vir::expr! { ([self_discr]) == ([**discr]) }, *conj, else_)
    })
}

impl<'vir> task_encoder::OutputRefAny for IndirectPredicatesEncOutputRef<'vir> {}

fn projection_lifetime<'tcx>(
    proj: &LifetimeProjection<'tcx, RustTyDecomposition<'tcx>>,
) -> GeneralizedLifetime<'tcx> {
    let regions = PrustiPcgCtxt.extract_regions(proj.base());
    regions[proj.region_idx()]
}

impl TaskEncoder for IndirectPredicatesEnc {
    task_encoder::encoder_cache!(IndirectPredicatesEnc);
    const ENCODER_NAME: &'static str = "indirect predicates encoder";

    type TaskDescription<'vir> = LifetimeProjection<'vir, RustTyDecomposition<'vir>>;

    type TaskKey<'tcx> = Self::TaskDescription<'tcx>;

    type EncodingError = ();

    type OutputFullDependency<'vir> = IndirectPredicatesEncOutputRef<'vir>;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;
        vir::with_vcx(|vcx| {
            let task_lifetime = projection_lifetime(task_key);
            // TODO: the lifetime `RegionsIn(T)` of an opaque type `T` is
            // encoded as no resources, so whatever `T` borrows is omitted.
            let GeneralizedLifetime::Region(task_region) = task_lifetime else {
                return Ok(((), IndirectPredicatesEncOutputRef::default()));
            };
            let ty = task_key.base();
            let self_ty_enc = deps.require_dep::<TyUsePureEnc>(ty)?;
            let combined = ty.ty.zip(self_ty_enc);
            let mut predicates = vec![];
            // Collects (accessor, indirect_predicate) pairs for the fields of a
            // struct-like (used for structs and enum variants). Fields are
            // recursed into at their concrete (normalized) type.
            let collect_field_predicates =
                |struct_data: StructData<'vir, (RustTyDatas, UsePureTyDatas)>,
                 deps: &mut TaskEncoderDependencies<'vir, IndirectPredicatesEnc>| {
                    let mut result = vec![];
                    for (field_ty, accessor) in struct_data.fields {
                        let field_ty = field_ty.decompose_normalize(ty.args);
                        if let Some(new_projection) =
                            LifetimeProjection::new(field_ty, task_lifetime, None, PrustiPcgCtxt)
                        {
                            let field_indirect =
                                deps.require_dep::<IndirectPredicatesEnc>(new_projection)?;
                            for inner in field_indirect.predicates {
                                result.push((accessor, inner));
                            }
                        }
                    }
                    Ok(result)
                };
            match combined.specifics {
                // Optimisation: if there are no type arguments, there cannot be
                // anything behind a ref inside (except for 'static, which we
                // ignore for now). Plus it skips unsupported types if they
                // don't have lifetimes.
                _ if ty.args.args().is_empty() => (),
                TySpecifics::Primitive(_)
                | TySpecifics::ImmRef(_)
                | TySpecifics::Raw(_)
                | TySpecifics::Builtin(_) => (),
                // TODO: it's not valid to have nothing for these. We should fix
                // this by using an opaque predicate to represent potential
                // indirect stuff. For example:
                // fn foo<'a, T: Trait<'a>>(x: T) -> &'a mut i32 { x.get() }
                // Here, `T` could be instantiated as `&'a mut i32` in which
                // case we would want a wand with `i32(result) --* opaque_behind_a(x)`.
                // This is why we should return `opaque_behind_a(x)` here.
                TySpecifics::Param(_) | TySpecifics::Opaque(_) | TySpecifics::ArrayLike(_) => (),
                TySpecifics::MutRef((data, ref_domain)) => {
                    let inner_ty = data.referent.decompose_context(ty.ty.params, ty.args);
                    let inner_impure = deps.require_dep::<TyUseImpureEnc>(inner_ty)?;
                    let ref_region = PcgRegion::from(ty.args.args()[0].expect_region());
                    if ref_region == task_region {
                        let application = vcx.mk_lazy_expr(
                            "ref_indirect",
                            vir::TYPE_BOOL,
                            Box::new(move |vcx, self_expr: vir::ExprSnap<'vir>| {
                                let addr = ref_domain.deref_access(self_expr.downcast_ty());
                                inner_impure.ref_to_pred(vcx, addr, None).kind
                            }),
                        );
                        let unchanged = vcx.mk_lazy_expr(
                            "ref_indirect_unchanged",
                            vir::TYPE_BOOL,
                            Box::new(move |vcx, (self_expr, label): UnchangedInput<'vir>| {
                                let addr = ref_domain.deref_access(self_expr.downcast_ty());
                                let referent = inner_impure.ref_to_snap(addr);
                                vcx.mk_eq_expr(
                                    referent,
                                    vcx.mk_labelled_old_expr(referent, Some(label)),
                                )
                                .kind
                            }),
                        );
                        predicates.push(IndirectPredicate {
                            application,
                            unchanged,
                        });
                    }
                    // Collect the indirect predicates of the referent itself
                    // (e.g. of the inner reference in `&'a mut &'b mut i32`).
                    let referent_ty = data.referent.decompose_normalize(ty.args);
                    if let Some(new_projection) =
                        LifetimeProjection::new(referent_ty, task_lifetime, None, PrustiPcgCtxt)
                    {
                        let inner_indirect =
                            deps.require_dep::<IndirectPredicatesEnc>(new_projection)?;
                        let referent_snap = move |self_expr: ExprInput<'vir>| {
                            let inner_snap = inner_impure
                                .ref_to_snap(ref_domain.deref_access(self_expr.downcast_ty()));
                            ref_domain.cast_to_caller_ctx(inner_snap.downcast_ty())
                        };
                        predicates.extend(inner_indirect.predicates.into_iter().map(|inner| {
                            IndirectPredicate {
                                application: vcx.mk_lazy_expr(
                                    "ref_inner_indirect",
                                    vir::TYPE_BOOL,
                                    Box::new(move |vcx, self_expr: vir::ExprGenSnap<_, _>| {
                                        inner.application.reify(vcx, referent_snap(self_expr)).kind
                                    }),
                                ),
                                unchanged: vcx.mk_lazy_expr(
                                    "ref_inner_indirect_unchanged",
                                    vir::TYPE_BOOL,
                                    Box::new(
                                        move |vcx, (self_expr, label): UnchangedInput<'vir>| {
                                            let inner_snap = referent_snap(self_expr);
                                            inner.unchanged.reify(vcx, (inner_snap, label)).kind
                                        },
                                    ),
                                ),
                            }
                        }));
                    }
                }
                TySpecifics::StructLike(data) => {
                    // TODO: invalid recursion here if the defined struct is
                    // recursive!
                    for (accessor, inner) in collect_field_predicates(data, deps)? {
                        let inner_expr = inner.application;
                        let application = vcx.mk_lazy_expr(
                            "struct_field_indirect",
                            vir::TYPE_BOOL,
                            Box::new(move |vcx, self_expr: vir::ExprGenSnap<_, _>| {
                                inner_expr
                                    .reify(vcx, accessor.read(self_expr.downcast_ty()))
                                    .kind
                            }),
                        );
                        let unchanged = vcx.mk_lazy_expr(
                            "struct_field_indirect_unchanged",
                            vir::TYPE_BOOL,
                            Box::new(move |vcx, (self_expr, label): UnchangedInput<'vir>| {
                                let field_snap = accessor.read(self_expr.downcast_ty());
                                inner.unchanged.reify(vcx, (field_snap, label)).kind
                            }),
                        );
                        predicates.push(IndirectPredicate {
                            application,
                            unchanged,
                        });
                    }
                }
                TySpecifics::EnumLike(data) => {
                    let snap_to_discr_snap = data.data.1.snap_to_discr_snap;

                    let variant_preds = data
                        .variants
                        .into_iter()
                        .map(|variant| {
                            let fields = collect_field_predicates(variant.inner, deps)?;
                            Ok((variant.data.1.discr, fields))
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    if variant_preds.is_empty() {
                        return Ok(((), IndirectPredicatesEncOutputRef::default()));
                    }

                    let application = vcx.mk_lazy_expr(
                        "enum_variant_indirect",
                        vir::TYPE_BOOL,
                        Box::new({
                            let variant_preds = variant_preds.clone();
                            move |vcx, self_expr: vir::ExprGenSnap<_, _>| {
                                variant_case_split(
                                    vcx,
                                    snap_to_discr_snap,
                                    self_expr.downcast_ty(),
                                    &variant_preds,
                                    |field_snap, inner| inner.application.reify(vcx, field_snap),
                                )
                                .kind
                            }
                        }),
                    );
                    let unchanged = vcx.mk_lazy_expr(
                        "enum_variant_indirect_unchanged",
                        vir::TYPE_BOOL,
                        Box::new(move |vcx, (self_expr, label): UnchangedInput<'vir>| {
                            variant_case_split(
                                vcx,
                                snap_to_discr_snap,
                                self_expr.downcast_ty(),
                                &variant_preds,
                                |field_snap, inner| inner.unchanged.reify(vcx, (field_snap, label)),
                            )
                            .kind
                        }),
                    );
                    predicates.push(IndirectPredicate {
                        application,
                        unchanged,
                    });
                }
            };
            Ok(((), IndirectPredicatesEncOutputRef { predicates }))
        })
    }
}

pub fn projection_for_generalized_idx<'tcx>(
    ty: ty::Ty<'tcx>,
    idx: LifetimeProjectionIdx<Generalized>,
    decomp: RustTyDecomposition<'tcx>,
    tcx: TyCtxt<'tcx>,
) -> Option<LifetimeProjection<'tcx, RustTyDecomposition<'tcx>>> {
    let lifetimes: IndexVec<_, GeneralizedLifetime<'tcx>> = tcx.extract_regions(ty);
    LifetimeProjection::new(decomp, *lifetimes.get(idx)?, None, PrustiPcgCtxt)
}
