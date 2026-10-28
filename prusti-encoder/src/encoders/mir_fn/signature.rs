use pcg::borrow_pcg::FunctionData;
use prusti_rustc_interface::{middle::ty, span::def_id::DefId};
use task_encoder::{EncodeFullError, TaskEncoder, TaskEncoderDependencies};

use crate::encoders::{
    Pure,
    ty::{
        LazyRustTy,
        generics::{GArgCaster, GArgs, GArgsCastEnc, GParams},
    },
};

pub struct RustSignature<'tcx> {
    pub gparams: GParams<'tcx>,
    pub inputs: &'tcx [LazyRustTy<'tcx>],
    pub output: LazyRustTy<'tcx>,
}

impl<'tcx> RustSignature<'tcx> {
    pub fn new(def_id: DefId) -> Self {
        // For a closure, this is the signature of its body: the environment
        // followed by the untupled arguments.
        let fn_sig = vir::with_vcx(|vcx| FunctionData::new(def_id).identity_fn_sig(vcx.tcx()));
        let gparams = GParams::from(def_id);
        let inputs = LazyRustTy::new_slice(fn_sig.inputs());
        let output = LazyRustTy::new(fn_sig.output());
        Self {
            gparams,
            inputs,
            output,
        }
    }

    /// The casters of the arguments and of the result between a call with
    /// `args` and the function's own generic context (see `GArgsCastEnc`).
    #[allow(clippy::type_complexity)]
    pub fn casters<E: TaskEncoder>(
        &self,
        args: GArgs<'tcx>,
        deps: &mut TaskEncoderDependencies<'tcx, E>,
    ) -> Result<(Vec<GArgCaster<'tcx, Pure>>, GArgCaster<'tcx, Pure>), EncodeFullError<'tcx, E>>
    {
        let mut caster = |ty: &LazyRustTy<'tcx>| {
            let normalized = ty.decompose_compare_normalize(self.gparams, args);
            deps.require_dep::<GArgsCastEnc<Pure>>(normalized)
        };
        let inputs = self
            .inputs
            .iter()
            .map(&mut caster)
            .collect::<Result<_, _>>()?;
        let output = caster(&self.output)?;
        Ok((inputs, output))
    }

    pub fn get_def_id_and_caller_substs(ty: ty::Ty<'tcx>) -> (DefId, ty::GenericArgsRef<'tcx>) {
        match ty.kind() {
            ty::TyKind::FnDef(def_id, substs) => (*def_id, substs.skip_binder()),
            ty::TyKind::Closure(def_id, substs) => (*def_id, substs),
            _ => todo!(),
        }
    }
}
