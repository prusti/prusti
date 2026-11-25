//! Rvalues.

use super::*;

impl<'vir: 'enc, 'enc> Enc<'vir, 'enc> {
    pub(super) fn encode_rvalue(
        &mut self,
        curr_ver: &Versions<'vir>,
        rvalue: &mir::Rvalue<'vir>,
        span: Span,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        let rvalue_ty = rvalue.ty(self.body, self.vcx.tcx());
        match rvalue {
            mir::Rvalue::Use(op, _) => self.encode_operand_snap(op, curr_ver),
            mir::Rvalue::Ref(_, kind, place) => {
                // Reading a mutable reference's referent needs an address
                // holding a predicate, which pure-created mutrefs don't have.
                if kind.mutability().is_mut() {
                    return Err(EncodeFullError::unsupported(
                        format!(
                            "mutable borrow of `{}` in a specification: take a shared \
                             reference (`&`) instead",
                            place.ty(self.body, self.vcx.tcx()).ty
                        ),
                        vec![self.current_span()],
                    ));
                }
                let (value, place_ref, metadata) =
                    self.encode_borrowed_place(curr_ver, (*place).into(), rvalue_ty)?;
                let e_rvalue_ty = self.ty_use(rvalue_ty)?.expect_immref();
                Ok(e_rvalue_ty
                    .prim_to_snap(place_ref, metadata, value)
                    .upcast_ty())
            }
            mir::Rvalue::BinaryOp(op, box (l, r)) => {
                self.encode_binop_snap(rvalue_ty, *op, l, r, curr_ver, span)
            }
            mir::Rvalue::UnaryOp(unop, operand) => {
                self.encode_unary_op_snap(rvalue_ty, *unop, operand, curr_ver)
            }
            mir::Rvalue::Aggregate(
                box kind @ (mir::AggregateKind::Adt(..)
                | mir::AggregateKind::Tuple
                | mir::AggregateKind::Closure(..)),
                fields,
            ) => self.encode_aggregate_snap(rvalue_ty, kind, fields, curr_ver),
            mir::Rvalue::Aggregate(box mir::AggregateKind::Array(..), elements) => {
                self.encode_array_snap(rvalue_ty, elements, curr_ver)
            }
            mir::Rvalue::Repeat(operand, len) => {
                self.encode_repeat_snap(rvalue_ty, operand, *len, curr_ver)
            }
            mir::Rvalue::Discriminant(place) => {
                let place_ty = place.ty(self.body, self.vcx.tcx());
                let ty = self.ty_use(place_ty.ty)?;
                if ty.specifics.is_opaque() {
                    return Err(unsupported_opaque(place_ty.ty, span));
                }
                let discr = match ty
                    .get_enumlike()
                    .filter(|_| place_ty.variant_index.is_none())
                {
                    Some(ty) => ty.snap_to_discr_snap(
                        self.encode_place_snap((*place).into(), curr_ver)?
                            .downcast_ty(),
                    ),
                    None => {
                        let e_rvalue_ty = self.ty_use(rvalue_ty)?.expect_primitive();
                        // mir::Rvalue::Discriminant documents "Returns zero for types without discriminant"
                        let zero = self.vcx.mk_uint::<0>();
                        e_rvalue_ty.prim_to_snap(zero.upcast_ty()).lazy()
                    }
                };
                Ok(discr.upcast_ty())
            }
            mir::Rvalue::Cast(kind, operand, ty) => {
                assert_eq!(*ty, rvalue_ty);
                // `encode_cast_snap` also returns a statement, which moves the
                // referent's predicate in a `&mut` unsize coercion. Pure code
                // holds no predicates, so only the value (`.1`) is used.
                Ok(self
                    .encode_cast_snap(rvalue_ty, *kind, operand, curr_ver)?
                    .1)
            }
            mir::Rvalue::RawPtr(_, place) => {
                let (_, place_ref, metadata) =
                    self.encode_borrowed_place(curr_ver, (*place).into(), rvalue_ty)?;
                let raw_ty = self.ty_use(rvalue_ty)?;
                Ok(raw_ty
                    .expect_raw()
                    .prim_to_snap(place_ref, metadata)
                    .upcast_ty())
            }
            _ => Err(EncodeFullError::unsupported(
                format!("unsupported rvalue {rvalue:?} in pure code"),
                vec![span],
            )),
        }
    }
}
