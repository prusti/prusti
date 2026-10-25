use prusti_rustc_interface::{abi, middle::ty};
use task_encoder::{EncodeFullError, EncodeFullResult, TaskEncoder};
use vir::CastType;

use crate::encoders::{
    Pure,
    ty::{
        LazyRustTy, RustTyDatas,
        generics::{GArgs, GArgsCastEnc, GArgsTyEnc, GParams},
    },
};

use super::{
    TyUseEnc, UseTyDatas,
    data::*,
    generics::{GArgCaster, GArgsTy},
    pure::{PureTyDatas, TyPureEnc, TyPureFieldRef, TyPureRef},
    rust_ty::RustTySpecial,
};

pub(super) type UsePureTyDatas = UseTyDatas<Pure>;

type FieldCaster<'vir> = GArgCaster<'vir, Pure>;

impl<'vir> TyDatas<'vir> for UsePureTyDatas {
    type TyData = TyUsePureRef<'vir>;
    type OpaqueData = <PureTyDatas as TyDatas<'vir>>::OpaqueData;
    type ParamData = <PureTyDatas as TyDatas<'vir>>::ParamData;
    type ArrayData = TyUsePureArrayData<'vir>;
    type PrimitiveData = <PureTyDatas as TyDatas<'vir>>::PrimitiveData;
    type ImmRefData = TyUsePureImmRef<'vir>;
    type MutRefData = TyUsePureMutRef<'vir>;
    type RawData = TyUsePureRaw<'vir>;
    type FieldData = TyUsePureField<'vir>;
    type StructData = TyUsePureStructData<'vir>;
    type VariantData = <PureTyDatas as TyDatas<'vir>>::VariantData;
    type EnumData = <PureTyDatas as TyDatas<'vir>>::EnumData;
    type BuiltinData = TyUsePureBuiltinData<'vir>;
}

pub type TyUsePure<'vir> = Ty<'vir, UsePureTyDatas>;
pub type TyUsePureArray<'vir> = ArrayData<'vir, UsePureTyDatas>;
pub type TyUsePureStruct<'vir> = StructData<'vir, UsePureTyDatas>;
pub type TyUsePureEnum<'vir> = EnumData<'vir, UsePureTyDatas>;

/// Use-side data for a `prusti_contracts` builtin. The collection builtins
/// store their elements as generic snapshots (`s_Param`), so their operations
/// cast the elements with the casters here; `Int`/`Real` have no elements.
#[derive(Debug, Clone, Copy)]
pub struct TyUsePureBuiltinData<'vir> {
    pub pure: <PureTyDatas as TyDatas<'vir>>::BuiltinData,
    /// `[elem, NoCast]` for `Set`/`Multiset`/`Seq`, `[key, val]` for `Map`,
    /// and `[NoCast; 2]` for `Int`/`Real`.
    casters: [FieldCaster<'vir>; 2],
}

impl<'vir> TyUsePureBuiltinData<'vir> {
    /// The element caster of a `Set`/`Multiset`/`Seq`.
    pub fn elem_caster(&self) -> &FieldCaster<'vir> {
        assert!(matches!(
            self.pure,
            super::pure::TyPureBuiltinData::Set
                | super::pure::TyPureBuiltinData::Multiset
                | super::pure::TyPureBuiltinData::Seq
        ));
        &self.casters[0]
    }

    pub fn map_key_caster(&self) -> &FieldCaster<'vir> {
        assert!(matches!(self.pure, super::pure::TyPureBuiltinData::Map));
        &self.casters[0]
    }

    pub fn map_val_caster(&self) -> &FieldCaster<'vir> {
        assert!(matches!(self.pure, super::pure::TyPureBuiltinData::Map));
        &self.casters[1]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TyUsePureRaw<'vir> {
    /// Caster for the pointer-metadata type, converting the generic
    /// `metadata_access` result into the concrete metadata snapshot.
    metadata_caster: FieldCaster<'vir>,
    pure: <PureTyDatas as TyDatas<'vir>>::RawData,
}

#[derive(Debug, Clone, Copy)]
pub struct TyUsePureImmRef<'vir> {
    referent_caster: FieldCaster<'vir>,
    metadata_caster: FieldCaster<'vir>,
    pure: <PureTyDatas as TyDatas<'vir>>::ImmRefData,
}

#[derive(Debug, Clone)]
pub struct TyUsePureMutRef<'vir> {
    referent_caster: FieldCaster<'vir>,
    metadata_caster: FieldCaster<'vir>,
    pure: <PureTyDatas as TyDatas<'vir>>::MutRefData,
}

#[derive(Debug, Clone, Copy)]
pub struct TyUsePureArrayData<'vir> {
    caster: FieldCaster<'vir>,
    #[allow(dead_code)]
    args: GArgsTy<'vir>,
    pure: <PureTyDatas as TyDatas<'vir>>::ArrayData,
}

