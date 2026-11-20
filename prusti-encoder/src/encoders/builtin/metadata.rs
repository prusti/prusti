use task_encoder::{EncodeFullResult, TaskEncoder, TaskEncoderDependencies};
use vir::{CastType, FunctionIdn};

use crate::encoders::{
    Pure,
    ty::{
        RustTyDecomposition, RustTyNormalized,
        generics::{GArgsCastEnc, GParams, TyExprEnc},
        use_pure::TyUsePureEnc,
    },
};

/// The (uninterpreted) function transferring the *referent value* of a `&mut`
/// across an unsize coercion: `value_cast(old_referent_snap, from_ty, to_ty)` is
/// the new referent snapshot. Like `metadata_cast`, it is only defined by
/// axioms; the array->slice axioms are added (for the concrete element types) in
/// `MirBuiltinUseCastEnc`. The identity axiom handles degenerate `T -> T`
/// reborrows; every other (e.g. `Struct -> dyn`) coercion is left unconstrained.
pub struct ValueCastEnc;

impl TaskEncoder for ValueCastEnc {
    task_encoder::encoder_cache!(ValueCastEnc);
    const ENCODER_NAME: &'static str = "unsize value cast encoder";
    type TaskDescription<'vir> = ();
    type OutputFullDependency<'vir> =
        FunctionIdn<'vir, (vir::PSnap, vir::TyVal, vir::TyVal), vir::PSnap>;
    type OutputFullLocal<'vir> = vir::Domain<'vir>;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;
        let idn = vir::ViperIdent::new("unsize_value_cast");
        let fn_idn = vir::FunctionIdn::new(
            idn,
            (vir::TYPE_PSNAP, vir::TYPE_TYVAL, vir::TYPE_TYVAL),
            vir::TYPE_PSNAP,
        );
        vir::with_vcx(|vcx| {
            let domain_fn = vcx.mk_domain_function(fn_idn, false, None);
            // See comment in `MetadataCastEnc` about the corresponding axiom there.
            // forall input: s_Param, same: Type :: { value_cast(input, same, same) }
            //     value_cast(input, same, same) == input
            let expr = vir::expr! {
                forall input: PSnap, same: Type :: { [fn_idn](input, same, same) }
                    ([fn_idn](input, same, same)) == (input)
            };
            let axiom = vcx.mk_domain_axiom(vir::ViperIdent::new("unsize_value_cast_eq"), expr);
            let domain = vcx.mk_domain(
                vir::ViperIdent::new("UnsizeValueCast"),
                &[],
                vcx.alloc_slice(&[axiom]),
                vcx.alloc_slice(&[domain_fn]),
                None,
            );
            Ok((domain, fn_idn))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        match Self::all_outputs_local(program).as_slice() {
            [] => (),
            [domain] => program.add_domain(domain),
            _ => unreachable!(),
        }
    }
}

/// The (uninterpreted) function giving a pointer's metadata after a cast:
/// `metadata_cast(metadata, from_ty, to_ty)`. An unsizing coercion passes the
/// referent types, a raw-pointer cast the pointee *metadata* types. It is only
/// defined by axioms: the universally valid ones here, and those for
/// particular types in `MetadataCastAxiomEnc`. For raw-pointer casts, like
/// rustc's `ptr_to_ptr` (`rustc_const_eval/src/interpret/cast.rs`), the ones
/// here keep the metadata between pointers with the same metadata type and
/// replace it with `()` in a cast to a thin pointer. The only other casts rustc
/// accepts are between trait objects that differ in auto traits, whose
/// metadata types are the same Viper type, as are all trait objects.
pub struct MetadataCastEnc;

impl TaskEncoder for MetadataCastEnc {
    task_encoder::encoder_cache!(MetadataCastEnc);
    const ENCODER_NAME: &'static str = "metadata cast encoder";
    type TaskDescription<'vir> = ();
    type OutputFullDependency<'vir> =
        FunctionIdn<'vir, (vir::PSnap, vir::TyVal, vir::TyVal), vir::PSnap>;
    type OutputFullLocal<'vir> = vir::Domain<'vir>;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;
        let idn = vir::ViperIdent::new("metadata_cast");
        let fn_idn = vir::FunctionIdn::new(
            idn,
            (vir::TYPE_PSNAP, vir::TYPE_TYVAL, vir::TYPE_TYVAL),
            vir::TYPE_PSNAP,
        );
        // The type `()` and its value as an `s_Param`, the snapshot type in
        // which pointers hold their metadata.
        let unit = vir::with_vcx(|vcx| {
            RustTyDecomposition::from_ty(vcx.tcx().types.unit, GParams::empty())
        });
        let unit_ty = deps.require_dep::<TyExprEnc>(unit)?;
        let unit_snap = deps
            .require_dep::<TyUsePureEnc>(unit)?
            .zst_to_snap()
            .unwrap();
        let unit_caster = deps.require_dep::<GArgsCastEnc<Pure>>(Some(RustTyNormalized {
            param: RustTyDecomposition::param(),
            concrete: unit,
        }))?;
        let unit_metadata = unit_caster
            .cast_to_callee_ctx(unit_snap.upcast_ty())
            .downcast_ty::<vir::PSnap>();
        vir::with_vcx(|vcx| {
            let domain_fn = vcx.mk_domain_function(fn_idn, false, None);

            // A cast between the same types leaves the metadata unchanged, and
            // a cast to the metadata type `()` from another one gives `()`.
            // The latter states the former for `()`, rather than `()` for
            // every `input`, which would contradict the former when `input` is
            // not `()`. Casts that rewrite the metadata otherwise (e.g. array
            // -> slice, where the metadata becomes the slice length) are
            // attached per type in `MetadataCastAxiomEnc`.
            let eq = vir::expr! {
                forall input: PSnap, same: Type :: { [fn_idn](input, same, same) }
                    ([fn_idn](input, same, same)) == (input)
            };
            let axiom_eq = vcx.mk_domain_axiom(vir::ViperIdent::new("metadata_cast_eq"), eq);
            let thin = vir::expr! {
                forall input: PSnap, from: Type :: { [fn_idn](input, from, unit_ty) }
                    ([fn_idn](input, from, unit_ty))
                        == (((from) == (unit_ty)) ? (input) : (unit_metadata))
            };
            let axiom_thin = vcx.mk_domain_axiom(vir::ViperIdent::new("metadata_cast_thin"), thin);

            let domain = vcx.mk_domain(
                vir::ViperIdent::new("MetadataCast"),
                &[],
                vcx.alloc_slice(&[axiom_eq, axiom_thin]),
                vcx.alloc_slice(&[domain_fn]),
                None,
            );
            Ok((domain, fn_idn))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        match Self::all_outputs_local(program).as_slice() {
            [] => (),
            [domain] => program.add_domain(domain),
            _ => unreachable!(),
        }
    }
}
