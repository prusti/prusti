use crate::TaskEncoder;
use vir::AdtGenData;

#[derive(Debug, Clone, Copy)]
pub struct OptionalDomain<'vir> {
    pub snapshot: &'vir vir::TypeData<'vir, vir::CSnap>,
    /// Constructs the some optional.
    pub some: vir::FunctionIdn<'vir, vir::PSnap, vir::CSnap>,
    /// Constructs the empty optional.
    pub none: vir::FunctionIdn<'vir, (), vir::CSnap>,
    /// Reads the value. Only meaningful if [`Self::is_some`] holds.
    pub value_access: vir::AdtDestructor<'vir, vir::CSnap, vir::PSnap>,
}

impl<'vir> OptionalDomain<'vir> {
    /// Whether the given optional holds a value.
    pub fn is_some<Curr: 'vir, Next: 'vir>(
        &self,
        optional: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenBool<'vir, Curr, Next> {
        vir::with_vcx(|vcx| vcx.mk_adt_discriminator_expr(optional, "s_Optional_some"))
    }
}

pub struct OptionalEnc;

impl TaskEncoder for OptionalEnc {
    task_encoder::encoder_cache!(OptionalEnc);
    const ENCODER_NAME: &'static str = "optional encoder";

    type TaskDescription<'vir> = ();

    type OutputFullLocal<'vir> = &'vir AdtGenData<'vir, (), !>;

    type OutputFullDependency<'vir> = OptionalDomain<'vir>;

    type EncodingError = ();

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for output in Self::all_outputs_local_no_errors(program) {
            program.add_adt(output);
        }
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut task_encoder::TaskEncoderDependencies<'vir, Self>,
    ) -> task_encoder::EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;

        let adt_ident = vir::ViperIdent::new("s_Optional");
        let snapshot = vir::DomainIdnCSnap::new(adt_ident, 1)(&[vir::TYPE_PSNAP][..]);

        vir::with_vcx(|vcx| {
            let typarams = vcx.alloc_slice(&[vcx.alloc(vir::DomainParamData {
                name: "OptionalTy",
                index: 0,
            })]);
            let dt = vir::TypeKind::DomainTypeParam(**typarams.first().unwrap());
            let ty = vcx.alloc(vir::TypeData::<vir::Dyn>::new(dt));
            let constructor_some = vcx.mk_adt_constructor(
                "s_Optional_some",
                vcx.alloc_slice(&[vcx.mk_local_decl("s_Optional_value", ty)]),
            );
            let constructor_none = vcx.mk_adt_constructor::<_, _, vir::Dyn>("s_Optional_none", &[]);
            let constructors = vcx.alloc_slice(&[constructor_some, constructor_none]);
            let adt_def = vcx.mk_adt(adt_ident, typarams, constructors);
            let some = vir::FunctionIdn::new(
                vir::ViperIdent::new(constructor_some.name),
                vir::TYPE_PSNAP,
                snapshot,
            );
            let none =
                vir::FunctionIdn::new(vir::ViperIdent::new(constructor_none.name), (), snapshot);
            let value_access = vcx.mk_adt_destructor("s_Optional_value", snapshot, vir::TYPE_PSNAP);
            Ok((
                adt_def,
                OptionalDomain {
                    snapshot,
                    some,
                    none,
                    value_access,
                },
            ))
        })
    }
}