#[derive(Debug, Clone, Copy)]
pub struct TyUsePureField<'vir> {
    caster: FieldCaster<'vir>,
    #[allow(dead_code)]
    args: GArgsTy<'vir>,
    pure: <PureTyDatas as TyDatas<'vir>>::FieldData,
}

/// Use-side data for the hardcoded extras of a `Box`.
#[derive(Debug, Clone, Copy)]
pub struct TyUsePureBoxData<'vir> {
    metadata_caster: FieldCaster<'vir>,
}

#[derive(Debug, Clone, Copy)]
pub struct TyUsePureStructData<'vir> {
    #[allow(dead_code)]
    args: GArgsTy<'vir>,
    pure: <PureTyDatas as TyDatas<'vir>>::StructData,
    box_data: Option<TyUsePureBoxData<'vir>>,
}

/// Encodes a type into the snapshot representation. Takes an arbitrary Rust
/// `Ty` and provides a wrapper around the results of the `DomainEnc` encoder.
/// This wrapper handles all the generic casts required.
pub type TyUsePureEnc = TyUseEnc<Pure>;

type EncResult<'vir, T> = Result<T, EncodeFullError<'vir, TyUsePureEnc>>;

#[derive(Debug, Clone, Copy)]
pub struct TyUsePureRef<'vir> {
    pub snapshot: vir::TypeSnap<'vir>,
    args: GArgsTy<'vir>,
    ty_pure_ref: TyPureRef<'vir>,
}

impl<'vir> task_encoder::OutputRefAny for TyUsePureRef<'vir> {}

impl TaskEncoder for TyUsePureEnc {
    task_encoder::encoder_cache!(TyUsePureEnc);

    type TaskDescription<'vir> = super::RustTyDecomposition<'vir>;

    type OutputRef<'vir> = TyUsePureRef<'vir>;
    type OutputFullDependency<'vir> = TyUsePure<'vir>;

    type TaskKey<'tcx> = Self::TaskDescription<'tcx>;

    type EncodingError = ();

    const ENCODER_NAME: &'static str = "pure type encoder";

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut task_encoder::TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        let ty_pure_ref = deps.require_ref::<TyPureEnc>(task_key.ty)?;
        let args = deps.require_dep::<GArgsTyEnc>(task_key.args)?;
        let snapshot = ty_pure_ref.snapshot;
        let inner = TyUsePureRef {
            args,
            snapshot,
            ty_pure_ref,
        };
        deps.emit_output_ref(*task_key, inner)?;

        let ty_pure = deps.require_dep::<TyPureEnc>(task_key.ty)?;
        let ty = task_key.ty.zip(ty_pure);
        let mut walker = TyUsePureWalker::new(deps, task_key.args)?;
        let specifics = walker.encode_ty(ty)?;
        let ty_use_pure = TyData::new(inner, specifics);
        Ok(((), ty_use_pure.alloc()))
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        TyPureEnc::emit_outputs(program)
    }
}

struct TyUsePureWalker<'a, 'vir> {
    deps: &'a mut task_encoder::TaskEncoderDependencies<'vir, TyUsePureEnc>,
    args_t: GArgsTy<'vir>,
    args: GArgs<'vir>,
}

