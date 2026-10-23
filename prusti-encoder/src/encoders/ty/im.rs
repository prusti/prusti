//! Interior mutability, address-keyed (the PLDI27 design).
//!
//! An `UnsafeCell`'s contents are an interior location: its predicate holds
//! only the place component, and its snapshot has a slot for the contents.
//! Shallow snapshots (what predicates hold) have [`ImUnsetEnc`] in the slot,
//! deep snapshots (what specifications and `#[pure_unstable]` functions see)
//! have the contents.

use task_encoder::TaskEncoder;

/// `im_unset(T): s_Param`, the slot of a shallow `UnsafeCell<T>` snapshot.
pub(crate) struct ImUnsetEnc;

pub(crate) type ImUnset<'vir> = vir::FunctionIdn<'vir, vir::TyVal, vir::PSnap>;

impl TaskEncoder for ImUnsetEnc {
    task_encoder::encoder_cache!(ImUnsetEnc);
    const ENCODER_NAME: &'static str = "interior mutability unset slot encoder";
    type TaskDescription<'vir> = ();
    type OutputFullDependency<'vir> = ImUnset<'vir>;
    type OutputFullLocal<'vir> = vir::Domain<'vir>;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        _task_key: &Self::TaskKey<'vir>,
        deps: &mut task_encoder::TaskEncoderDependencies<'vir, Self>,
    ) -> task_encoder::EncodeFullResult<'vir, Self> {
        deps.emit_output_ref((), ())?;
        vir::with_vcx(|vcx| {
            let unset: ImUnset<'vir> = vir::FunctionIdn::new(
                vir::vir_format_identifier!(vcx, "im_unset"),
                vir::TYPE_TYVAL,
                vir::TYPE_PSNAP,
            );
            let domain = vcx.mk_domain(
                vir::ViperIdent::new("def_im_unset"),
                &[],
                &[],
                vcx.alloc_slice(&[vcx.mk_domain_function(unset, false, None)]),
                None,
            );
            Ok((domain, unset))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for domain in Self::all_outputs_local_no_errors(program) {
            program.add_domain(domain);
        }
    }
}
