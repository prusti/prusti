use pcg::borrow_pcg::FunctionData;
use prusti_rustc_interface::{middle::ty, span::def_id::DefId};

use crate::encoders::ty::{LazyRustTy, generics::GParams};

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

    pub fn get_def_id_and_caller_substs(ty: ty::Ty<'tcx>) -> (DefId, ty::GenericArgsRef<'tcx>) {
        match ty.kind() {
            ty::TyKind::FnDef(def_id, substs) => (*def_id, substs.skip_binder()),
            ty::TyKind::Closure(def_id, substs) => (*def_id, substs),
            _ => todo!(),
        }
    }
}