impl<'a, 'vir> TyUsePureWalker<'a, 'vir> {
    fn new(
        deps: &'a mut task_encoder::TaskEncoderDependencies<'vir, TyUsePureEnc>,
        args: GArgs<'vir>,
    ) -> EncResult<'vir, Self> {
        let args_t = deps.require_dep::<GArgsTyEnc>(args)?;
        Ok(TyUsePureWalker { deps, args_t, args })
    }

    fn encode_ty(
        &mut self,
        ty: TyData<'vir, (RustTyDatas, PureTyDatas)>,
    ) -> EncResult<'vir, TySpecifics<'vir, UsePureTyDatas>> {
        let specifics = match &ty.specifics {
            TySpecifics::Param(data) => {
                let _: () = *data.1;
                TySpecifics::mk_param(())
            }
            TySpecifics::Opaque(data) => TySpecifics::mk_opaque(*data.1),
            TySpecifics::Raw((data, raw_domain)) => {
                let metadata_caster = self.encode_normalized(data.metadata, ty.0.params)?;
                TySpecifics::mk_raw(TyUsePureRaw {
                    metadata_caster,
                    pure: **raw_domain,
                })
            }
            TySpecifics::Primitive(data) => TySpecifics::mk_primitive(*data.1),
            TySpecifics::ImmRef((data, ref_domain)) => {
                let referent_caster = self.encode_normalized(data.referent, ty.0.params)?;
                let metadata_caster = self.encode_normalized(data.metadata, ty.0.params)?;
                TySpecifics::mk_immref(TyUsePureImmRef {
                    referent_caster,
                    metadata_caster,
                    pure: **ref_domain,
                })
            }
            TySpecifics::MutRef((data, ref_domain)) => {
                let referent_caster = self.encode_normalized(data.referent, ty.0.params)?;
                let metadata_caster = self.encode_normalized(data.metadata, ty.0.params)?;
                TySpecifics::mk_mutref(TyUsePureMutRef {
                    referent_caster,
                    metadata_caster,
                    pure: **ref_domain,
                })
            }
            TySpecifics::ArrayLike(data) => {
                TySpecifics::ArrayLike(self.encode_array(data, ty.0.params)?)
            }
            TySpecifics::StructLike(data) => {
                let box_data = match ty.0.special {
                    RustTySpecial::Box => Some(TyUsePureBoxData {
                        metadata_caster: self
                            .encode_normalized(ty.box_metadata_ty(), ty.0.params)?,
                    }),
                    RustTySpecial::None | RustTySpecial::UnsafeCell | RustTySpecial::Holds(..) => {
                        None
                    }
                };
                TySpecifics::StructLike(self.encode_structlike(data, ty.0.params, box_data)?)
            }
            TySpecifics::EnumLike(data) => {
                TySpecifics::EnumLike(self.encode_enumlike(data, ty.0.params)?)
            }
            TySpecifics::Builtin(data) => {
                use super::rust_ty::RustBuiltinData;
                let no_cast = GArgCaster::NoCast;
                let casters = match data.0 {
                    RustBuiltinData::Int | RustBuiltinData::Real => [no_cast, no_cast],
                    RustBuiltinData::Set(elem)
                    | RustBuiltinData::Multiset(elem)
                    | RustBuiltinData::Seq(elem) => {
                        [self.encode_normalized(*elem, ty.0.params)?, no_cast]
                    }
                    RustBuiltinData::Map(key, val) => [
                        self.encode_normalized(*key, ty.0.params)?,
                        self.encode_normalized(*val, ty.0.params)?,
                    ],
                };
                TySpecifics::mk_builtin(TyUsePureBuiltinData {
                    pure: *data.1,
                    casters,
                })
            }
        };
        Ok(specifics)
    }

    fn encode_normalized(
        &mut self,
        inner: LazyRustTy<'vir>,
        params: GParams<'vir>,
    ) -> EncResult<'vir, FieldCaster<'vir>> {
        let normalized = inner.decompose_compare_normalize(params, self.args);
        self.deps.require_dep::<GArgsCastEnc<Pure>>(normalized)
    }

    fn encode_array(
        &mut self,
        data: &ArrayData<'vir, (RustTyDatas, PureTyDatas)>,
        params: GParams<'vir>,
    ) -> EncResult<'vir, ArrayData<'vir, UsePureTyDatas>> {
        let caster = self.encode_normalized(*data.0, params)?;
        let slice = data.slice;
        let data = TyUsePureArrayData {
            caster,
            args: self.args_t,
            pure: *data.data.1,
        };
        Ok(ArrayData::new(data, slice))
    }

    fn encode_structlike(
        &mut self,
        data: &StructData<'vir, (RustTyDatas, PureTyDatas)>,
        params: GParams<'vir>,
        box_data: Option<TyUsePureBoxData<'vir>>,
    ) -> EncResult<'vir, StructData<'vir, UsePureTyDatas>> {
        let fields = data
            .fields
            .iter()
            .map(|field| {
                let caster = self.encode_normalized(field.0.ty(), params)?;
                Ok(TyUsePureField {
                    caster,
                    args: self.args_t,
                    pure: *field.1,
                })
            })
            .collect::<EncResult<'vir, Vec<_>>>()?;
        let data = TyUsePureStructData {
            args: self.args_t,
            pure: *data.1,
            box_data,
        };
        Ok(StructData::new(data, fields))
    }

    fn encode_enumlike(
        &mut self,
        data: &EnumData<'vir, (RustTyDatas, PureTyDatas)>,
        params: GParams<'vir>,
    ) -> EncResult<'vir, EnumData<'vir, UsePureTyDatas>> {
        let variants = data
            .variants
            .iter()
            .map(|variant| {
                let structlike = self.encode_structlike(&variant.inner, params, None)?;
                Ok(VariantData::new(*variant.1, structlike))
            })
            .collect::<EncResult<'vir, Vec<_>>>()?;
        Ok(EnumData::new(*data.1, variants))
    }
}

