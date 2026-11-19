use crate::encoders::ty::{
    RustParam, RustParamData, RustTy,
    generics::ParamTypEnc,
    impure::{PredicateBuilder, TyImpureEnc, TyImpureParam},
    indirect_pred::IndirectPredEnc,
    pure::{TyPureBuilder, TyPureEnc, TyPureParam},
};
use task_encoder::{EncodeFullError, TaskEncoderDependencies};
use vir::CastType;

pub(crate) fn ty_pure<'vir>(
    data: &RustParam<'vir>,
    _deps: &mut TaskEncoderDependencies<'vir, TyPureEnc>,
    builder: &mut TyPureBuilder<'vir>,
) -> Result<TyPureParam<'vir>, EncodeFullError<'vir, TyPureEnc>> {
    // Only generic params share the `s_Param` adt; `dyn` keeps its own
    // domain.
    if let RustParamData::Dyn = data {
        builder.set_domain_builder();
    }
    Ok(())
}

pub(crate) fn ty_impure<'vir>(
    ty: RustTy<'vir>,
    data: &(&RustParam<'vir>, &TyPureParam<'vir>),
    deps: &mut TaskEncoderDependencies<'vir, TyImpureEnc>,
    builder: &mut PredicateBuilder<'vir>,
) -> Result<TyImpureParam<'vir>, EncodeFullError<'vir, TyImpureEnc>> {
    if let RustParamData::Dyn = data.0 {
        super::opaque::set_opaque(builder);
        return Ok(());
    }
    let typ = deps.require_dep::<ParamTypEnc>(())?.typ;
    let shallow = deps.require_ref::<IndirectPredEnc>(ty)?.shallow;
    builder.mk_predicate("", None);
    // The intended invariant of the predicate: its snapshot is a value of the
    // type the predicate is instantiated at. Everywhere a param is read out
    // of one, this is what lets the variant bridge reconstruct it from its
    // concrete value.
    //
    // This is assumed rather than checked: a param nested in a struct, enum
    // or reference has its snapshot set by that type's assign method, so
    // constraining `p_Param_assign` alone would not cover the producers.
    let vcx = builder.vcx;
    let result = vcx.mk_result(vir::TYPE_PSNAP);
    let ty_expr = builder.params.ty_exprs()[0];
    // A direct snapshot holds no extensions, so making it shallow changes nothing.
    let shallow = shallow.call()(result.upcast_ty(), &[ty_expr], &[]);
    builder.mk_snap_function(
        None,
        &[
            vcx.mk_eq_expr(typ(result), ty_expr),
            vcx.mk_eq_expr(shallow, result.upcast_ty()),
        ],
    );
    Ok(())
}
