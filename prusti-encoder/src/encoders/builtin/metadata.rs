use task_encoder::{EncodeFullResult, TaskEncoder, TaskEncoderDependencies};
use vir::{CastType, FunctionIdn};

use crate::encoders::{
    Pure,
    ty::{
        RustTyDecomposition, RustTyNormalized,
        generics::{GArgsCastEnc, GParams, TyExprEnc, builtin_impls::pointee_metadata},
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

/// The (uninterpreted) function giving a pointer's metadata after a cast from
/// a pointer to `from_ty` to a pointer to `to_ty`:
/// `metadata_cast(metadata, from_ty, to_ty)`. It is only defined by axioms:
/// the ones here, which hold for every pointer cast, and those for particular
/// types in `MetadataCastAxiomEnc`. Like rustc's `ptr_to_ptr`
/// (`rustc_const_eval/src/interpret/cast.rs`), a cast keeps the metadata
/// between pointees with the same metadata type and replaces it with `()` in a
/// cast to a thin pointer; rustc accepts no other raw-pointer casts (all trait
/// objects are one Viper type). An unsizing coercion from a sized type changes
/// the metadata type, so neither axiom applies to it. A coercion between trait
/// objects falls under `metadata_cast_eq`: it keeps the vtable, also for an
/// upcast, where rustc replaces it.
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
        let (_, pointee_metadata) = pointee_metadata(deps)?;
        vir::with_vcx(|vcx| {
            let domain_fn = vcx.mk_domain_function(fn_idn, false, None);
            let input_decl = vcx.mk_local_decl("input", vir::TYPE_PSNAP);
            let input = vcx.mk_local_ex(input_decl);
            let from_decl = vcx.mk_local_decl("from", vir::TYPE_TYVAL);
            let from = vcx.mk_local_ex(from_decl);
            let to_decl = vcx.mk_local_decl("to", vir::TYPE_TYVAL);
            let to = vcx.mk_local_ex(to_decl);
            let call = fn_idn.call()(input, from, to);
            let from_metadata = pointee_metadata(&[from], &[]);
            let to_metadata = pointee_metadata(&[to], &[]);

            // The two conditions are disjoint: both conclusions together would
            // equate `input` with `()` for any `input`.
            let eq = vir::expr! {
                forall [input_decl], [from_decl], [to_decl] :: { [call] }
                    ((from_metadata) == (to_metadata)) ==> ((call) == (input))
            };
            let axiom_eq = vcx.mk_domain_axiom(vir::ViperIdent::new("metadata_cast_eq"), eq);
            let thin = vir::expr! {
                forall [input_decl], [from_decl], [to_decl] :: { [call] }
                    ((!((from_metadata) == (unit_ty))) && ((to_metadata) == (unit_ty)))
                        ==> ((call) == (unit_metadata))
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