impl<'vir> TyUsePureRef<'vir> {
    pub fn unreachable_to_snap<Curr, Next>(&self) -> vir::ExprGenSnap<'vir, Curr, Next> {
        self.ty_pure_ref.unreachable_to_snap.call()(self.args.get_ty(), self.args.get_const())
    }

    pub fn arbitrary_to_snap<Curr, Next>(&self) -> vir::ExprGenSnap<'vir, Curr, Next> {
        self.ty_pure_ref.arbitrary_to_snap.call()(self.args.get_ty(), self.args.get_const())
    }
}

impl<'vir> TyData<'vir, UsePureTyDatas> {
    /// The snapshot of a zero-sized type such as `()`: all values are equal, so
    /// it is built with the regular (zero-field) constructor. Returns `None`
    /// when the type is not a fieldless struct-like, leaving error handling to
    /// the caller.
    pub fn zst_to_snap<Curr, Next>(&self) -> Option<vir::ExprGenCSnap<'vir, Curr, Next>> {
        match &self.specifics {
            TySpecifics::StructLike(data) if data.fields.is_empty() => {
                Some(data.field_snaps_to_snap(Vec::new()))
            }
            _ => None,
        }
    }

    pub fn metadata_access<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenSnap<'vir, Curr, Next> {
        match &self.specifics {
            TySpecifics::Raw(data) => data.metadata_access(snap),
            TySpecifics::ImmRef(data) => data.metadata_access(snap),
            TySpecifics::MutRef(data) => data.metadata_access(snap),
            // Cannot be reached as per `UnOp::PtrMetadata` description
            _ => unreachable!("metadata_access called on non-ref type"),
        }
    }

    /// Replaces the address of a shared reference snapshot with a dummy value,
    /// leaving any other snapshot unchanged. Applied to the arguments of a
    /// pure function call so that the result cannot depend on the identity of
    /// a shared reference, only on the value it points to.
    pub fn dummy_ref_address<Curr, Next>(
        &self,
        snap: vir::ExprGenSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenSnap<'vir, Curr, Next> {
        match &self.specifics {
            TySpecifics::ImmRef(data) => data.value_snap_of(snap.downcast_ty()).upcast_ty(),
            _ => snap,
        }
    }
}

impl<'vir> TyUsePureImmRef<'vir> {
    pub fn prim_to_snap<Curr, Next>(
        &self,
        ref_: vir::ExprGenRef<'vir, Curr, Next>,
        metadata: vir::ExprGenSnap<'vir, Curr, Next>,
        inner: vir::ExprGenSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenCSnap<'vir, Curr, Next> {
        let metadata = self.metadata_caster.cast_to_callee_ctx(metadata);
        let inner = self.referent_caster.cast_to_callee_ctx(inner);
        self.pure.prim_to_snap.call()(ref_, metadata.downcast_ty(), inner.downcast_ty())
    }

    pub fn addr_access<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenRef<'vir, Curr, Next> {
        self.pure.deref_access.call()(snap)
    }

    pub fn metadata_access<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenSnap<'vir, Curr, Next> {
        let metadata = self.pure.metadata_access.call()(snap);
        self.metadata_caster
            .cast_to_caller_ctx(metadata.upcast_ty())
    }

    /// The snapshot with the address replaced by `null`.
    pub fn value_snap_of<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenCSnap<'vir, Curr, Next> {
        vir::with_vcx(|vcx| {
            self.pure.prim_to_snap.call()(
                vcx.mk_null().lazy(),
                self.pure.metadata_access.call()(snap),
                self.pure.value_access.call()(snap),
            )
        })
    }

    pub fn value_access<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenSnap<'vir, Curr, Next> {
        let value = self.pure.value_access.call()(snap);
        self.referent_caster.cast_to_caller_ctx(value.upcast_ty())
    }
}

impl<'vir> TyUsePureRaw<'vir> {
    /// Construct the raw-pointer snapshot from an address and (fat) pointer
    /// metadata. The pointee is opaque, so there is no value/referent argument.
    pub fn prim_to_snap<Curr, Next>(
        &self,
        ref_: vir::ExprGenRef<'vir, Curr, Next>,
        metadata: vir::ExprGenSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenCSnap<'vir, Curr, Next> {
        let metadata = self.metadata_caster.cast_to_callee_ctx(metadata);
        self.pure.prim_to_snap.call()(ref_, metadata.downcast_ty())
    }

    pub fn address_access<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenRef<'vir, Curr, Next> {
        self.pure.address_access.call()(snap)
    }

    pub fn metadata_access<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenSnap<'vir, Curr, Next> {
        let metadata = self.pure.metadata_access.call()(snap);
        self.metadata_caster
            .cast_to_caller_ctx(metadata.upcast_ty())
    }
}

impl<'vir> TyUsePureMutRef<'vir> {
    pub fn prim_to_snap<Curr, Next>(
        &self,
        ref_: vir::ExprGenRef<'vir, Curr, Next>,
        metadata: vir::ExprGenSnap<'vir, Curr, Next>,
        val: vir::ExprGenSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenCSnap<'vir, Curr, Next> {
        let metadata = self
            .metadata_caster
            .cast_to_callee_ctx(metadata)
            .downcast_ty();
        self.pure.prim_to_snap.call()(ref_, metadata, self.cast_to_callee_ctx(val))
    }

    pub fn deref_access<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenRef<'vir, Curr, Next> {
        self.pure.deref_access.call()(snap)
    }

    pub fn metadata_access<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenSnap<'vir, Curr, Next> {
        let metadata = self.pure.metadata_access.call()(snap);
        self.metadata_caster
            .cast_to_caller_ctx(metadata.upcast_ty())
    }

    /// Function to access the value (beware that this may not be set).
    pub fn value_access<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenSnap<'vir, Curr, Next> {
        self.cast_to_caller_ctx(self.pure.value_access.call()(snap))
    }

    pub fn cast_to_caller_ctx<Curr, Next>(
        &self,
        inner_snap: vir::ExprGenPSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenSnap<'vir, Curr, Next> {
        self.referent_caster
            .cast_to_caller_ctx(inner_snap.upcast_ty())
    }

    pub fn cast_to_callee_ctx<Curr, Next>(
        &self,
        inner_snap: vir::ExprGenSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenPSnap<'vir, Curr, Next> {
        self.referent_caster
            .cast_to_callee_ctx(inner_snap)
            .downcast_ty()
    }
}

impl<'vir> TyUsePureArray<'vir> {
    pub fn index<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
        index: vir::ExprGenInt<'vir, Curr, Next>,
    ) -> vir::ExprGenSnap<'vir, Curr, Next> {
        let res = self.pure.index_access.call()(snap, index);
        self.caster.cast_to_caller_ctx(res.upcast_ty())
    }

    /// The element snapshot at `index` in its generic (`p_Param`) form, i.e.
    /// without applying the element caster. Use when relating two array-likes
    /// that share the same generic element type, where the concrete conversion
    /// would be redundant.
    pub fn index_generic<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
        index: vir::ExprGenInt<'vir, Curr, Next>,
    ) -> vir::ExprGenSnap<'vir, Curr, Next> {
        self.pure.index_access.call()(snap, index).upcast_ty()
    }

    /// Get the (Ref) address of an index. Identical to the function one would
    /// call in `use_pure`.
    pub fn ref_to_index_ref<Curr, Next>(
        &self,
        self_ref: vir::ExprGenRef<'vir, Curr, Next>,
        index: vir::ExprGenInt<'vir, Curr, Next>,
    ) -> vir::ExprGenRef<'vir, Curr, Next> {
        self.data.pure.ref_to_index_ref.call()(self_ref, index, self.args.get_ty())
    }
}

impl<'vir> TyUsePureStruct<'vir> {
    /// The `Unique` field's snapshot of a `Box`, where its pointer lives.
    #[track_caller]
    fn box_unique_snap<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenCSnap<'vir, Curr, Next> {
        assert!(self.data.box_data.is_some(), "expected box");
        self[abi::FieldIdx::ZERO].read(snap).downcast_ty()
    }

    /// Function to access the address of the boxed `T` (read out of the raw
    /// pointer's snapshot).
    #[track_caller]
    pub fn box_address_access<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenRef<'vir, Curr, Next> {
        assert!(self.data.box_data.is_some(), "expected box");
        let TyPureFieldRef::Dynamic(address_access) =
            self.fields.last().unwrap().pure.ref_to_field_ref
        else {
            unreachable!()
        };
        address_access.call()(self.box_unique_snap(snap))
    }

    /// Function to access the pointer metadata of a `Box`.
    #[track_caller]
    pub fn box_metadata_access<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenSnap<'vir, Curr, Next> {
        let box_data = self.data.pure.box_data.expect("expected box");
        let metadata = box_data.metadata_access.call()(self.box_unique_snap(snap));
        self.data
            .box_data
            .expect("expected box")
            .metadata_caster
            .cast_to_caller_ctx(metadata.upcast_ty())
    }

    /// Function to access the value of the boxed `T` (the box's last field).
    #[track_caller]
    pub fn box_value_access<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenSnap<'vir, Curr, Next> {
        assert!(self.data.box_data.is_some(), "expected box");
        self.fields.last().unwrap().read(snap)
    }

    /// The snapshot of a `Box` (given as `snap`) with the same address and
    /// other fields, but new pointer metadata and value; used for unsize
    /// coercions. `metadata` and `value` are given in this box's context.
    #[track_caller]
    pub fn box_unsize_snap<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
        metadata: vir::ExprGenSnap<'vir, Curr, Next>,
        value: vir::ExprGenSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenCSnap<'vir, Curr, Next> {
        let box_data = self.data.box_data.expect("expected box");
        // The `Unique` around the same address with the new metadata.
        let metadata = box_data.metadata_caster.cast_to_callee_ctx(metadata);
        let unique = self.data.pure.box_data.unwrap().mk_unique.call()(
            self.box_unique_snap(snap),
            self.box_address_access(snap),
            metadata.downcast_ty(),
        );
        // The box itself: the new `Unique`, the other fields, the new value.
        let mut snaps = vec![unique.upcast_ty()];
        snaps.extend(
            self.fields[1..self.fields.len() - 1]
                .iter()
                .map(|f| f.read(snap)),
        );
        snaps.push(value);
        self.field_snaps_to_snap(snaps)
    }

    pub fn field_snaps_to_snap<Curr, Next>(
        &self,
        mut snaps: Vec<vir::ExprGenSnap<'vir, Curr, Next>>,
    ) -> vir::ExprGenCSnap<'vir, Curr, Next> {
        assert_eq!(snaps.len(), self.fields.len());
        for (snap, field) in snaps.iter_mut().zip(&self.fields) {
            *snap = field.caster.cast_to_callee_ctx(*snap);
        }
        self.pure.field_snaps_to_snap.call()(&snaps)
    }

    /// Like [`Self::field_snaps_to_snap`], for field snapshots already in the
    /// type's own (generic) context.
    pub fn field_snaps_to_snap_generic<Curr, Next>(
        &self,
        snaps: Vec<vir::ExprGenSnap<'vir, Curr, Next>>,
    ) -> vir::ExprGenCSnap<'vir, Curr, Next> {
        assert_eq!(snaps.len(), self.fields.len());
        self.pure.field_snaps_to_snap.call()(&snaps)
    }
}

impl<'vir> TyUsePureField<'vir> {
    /// A value of the field in the type's own (generic) context, in the
    /// caller's context (as [`Self::read`] returns it).
    pub fn generic_to_caller<Curr, Next>(
        &self,
        snap: vir::ExprGenSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenSnap<'vir, Curr, Next> {
        self.caster.cast_to_caller_ctx(snap)
    }

    pub fn read<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenSnap<'vir, Curr, Next> {
        let res = self.pure.read.call()(snap);
        self.caster.cast_to_caller_ctx(res)
    }

    /// Get the (Ref) address of a field. Identical to the function one would
    /// call in `use_impure`.
    #[track_caller]
    pub fn field_ref<Curr, Next>(
        &self,
        self_ref: vir::ExprGenRef<'vir, Curr, Next>,
    ) -> vir::ExprGenRef<'vir, Curr, Next> {
        match self.pure.ref_to_field_ref {
            TyPureFieldRef::Constant(ref_to_field_ref) => {
                ref_to_field_ref.call()(self_ref, self.args.get_ty(), self.args.get_const())
            }
            TyPureFieldRef::Dynamic(_) => {
                panic!("field without a pure address (the value of a box)")
            }
        }
    }
}

impl<'vir> TyUsePureEnum<'vir> {
    pub fn snap_to_discr_snap<Curr, Next>(
        &self,
        snap: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenCSnap<'vir, Curr, Next> {
        self.snap_to_discr_snap.call()(snap)
    }
}

/// Whether `ty` is a pointer as [`pointer_parts`] takes it.
pub(crate) fn is_pointer(mut ty: ty::Ty<'_>) -> bool {
    loop {
        match *ty.kind() {
            ty::TyKind::RawPtr(..) => return true,
            ty::TyKind::Pat(base, _) => ty = base,
            ty::TyKind::Adt(adt, substs)
                if adt.is_struct() && !adt.non_enum_variant().fields.is_empty() =>
            {
                ty = vir::with_vcx(|vcx| {
                    adt.non_enum_variant().fields[abi::FieldIdx::ZERO]
                        .ty(vcx.tcx(), substs)
                        .skip_normalization()
                });
            }
            _ => return false,
        }
    }
}

/// The address and metadata of a pointer of type `ty` (a raw pointer, or
/// structs around one in their first field, as `NonNull`) with snapshot
/// `snap`.
pub(crate) fn pointer_parts<'vir, E: TaskEncoder>(
    deps: &mut task_encoder::TaskEncoderDependencies<'vir, E>,
    params: GParams<'vir>,
    mut ty: ty::Ty<'vir>,
    mut snap: vir::ExprCSnap<'vir>,
) -> Result<(vir::ExprRef<'vir>, vir::ExprSnap<'vir>), EncodeFullError<'vir, E>> {
    while let ty::TyKind::Adt(adt, substs) = *ty.kind() {
        let ty_use =
            deps.require_dep::<TyUsePureEnc>(super::RustTyDecomposition::from_ty(ty, params))?;
        snap = ty_use.expect_structlike()[abi::FieldIdx::ZERO]
            .read(snap)
            .downcast_ty();
        ty = vir::with_vcx(|vcx| {
            adt.non_enum_variant().fields[abi::FieldIdx::ZERO]
                .ty(vcx.tcx(), substs)
                .skip_normalization()
        });
        // A pattern type (e.g. `*const T is !null` in `NonNull`) is encoded as
        // its base type.
        if let ty::TyKind::Pat(base, _) = *ty.kind() {
            ty = base;
        }
    }
    let raw_use =
        deps.require_dep::<TyUsePureEnc>(super::RustTyDecomposition::from_ty(ty, params))?;
    let raw = raw_use.expect_raw();
    Ok((raw.address_access(snap), raw.metadata_access(snap)))
}

/// Walks the (possibly private) field path `path` from a value of type `ty`
/// with snapshot `snap` at address `addr`: the type, snapshot and address of
/// the final field, and the pointer metadata of the last `*FIELD` step (the
/// pointee held by a `#[ptr_holds]` struct), if any. The inner error
/// describes a malformed path.
#[allow(clippy::type_complexity)]
pub(crate) fn project_field_path<'vir, E: TaskEncoder>(
    vcx: &'vir vir::VirCtxt<'vir>,
    deps: &mut task_encoder::TaskEncoderDependencies<'vir, E>,
    params: GParams<'vir>,
    path: &[String],
    mut ty: ty::Ty<'vir>,
    mut snap: vir::ExprSnap<'vir>,
    mut addr: vir::ExprRef<'vir>,
) -> Result<
    Result<
        (
            ty::Ty<'vir>,
            vir::ExprSnap<'vir>,
            vir::ExprRef<'vir>,
            Option<vir::ExprSnap<'vir>>,
        ),
        String,
    >,
    EncodeFullError<'vir, E>,
> {
    let mut metadata = None;
    for name in path {
        let ty::TyKind::Adt(adt, substs) = *ty.kind() else {
            return Ok(Err(format!("`{ty}` has no field `{name}`")));
        };
        if !adt.is_struct() {
            return Ok(Err(format!("`{ty}` is not a struct")));
        }
        // `@shares`: the shared value of an `#[im_shares]` struct.
        if name == "@shares" {
            let Some(shared_ty) = crate::encoders::ty::im::shares_slot_ty(adt.did()) else {
                return Ok(Err(format!("`{ty}` shares nothing")));
            };
            let decomp = super::RustTyDecomposition::from_ty(ty, params);
            let idx = decomp
                .ty
                .expect_structlike()
                .fields
                .iter()
                .position(|f| f.address == super::RustFieldAddress::Slot)
                .unwrap();
            let ty_use = deps.require_dep::<TyUsePureEnc>(decomp)?;
            let proj = ty_use.expect_variant_opt(None).fields[idx];
            snap = proj.read(snap.downcast_ty());
            addr = proj.field_ref(addr);
            ty = ty::EarlyBinder::bind(vcx.tcx(), shared_ty)
                .instantiate(vcx.tcx(), substs)
                .skip_normalization();
            continue;
        }
        // `*FIELD`: the pointee held by a `#[ptr_holds]` struct.
        if let Some(ptr_name) = name.strip_prefix('*') {
            let decomp = super::RustTyDecomposition::from_ty(ty, params);
            let held = crate::encoders::ty::im::ptr_holds_field(adt.did())
                .filter(|(ptr, _)| adt.non_enum_variant().fields[*ptr].name.as_str() == ptr_name);
            let Some((ptr, pointee)) = held else {
                return Ok(Err(format!("`{ty}` holds no pointee of `{ptr_name}`")));
            };
            let ty_use = deps.require_dep::<TyUsePureEnc>(decomp)?;
            let variant = ty_use.expect_variant_opt(None);
            let held_field = variant.fields.last().unwrap();
            let TyPureFieldRef::Dynamic(address_access) = held_field.pure.ref_to_field_ref else {
                unreachable!()
            };
            let ptr_snap = variant[ptr].read(snap.downcast_ty()).downcast_ty();
            addr = address_access.call()(ptr_snap);
            let ptr_ty = adt.non_enum_variant().fields[ptr]
                .ty(vcx.tcx(), substs)
                .skip_normalization();
            if let Some((_, Some(sub), _)) = crate::encoders::ty::im::ptr_holds(adt.did()) {
                addr = super::kinds::r#box::held_sub_address(
                    deps,
                    super::RustTyDecomposition::from_ty(ptr_ty, params),
                    sub,
                    addr,
                )?;
            }
            metadata = Some(pointer_parts(deps, params, ptr_ty, ptr_snap)?.1);
            snap = held_field.read(snap.downcast_ty());
            ty = ty::EarlyBinder::bind(vcx.tcx(), pointee)
                .instantiate(vcx.tcx(), substs)
                .skip_normalization();
            continue;
        }
        let Some((idx, field)) = adt
            .non_enum_variant()
            .fields
            .iter_enumerated()
            .find(|(_, field)| field.name.as_str() == name)
        else {
            return Ok(Err(format!("`{ty}` has no field `{name}`")));
        };
        let ty_use =
            deps.require_dep::<TyUsePureEnc>(super::RustTyDecomposition::from_ty(ty, params))?;
        let proj = ty_use.expect_variant_opt(None)[idx];
        snap = proj.read(snap.downcast_ty());
        addr = proj.field_ref(addr);
        ty = field.ty(vcx.tcx(), substs).skip_normalization();
        // A pattern type (e.g. `*const T is !null` in `NonNull`) is encoded as
        // its base type.
        if let ty::TyKind::Pat(base, _) = *ty.kind() {
            ty = base;
        }
    }
    Ok(Ok((ty, snap, addr, metadata)))
}

/// The parts of a value of the struct `ty` that an aggregate of its fields'
/// snapshots `fields` does not give: an `#[im_shares]` struct's slot
/// (arbitrary, as in shallow snapshots), and a `#[ptr_holds]` struct's held
/// pointee, read at the address its pointer field gives (where the amount
/// held is positive, as in the struct's snapshot function), with the
/// permission to it that the value holds (taken from the state).
pub(crate) struct HolderParts<'vir> {
    pub(crate) slot: Option<vir::ExprSnap<'vir>>,
    pub(crate) held: Option<(vir::ExprSnap<'vir>, vir::ExprBool<'vir>)>,
}

/// Whether an aggregate of the type `ty` with `operands` fields lacks
/// [`HolderParts`].
pub(crate) fn has_holder_parts(ty: super::RustTyDecomposition<'_>, operands: usize) -> bool {
    !matches!(ty.ty.special, RustTySpecial::Box)
        && ty
            .ty
            .get_structlike()
            .is_some_and(|data| data.fields.len() > operands)
}

pub(crate) fn holder_parts<'vir, E: TaskEncoder>(
    deps: &mut task_encoder::TaskEncoderDependencies<'vir, E>,
    ty: super::RustTyDecomposition<'vir>,
    fields: &[vir::ExprSnap<'vir>],
) -> Result<HolderParts<'vir>, EncodeFullError<'vir, E>> {
    let data = ty.ty.expect_structlike();
    let ty_use = deps.require_dep::<TyUsePureEnc>(ty)?;
    let variant = ty_use.expect_variant_opt(None);
    let mut parts = HolderParts {
        slot: None,
        held: None,
    };
    for (idx, field) in data.fields.iter().enumerate().skip(fields.len()) {
        let field_ty = field.decompose_normalize(ty.args);
        match field.address {
            super::RustFieldAddress::Slot => {
                parts.slot = Some(deps.require_ref::<TyUsePureEnc>(field_ty)?.arbitrary_to_snap());
            }
            super::RustFieldAddress::Dynamic => {
                let RustTySpecial::Holds(did, ptr) = ty.ty.special else {
                    unreachable!("a dynamic field of a struct that is no `Box` is a held pointee")
                };
                let TyPureFieldRef::Dynamic(address_access) = variant.fields[idx].pure.ref_to_field_ref
                else {
                    unreachable!()
                };
                let mut addr = address_access.call()(fields[ptr.as_usize()].downcast_ty());
                if let Some((_, Some(sub), _)) = crate::encoders::ty::im::ptr_holds(did) {
                    let ptr_ty = data.fields[ptr.as_usize()].decompose_normalize(ty.args);
                    addr = super::kinds::r#box::held_sub_address(deps, ptr_ty, sub, addr)?;
                }
                let amount = crate::encoders::ty::im::holds_amount_of(deps, did, ty)?;
                let held_use = deps.require_dep::<crate::encoders::TyUseImpureEnc>(field_ty)?;
                let arbitrary = deps.require_ref::<TyUsePureEnc>(field_ty)?.arbitrary_to_snap();
                let snap = vir::with_vcx(|vcx| {
                    let positive = vcx
                        .mk_bin_op_expr(vir::BinOpKind::CmpGt, amount, vcx.mk_no_perm())
                        .downcast_ty();
                    vcx.mk_ternary_expr(positive, held_use.ref_to_snap(addr), arbitrary)
                });
                let acc = vir::with_vcx(|vcx| held_use.ref_to_pred(vcx, addr, Some(amount)));
                parts.held = Some((snap, acc));
            }
            super::RustFieldAddress::Constant => unreachable!("a struct's own fields come first"),
        }
    }
    Ok(parts)
}
