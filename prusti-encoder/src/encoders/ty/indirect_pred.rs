use task_encoder::{EncodeFullError, EncodeFullResult, TaskEncoder, TaskEncoderDependencies};
use vir::{CastType, FunctionIdn, PredicateIdn};

use crate::encoders::TyUseImpureEnc;

use super::{
    RustTy, RustTyDecomposition,
    data::TySpecifics,
    generics::{
        GArgsTyEnc, GenericParamsEnc,
        arity::{TyArityEnc, add, sub},
        clamp::ClampEnc,
        lifetime_position::{
            EnclosureGraph, GeneralizedLifetime, LifetimePosition, LifetimePositionEnc,
        },
    },
    impure::TyImpureEnc,
    pure::TyPureEnc,
};

pub struct IndirectPredEnc;

#[derive(Debug, Clone, Copy)]
pub struct IndirectPredRef<'vir> {
    pub predicate: PredicateIdn<
        'vir,
        (
            vir::Snap,
            vir::ManyTyVal,
            vir::ManyCSnap,
            vir::Int,
            vir::Int,
        ),
    >,
    /// `extend_X(k, T.., lo, hi)`: `k` extended by the positions `[lo, hi)`: the references
    /// there hold their current referents, read through `p_X_ind(k, T.., lo, hi)`.
    pub extend: FunctionIdn<
        'vir,
        (
            vir::Snap,
            vir::ManyTyVal,
            vir::ManyCSnap,
            vir::Int,
            vir::Int,
        ),
        vir::Snap,
    >,
    /// `graft_X(s, T.., t, lo, hi)`: `s` with the references at `[lo, hi)` holding `t`'s
    /// referents, in which the references at other positions are collapsed.
    pub graft: FunctionIdn<
        'vir,
        (
            vir::Snap,
            vir::ManyTyVal,
            vir::ManyCSnap,
            vir::Snap,
            vir::Int,
            vir::Int,
        ),
        vir::Snap,
    >,
    /// `shallow_X(s, T..)`: `s` with every reference collapsed, the shallow snapshot.
    pub shallow: FunctionIdn<'vir, (vir::Snap, vir::ManyTyVal, vir::ManyCSnap), vir::Snap>,
}

impl<'vir> task_encoder::OutputRefAny for IndirectPredRef<'vir> {}

impl<'vir> IndirectPredRef<'vir> {
    /// `snap` with the positions `[lo, hi)` holding what extending `key` by them reads
    /// through the predicate on `key`.
    pub fn extend_from(
        &self,
        snap: vir::ExprSnap<'vir>,
        key: vir::ExprSnap<'vir>,
        tys: &'vir [vir::ExprTyVal<'vir>],
        consts: &'vir [vir::ExprCSnap<'vir>],
        lo: vir::ExprInt<'vir>,
        hi: vir::ExprInt<'vir>,
    ) -> vir::ExprSnap<'vir> {
        if vir::simplify::syntactic_eq(snap.as_dyn(), key.as_dyn()) {
            self.extend.call()(snap, tys, consts, lo, hi)
        } else {
            let extended = self.extend.call()(key, tys, consts, lo, hi);
            self.graft.call()(snap, tys, consts, extended, lo, hi)
        }
    }
}

/// The positions `[first, end)` of an instantiated type, merged on the
/// snapshot `k`.
pub(crate) struct LifetimePositionBlock<'vir, 'a> {
    pub(crate) graph: &'a EnclosureGraph<'vir>,
    pub(crate) first: LifetimePosition,
    pub(crate) end: LifetimePosition,
    pub(crate) ind: IndirectPredRef<'vir>,
    pub(crate) tys: &'vir [vir::ExprTyVal<'vir>],
    pub(crate) consts: &'vir [vir::ExprCSnap<'vir>],
    pub(crate) k: vir::ExprSnap<'vir>,
}

impl<'vir> LifetimePositionBlock<'vir, '_> {
    fn contains(&self, p: LifetimePosition) -> bool {
        self.first <= p && p < self.end
    }

    /// The positions of the block, each after the positions enclosing it.
    pub(crate) fn positions(&self) -> Vec<LifetimePosition> {
        self.graph
            .positions_outermost_first()
            .filter(|&p| self.contains(p))
            .collect()
    }

    /// The key of `p`'s predicate: `k` with what the positions of the block
    /// enclosing `p` read grafted in, as in the body of the merged predicate.
    pub(crate) fn key(&self, p: LifetimePosition) -> vir::ExprSnap<'vir> {
        let ancestors = self
            .graph
            .ancestors(p)
            .iter()
            .copied()
            .filter(|&q| self.contains(q))
            .collect::<Vec<_>>();
        self.fold(&ancestors)
    }

    /// `k` with the data at the positions of the block enclosing `p` taken
    /// from `from`, another key of `p`.
    pub(crate) fn graft_enclosing(
        &self,
        p: LifetimePosition,
        from: vir::ExprSnap<'vir>,
    ) -> vir::ExprSnap<'vir> {
        let mut snap = self.k;
        for &q in self
            .graph
            .ancestors(p)
            .iter()
            .filter(|&&q| self.contains(q))
        {
            let (lo, hi) = self.graph.interval(q);
            snap = self.ind.graft.call()(snap, self.tys, self.consts, from, lo, hi);
        }
        snap
    }

    /// `k` with what each of `positions` reads through its key grafted in, in
    /// turn.
    pub(crate) fn fold(&self, positions: &[LifetimePosition]) -> vir::ExprSnap<'vir> {
        let mut snap = self.k;
        for &q in positions {
            let (lo, hi) = self.graph.interval(q);
            snap = self
                .ind
                .extend_from(snap, self.key(q), self.tys, self.consts, lo, hi);
        }
        snap
    }

    pub(crate) fn predicate(
        &self,
        key: vir::ExprSnap<'vir>,
        lo: vir::ExprInt<'vir>,
        hi: vir::ExprInt<'vir>,
    ) -> vir::ExprBool<'vir> {
        let pred = self.ind.predicate;
        let (tys, consts) = (self.tys, self.consts);
        vir::expr! { acc([pred](key, [..[tys]], [..[consts]], lo, hi)) }
    }
}

/// The part of a query `[lo, hi)` at one own position.
#[derive(Clone, Copy)]
struct Subquery<'vir> {
    lifetime: GeneralizedLifetime<'vir>,
    /// The lifetimes of the own positions enclosing this one.
    enclosing: &'vir [GeneralizedLifetime<'vir>],
    /// Whether the query contains part of this position.
    guard: vir::ExprBool<'vir>,
    /// The query.
    lo: vir::ExprInt<'vir>,
    hi: vir::ExprInt<'vir>,
    /// The position covers `[x, x + width)` of the type.
    x: vir::ExprInt<'vir>,
    width: vir::ExprInt<'vir>,
    clamp: FunctionIdn<'vir, (vir::Int, vir::Int, vir::Int), vir::Int>,
}

impl<'vir> Subquery<'vir> {
    /// The part of the query at this position, numbered as in a field type in
    /// which the position starts at `x_f`: empty if `guard` is false.
    fn in_field(self, x_f: vir::ExprInt<'vir>) -> (vir::ExprInt<'vir>, vir::ExprInt<'vir>) {
        let shift = sub(self.x, x_f);
        let end = add(x_f, self.width);
        (
            self.clamp.call()(sub(self.lo, shift), x_f, end),
            self.clamp.call()(sub(self.hi, shift), x_f, end),
        )
    }

    /// Like `in_field`, where `guard` holds: the whole position for an own lifetime.
    fn guarded_in_field(self, x_f: vir::ExprInt<'vir>) -> (vir::ExprInt<'vir>, vir::ExprInt<'vir>) {
        match self.lifetime {
            GeneralizedLifetime::Region(_) => (x_f, add(x_f, self.width)),
            GeneralizedLifetime::RegionsIn(_) => self.in_field(x_f),
        }
    }
}

/// A field of a struct or of an enum variant: its type and its reader.
type Field<'vir> = (
    RustTyDecomposition<'vir>,
    vir::AdtDestructor<'vir, vir::CSnap, vir::Snap>,
);

impl IndirectPredEnc {
    /// The subqueries of the query `[lo, hi)` for lifetime and type parameters of `ty`.
    fn subqueries<'vir>(
        ty: RustTy<'vir>,
        lo: vir::ExprInt<'vir>,
        hi: vir::ExprInt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> Result<Vec<Subquery<'vir>>, EncodeFullError<'vir, Self>> {
        let clamp = deps.require_dep::<ClampEnc>(())?;
        let graph = deps.require_dep::<LifetimePositionEnc>(RustTyDecomposition::identity(ty))?;
        vir::with_vcx(|vcx| {
            // A position's predicate is on its key extended by the positions enclosing it,
            // so their predicates and extensions come first.
            graph
                .positions_outermost_first()
                .map(|p| {
                    let lifetime = graph.lifetime(p);
                    let enclosing = graph
                        .ancestors(p)
                        .iter()
                        .map(|&q| graph.lifetime(q))
                        .collect::<Vec<_>>();
                    let enclosing = vcx.alloc_slice(&enclosing);
                    let (x, _) = graph.interval(p);
                    let width = match lifetime {
                        // An own lifetime is a single position, either in the query or not.
                        GeneralizedLifetime::Region(_) => vcx.mk_uint::<1>(),
                        GeneralizedLifetime::RegionsIn(ty) => deps.require_dep::<TyArityEnc>(ty)?,
                    };
                    let mut subquery = Subquery {
                        lifetime,
                        enclosing,
                        guard: vcx.mk_bool::<true>(),
                        lo,
                        hi,
                        x,
                        width,
                        clamp,
                    };
                    subquery.guard = match lifetime {
                        GeneralizedLifetime::Region(_) => {
                            vir::expr! { vcx; ((lo) <= (x)) && ((x) < (hi)) }
                        }
                        GeneralizedLifetime::RegionsIn(_) => {
                            let (start, end) = subquery.in_field(x);
                            vir::expr! { vcx; (start) < (end) }
                        }
                    };
                    Ok(subquery)
                })
                .collect()
        })
    }

    /// The snapshot of the field predicate of `p` for `subquery`: `field_snap`, restricted,
    /// then extended by `p`'s ancestors in the query; the caller extended it by the others.
    fn position_snap<'vir>(
        subqueries: &[Subquery<'vir>],
        subquery: Subquery<'vir>,
        field_ty: RustTyDecomposition<'vir>,
        graph: &EnclosureGraph<'vir>,
        p: LifetimePosition,
        field_snap: vir::ExprSnap<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> Result<vir::ExprSnap<'vir>, EncodeFullError<'vir, Self>> {
        let mut snap = Self::restricted_snap(subquery, field_ty, graph, p, field_snap, deps)?;
        for &enclosing in graph.ancestors(p) {
            let lifetime = graph.lifetime(enclosing);
            let Some(&outer) = subqueries.iter().find(|s| s.lifetime == lifetime) else {
                continue;
            };
            let key = Self::position_snap(
                subqueries, outer, field_ty, graph, enclosing, field_snap, deps,
            )?;
            let (start, end) = outer.in_field(graph.interval(enclosing).0);
            snap = Self::extend_from(field_ty, snap, key, start, end, deps)?;
        }
        Ok(snap)
    }

    /// `snap` extended by the positions `[start, end)` of `field_ty`, whose permissions are
    /// on `key`.
    fn extend_from<'vir>(
        field_ty: RustTyDecomposition<'vir>,
        snap: vir::ExprSnap<'vir>,
        key: vir::ExprSnap<'vir>,
        start: vir::ExprInt<'vir>,
        end: vir::ExprInt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> Result<vir::ExprSnap<'vir>, EncodeFullError<'vir, Self>> {
        let field_ind = deps.require_ref::<IndirectPredEnc>(field_ty.ty)?;
        let args = deps.require_dep::<GArgsTyEnc>(field_ty.args)?;
        Ok(field_ind.extend_from(snap, key, args.get_ty(), args.get_const(), start, end))
    }

    /// `field_snap` restricted to `p`'s ancestors in `field_ty` and, for a type parameter,
    /// all of `p`'s block, if the container may have extended other positions of
    /// `field_ty`: the field made shallow, with those positions grafted back in from
    /// `field_snap`. Which positions of a type parameter enclose the subquery is up to its
    /// instantiation, so its block keeps what the container's key has there.
    fn restricted_snap<'vir>(
        subquery: Subquery<'vir>,
        field_ty: RustTyDecomposition<'vir>,
        graph: &EnclosureGraph<'vir>,
        p: LifetimePosition,
        field_snap: vir::ExprSnap<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> Result<vir::ExprSnap<'vir>, EncodeFullError<'vir, Self>> {
        let spurious = graph.positions().any(|q| {
            subquery.enclosing.contains(&graph.lifetime(q)) && !graph.ancestors(p).contains(&q)
        });
        if !spurious {
            return Ok(field_snap);
        }
        let field_ind = deps.require_ref::<IndirectPredEnc>(field_ty.ty)?;
        let args = deps.require_dep::<GArgsTyEnc>(field_ty.args)?;
        let mut keep = graph
            .ancestors(p)
            .iter()
            .map(|&q| graph.interval(q))
            .collect::<Vec<_>>();
        if let GeneralizedLifetime::RegionsIn(_) = graph.lifetime(p) {
            keep.push(graph.interval(p));
        }
        let mut snap = field_ind.shallow.call()(field_snap, args.get_ty(), args.get_const());
        for (lo, hi) in keep {
            snap =
                field_ind.graft.call()(snap, args.get_ty(), args.get_const(), field_snap, lo, hi);
        }
        Ok(snap)
    }

    /// `s_field` with the positions of `subquery` in `field_ty` grafted in from `t_field`;
    /// unchanged where the subquery is empty, since grafting nothing changes nothing.
    fn field_graft<'vir>(
        subquery: Subquery<'vir>,
        field_ty: RustTyDecomposition<'vir>,
        s_field: vir::ExprSnap<'vir>,
        t_field: vir::ExprSnap<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> Result<vir::ExprSnap<'vir>, EncodeFullError<'vir, Self>> {
        let graph = deps.require_dep::<LifetimePositionEnc>(field_ty)?;
        if graph
            .positions()
            .all(|p| graph.lifetime(p) != subquery.lifetime)
        {
            return Ok(s_field);
        }
        let graft = deps.require_ref::<IndirectPredEnc>(field_ty.ty)?.graft;
        let args = deps.require_dep::<GArgsTyEnc>(field_ty.args)?;
        let mut snap = s_field;
        for p in graph.positions() {
            if graph.lifetime(p) != subquery.lifetime {
                continue;
            }
            let (start, end) = subquery.in_field(graph.interval(p).0);
            snap = graft.call()(snap, args.get_ty(), args.get_const(), t_field, start, end);
        }
        Ok(snap)
    }

    /// The fields `fields`, given by their types and readers, of `s`, with all
    /// `subqueries` grafted in from the fields of `t`.
    fn fields_graft<'vir>(
        subqueries: &[Subquery<'vir>],
        fields: &[Field<'vir>],
        s: vir::ExprSnap<'vir>,
        t: vir::ExprSnap<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> Result<Vec<vir::ExprSnap<'vir>>, EncodeFullError<'vir, Self>> {
        let mut field_snaps = Vec::new();
        for &(field_ty, read) in fields {
            let mut field_snap = read.call()(s.downcast_ty());
            let t_field = read.call()(t.downcast_ty());
            for &subquery in subqueries {
                field_snap = Self::field_graft(subquery, field_ty, field_snap, t_field, deps)?;
            }
            field_snaps.push(field_snap);
        }
        Ok(field_snaps)
    }

    /// The fields `fields`, given by their types and readers, of `s`, each made shallow.
    fn fields_shallow<'vir>(
        fields: &[Field<'vir>],
        s: vir::ExprSnap<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> Result<Vec<vir::ExprSnap<'vir>>, EncodeFullError<'vir, Self>> {
        let mut field_snaps = Vec::new();
        for &(field_ty, read) in fields {
            let field_snap = read.call()(s.downcast_ty());
            field_snaps.push(Self::field_shallow(field_ty, field_snap, deps)?);
        }
        Ok(field_snaps)
    }

    /// `field_snap` of type `field_ty` made shallow.
    fn field_shallow<'vir>(
        field_ty: RustTyDecomposition<'vir>,
        field_snap: vir::ExprSnap<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> Result<vir::ExprSnap<'vir>, EncodeFullError<'vir, Self>> {
        if deps
            .require_dep::<LifetimePositionEnc>(field_ty)?
            .is_empty()
        {
            return Ok(field_snap);
        }
        let shallow = deps.require_ref::<IndirectPredEnc>(field_ty.ty)?.shallow;
        let args = deps.require_dep::<GArgsTyEnc>(field_ty.args)?;
        Ok(shallow.call()(field_snap, args.get_ty(), args.get_const()))
    }

    /// The field predicates for `subquery`: one per position of its lifetime in `field_ty`.
    fn field_preds<'vir>(
        subqueries: &[Subquery<'vir>],
        subquery: Subquery<'vir>,
        field_ty: RustTyDecomposition<'vir>,
        field_snap: vir::ExprSnap<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> Result<Vec<vir::ExprBool<'vir>>, EncodeFullError<'vir, Self>> {
        let graph = deps.require_dep::<LifetimePositionEnc>(field_ty)?;
        let mut preds = Vec::new();
        for p in graph.positions_outermost_first() {
            if graph.lifetime(p) != subquery.lifetime {
                continue;
            }
            let pred = deps.require_ref::<IndirectPredEnc>(field_ty.ty)?.predicate;
            let args = deps.require_dep::<GArgsTyEnc>(field_ty.args)?;
            let snap =
                Self::position_snap(subqueries, subquery, field_ty, &graph, p, field_snap, deps)?;
            let (start, end) = subquery.guarded_in_field(graph.interval(p).0);
            preds.push(vir::expr! {
                acc([pred](snap, [..[args.get_ty()]], [..[args.get_const()]], start, end))
            });
        }
        Ok(preds)
    }

    /// `field_snap` extended by `subquery` at each position of its lifetime in `field_ty`;
    /// the field predicates are on `field_pred`.
    fn field_extend<'vir>(
        subqueries: &[Subquery<'vir>],
        subquery: Subquery<'vir>,
        field_ty: RustTyDecomposition<'vir>,
        field_snap: vir::ExprSnap<'vir>,
        field_pred: vir::ExprSnap<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> Result<vir::ExprSnap<'vir>, EncodeFullError<'vir, Self>> {
        let graph = deps.require_dep::<LifetimePositionEnc>(field_ty)?;
        if graph
            .positions()
            .all(|p| graph.lifetime(p) != subquery.lifetime)
        {
            return Ok(field_snap);
        }
        let mut snap = field_snap;
        for p in graph.positions_outermost_first() {
            if graph.lifetime(p) != subquery.lifetime {
                continue;
            }
            let key =
                Self::position_snap(subqueries, subquery, field_ty, &graph, p, field_pred, deps)?;
            let (start, end) = subquery.in_field(graph.interval(p).0);
            snap = Self::extend_from(field_ty, snap, key, start, end, deps)?;
        }
        Ok(snap)
    }

    /// The field predicates of the fields `fields`, given by their types and
    /// readers, of the snapshot `k` for `subquery`.
    fn fields_preds<'vir>(
        subqueries: &[Subquery<'vir>],
        subquery: Subquery<'vir>,
        fields: &[Field<'vir>],
        k: vir::ExprSnap<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> Result<Vec<vir::ExprBool<'vir>>, EncodeFullError<'vir, Self>> {
        let mut preds = Vec::new();
        for &(field_ty, read) in fields {
            let field_snap = read.call()(k.downcast_ty());
            preds.extend(Self::field_preds(
                subqueries, subquery, field_ty, field_snap, deps,
            )?);
        }
        Ok(preds)
    }

    /// The fields `fields`, given by their types and readers, of `k`, each
    /// extended by all `subqueries`, whose field predicates are on the fields
    /// of `k`.
    fn fields_extend<'vir>(
        subqueries: &[Subquery<'vir>],
        fields: &[Field<'vir>],
        k: vir::ExprSnap<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> Result<Vec<vir::ExprSnap<'vir>>, EncodeFullError<'vir, Self>> {
        let mut field_snaps = Vec::new();
        for &(field_ty, read) in fields {
            let field_pred = read.call()(k.downcast_ty());
            let mut field_snap = field_pred;
            for &subquery in subqueries {
                field_snap = Self::field_extend(
                    subqueries, subquery, field_ty, field_snap, field_pred, deps,
                )?;
            }
            field_snaps.push(field_snap);
        }
        Ok(field_snaps)
    }
}

impl TaskEncoder for IndirectPredEnc {
    task_encoder::encoder_cache!(IndirectPredEnc);
    const ENCODER_NAME: &'static str = "indirect predicate encoder";

    type TaskDescription<'vir> = RustTy<'vir>;

    type OutputRef<'vir> = IndirectPredRef<'vir>;
    type OutputFullLocal<'vir> = (
        vir::Predicate<'vir>,
        vir::Function<'vir>,
        vir::Function<'vir>,
        vir::Function<'vir>,
    );

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        let pure = deps.require_dep::<TyPureEnc>(*task_key)?;
        let params = deps.require_dep::<GenericParamsEnc>(task_key.params)?;
        let ty = task_key.zip(pure);
        vir::with_vcx(|vcx| {
            let snapshot = pure.data.snapshot;
            let predicate = PredicateIdn::new(
                vir::ViperIdent::new(vir::vir_format!(vcx, "p_{}_ind", task_key.name())),
                (
                    snapshot,
                    params.ty_args(),
                    params.const_args(),
                    vir::TYPE_INT,
                    vir::TYPE_INT,
                ),
            );
            let extend = FunctionIdn::new(
                vir::ViperIdent::new(vir::vir_format!(vcx, "extend_{}", task_key.name())),
                (
                    snapshot,
                    params.ty_args(),
                    params.const_args(),
                    vir::TYPE_INT,
                    vir::TYPE_INT,
                ),
                snapshot,
            );
            let graft = FunctionIdn::new(
                vir::ViperIdent::new(vir::vir_format!(vcx, "graft_{}", task_key.name())),
                (
                    snapshot,
                    params.ty_args(),
                    params.const_args(),
                    snapshot,
                    vir::TYPE_INT,
                    vir::TYPE_INT,
                ),
                snapshot,
            );
            let shallow = FunctionIdn::new(
                vir::ViperIdent::new(vir::vir_format!(vcx, "shallow_{}", task_key.name())),
                (snapshot, params.ty_args(), params.const_args()),
                snapshot,
            );
            deps.emit_output_ref(
                *task_key,
                IndirectPredRef {
                    predicate,
                    extend,
                    graft,
                    shallow,
                },
            )?;

            let k_decl = vcx.mk_local_decl("k", snapshot);
            let lo_decl = vcx.mk_local_decl("lo", vir::TYPE_INT);
            let hi_decl = vcx.mk_local_decl("hi", vir::TYPE_INT);
            let k: vir::ExprSnap<'vir> = vcx.mk_local_ex(k_decl);
            let lo = vcx.mk_local_ex(lo_decl);
            let hi = vcx.mk_local_ex(hi_decl);
            // The snapshot arguments of `graft` and `shallow`.
            let s_decl = vcx.mk_local_decl("s", snapshot);
            let t_decl = vcx.mk_local_decl("t", snapshot);
            let s: vir::ExprSnap<'vir> = vcx.mk_local_ex(s_decl);
            let t: vir::ExprSnap<'vir> = vcx.mk_local_ex(t_decl);

            let (body, extend_body, graft_body, shallow_body) = match &ty.specifics {
                TySpecifics::StructLike(data) => {
                    let subqueries = Self::subqueries(task_key, lo, hi, deps)?;
                    let fields = data
                        .fields
                        .iter()
                        .map(|(field, pure_field)| {
                            (field.decompose(task_key.params), pure_field.read)
                        })
                        .collect::<Vec<_>>();
                    let mut conjuncts = Vec::new();
                    for &subquery in &subqueries {
                        let preds = Self::fields_preds(&subqueries, subquery, &fields, k, deps)?;
                        if !preds.is_empty() {
                            let guard = subquery.guard;
                            let preds = vcx.mk_conj(&preds);
                            conjuncts.push(vir::expr! { vcx; (guard) ==> (preds) });
                        }
                    }
                    let cons = data.data.1.field_snaps_to_snap;
                    let field_snaps = Self::fields_extend(&subqueries, &fields, k, deps)?;
                    let extend_body = cons.call()(&field_snaps).upcast_ty();
                    let field_snaps = Self::fields_graft(&subqueries, &fields, s, t, deps)?;
                    let graft_body = cons.call()(&field_snaps).upcast_ty();
                    let field_snaps = Self::fields_shallow(&fields, s, deps)?;
                    let shallow_body = cons.call()(&field_snaps).upcast_ty();
                    (
                        Some(vcx.mk_conj(&conjuncts)),
                        Some(extend_body),
                        Some(graft_body),
                        Some(shallow_body),
                    )
                }
                TySpecifics::EnumLike(data) => {
                    let subqueries = Self::subqueries(task_key, lo, hi, deps)?;
                    let discr = data.data.1.snap_to_discr_snap;
                    // The variants with lifetime positions.
                    let mut variants = Vec::new();
                    for variant in &data.variants {
                        let fields = variant
                            .inner
                            .fields
                            .iter()
                            .map(|(field, pure_field)| {
                                (field.decompose(task_key.params), pure_field.read)
                            })
                            .collect::<Vec<_>>();
                        let mut has_positions = false;
                        for &(field_ty, _) in &fields {
                            has_positions |= !deps
                                .require_dep::<LifetimePositionEnc>(field_ty)?
                                .is_empty();
                        }
                        if has_positions {
                            let cons = variant.inner.data.1.field_snaps_to_snap;
                            variants.push((variant.data.1.discr, fields, cons));
                        }
                    }
                    // The field predicates of each variant are guarded by its
                    // discriminant.
                    let mut conjuncts = Vec::new();
                    for &subquery in &subqueries {
                        let mut preds = Vec::new();
                        for (variant_discr, fields, _) in &variants {
                            let variant_preds =
                                Self::fields_preds(&subqueries, subquery, fields, k, deps)?;
                            if !variant_preds.is_empty() {
                                let in_variant =
                                    vcx.mk_eq_expr(discr.call()(k.downcast_ty()), *variant_discr);
                                let variant_preds = vcx.mk_conj(&variant_preds);
                                preds.push(vir::expr! { vcx; (in_variant) ==> (variant_preds) });
                            }
                        }
                        if !preds.is_empty() {
                            let guard = subquery.guard;
                            let preds = vcx.mk_conj(&preds);
                            conjuncts.push(vir::expr! { vcx; (guard) ==> (preds) });
                        }
                    }
                    let mut extend_body = k;
                    for (variant_discr, fields, cons) in variants.iter().rev() {
                        let field_snaps = Self::fields_extend(&subqueries, fields, k, deps)?;
                        let extended: vir::ExprSnap<'vir> = cons.call()(&field_snaps).upcast_ty();
                        let in_variant =
                            vcx.mk_eq_expr(discr.call()(k.downcast_ty()), *variant_discr);
                        extend_body = vir::expr! { vcx;
                            (in_variant) ? (extended) : (extend_body)
                        };
                    }
                    // Plug only if both snapshots have the variant; `s` must not change
                    // otherwise.
                    let mut graft_body = s;
                    for (variant_discr, fields, cons) in variants.iter().rev() {
                        let field_snaps = Self::fields_graft(&subqueries, fields, s, t, deps)?;
                        let grafted: vir::ExprSnap<'vir> = cons.call()(&field_snaps).upcast_ty();
                        let s_in_variant =
                            vcx.mk_eq_expr(discr.call()(s.downcast_ty()), *variant_discr);
                        let t_in_variant =
                            vcx.mk_eq_expr(discr.call()(t.downcast_ty()), *variant_discr);
                        graft_body = vir::expr! { vcx;
                            ((s_in_variant) && (t_in_variant)) ? (grafted) : (graft_body)
                        };
                    }
                    let mut shallow_body = s;
                    for (variant_discr, fields, cons) in variants.iter().rev() {
                        let field_snaps = Self::fields_shallow(fields, s, deps)?;
                        let collapsed: vir::ExprSnap<'vir> = cons.call()(&field_snaps).upcast_ty();
                        let in_variant =
                            vcx.mk_eq_expr(discr.call()(s.downcast_ty()), *variant_discr);
                        shallow_body = vir::expr! { vcx;
                            (in_variant) ? (collapsed) : (shallow_body)
                        };
                    }
                    (
                        Some(vcx.mk_conj(&conjuncts)),
                        Some(extend_body),
                        Some(graft_body),
                        Some(shallow_body),
                    )
                }
                TySpecifics::MutRef((data, pure_data)) => {
                    let referent_ty = data.referent.decompose(task_key.params);
                    let referent = deps.require_dep::<TyUseImpureEnc>(referent_ty)?;
                    let addr = pure_data.deref_access.call()(k.downcast_ty());
                    let referent_pred = referent.ref_to_pred(vcx, addr, None);
                    let subqueries = Self::subqueries(task_key, lo, hi, deps)?;
                    // With 'a in the query, the referent's snapshot is read from the heap,
                    // otherwise it is the stored one.
                    let in_query = subqueries
                        .iter()
                        .find(|subquery| {
                            matches!(subquery.lifetime, GeneralizedLifetime::Region(_))
                        })
                        .unwrap()
                        .guard;
                    let current_snap = referent.ref_to_snap(addr);
                    let stored_snap: vir::ExprSnap<'vir> =
                        pure_data.value_access.call()(k.downcast_ty()).upcast_ty();
                    let referent_snap = vir::expr! { vcx;
                        (in_query) ? (current_snap) : (stored_snap)
                    };
                    let mut conjuncts = Vec::new();
                    for &subquery in &subqueries {
                        let mut preds = Vec::new();
                        // The own lifetime of the reference gives access to the referent.
                        if matches!(subquery.lifetime, GeneralizedLifetime::Region(_)) {
                            preds.push(referent_pred);
                        }
                        // The referent's positions are passed on like a field's.
                        preds.extend(Self::field_preds(
                            &subqueries,
                            subquery,
                            referent_ty,
                            referent_snap,
                            deps,
                        )?);
                        if !preds.is_empty() {
                            let guard = subquery.guard;
                            let preds = vcx.mk_conj(&preds);
                            conjuncts.push(vir::expr! { vcx; (guard) ==> (preds) });
                        }
                    }
                    // The value is read from the heap if the query contains 'a and stored
                    // otherwise, then extended at the referent's positions.
                    let mut value = referent_snap;
                    for &subquery in &subqueries {
                        value = Self::field_extend(
                            &subqueries,
                            subquery,
                            referent_ty,
                            value,
                            referent_snap,
                            deps,
                        )?;
                    }
                    let metadata = pure_data.metadata_access.call()(k.downcast_ty());
                    let extend_body =
                        pure_data.prim_to_snap.call()(addr, metadata, value.downcast_ty());
                    // The referent's positions in the interval are grafted in like a
                    // field's: into `t`'s referent made shallow if 'a is in the
                    // interval, into `s`'s otherwise. A position behind 'a that is not
                    // in the interval is thus collapsed, even if `t` extended it.
                    let s_addr = pure_data.deref_access.call()(s.downcast_ty());
                    let s_metadata = pure_data.metadata_access.call()(s.downcast_ty());
                    let s_value: vir::ExprSnap<'vir> =
                        pure_data.value_access.call()(s.downcast_ty()).upcast_ty();
                    let t_value: vir::ExprSnap<'vir> =
                        pure_data.value_access.call()(t.downcast_ty()).upcast_ty();
                    let t_shallow = Self::field_shallow(referent_ty, t_value, deps)?;
                    let mut value = vir::expr! { vcx; (in_query) ? (t_shallow) : (s_value) };
                    for &subquery in &subqueries {
                        value = Self::field_graft(subquery, referent_ty, value, t_value, deps)?;
                    }
                    let graft_body =
                        pure_data.prim_to_snap.call()(s_addr, s_metadata, value.downcast_ty());
                    // Reset, the reference has the snapshot its direct predicate gives it.
                    let arbitrary_value = deps
                        .require_dep::<TyImpureEnc>(*task_key)?
                        .expect_mutref()
                        .arbitrary_value;
                    let shallow_body = arbitrary_value(
                        s_addr,
                        s_metadata,
                        params.ty_exprs(),
                        params.const_exprs(),
                    );
                    (
                        Some(vcx.mk_conj(&conjuncts)),
                        Some(extend_body.upcast_ty()),
                        Some(graft_body.upcast_ty()),
                        Some(shallow_body.upcast_ty()),
                    )
                }
                // A shared reference has no lifetime positions.
                TySpecifics::ImmRef(_) => (Some(vcx.mk_bool::<true>()), Some(k), Some(s), Some(s)),
                // TODO: the remaining types.
                _ => (None, None, None, None),
            };

            let predicate_decl = vcx.mk_predicate(
                predicate,
                (
                    k_decl,
                    params.ty_decls(),
                    params.const_decls(),
                    lo_decl,
                    hi_decl,
                ),
                body,
            );

            // Fetching through an empty interval reads nothing and needs no predicate, so
            // callers extend through a position's interval without checking whether the
            // query contains it.
            let pre = vir::expr! { vcx;
                ((lo) < (hi)) ==> (acc([predicate](
                    k,
                    [..[params.ty_exprs()]],
                    [..[params.const_exprs()]],
                    lo,
                    hi
                )))
            };
            let extend_body = extend_body.map(|extend_body| {
                vir::expr! { vcx;
                    ((lo) < (hi)) ? (unfolding ([predicate](
                        k,
                        [..[params.ty_exprs()]],
                        [..[params.const_exprs()]],
                        lo,
                        hi
                    )) in (extend_body)) : (k)
                }
            });
            // At a type parameter, `extend`, `graft` and `shallow` are abstract. What
            // `extend` reads differs from its key only at the extended positions, so
            // grafting it back in gives it again, and making it shallow gives the
            // shallow key; field
            // snapshots of types containing the parameter have these forms.
            let mut extend_posts = Vec::new();
            if ty.specifics.is_param() {
                let result = vcx.mk_result(snapshot);
                let grafted =
                    graft.call()(k, params.ty_exprs(), params.const_exprs(), result, lo, hi);
                extend_posts.push(vcx.mk_eq_expr(grafted, result));
                let shallow_result =
                    shallow.call()(result, params.ty_exprs(), params.const_exprs());
                let shallow_key = shallow.call()(k, params.ty_exprs(), params.const_exprs());
                extend_posts.push(vcx.mk_eq_expr(shallow_result, shallow_key));
            }
            // Grafting an empty interval changes nothing, as for every type whose `graft`
            // has a body.
            let mut graft_posts = Vec::new();
            if ty.specifics.is_param() {
                let result = vcx.mk_result(snapshot);
                let unchanged = vcx.mk_eq_expr(result, s);
                graft_posts.push(vir::expr! { vcx; ((hi) <= (lo)) ==> (unchanged) });
            }
            let extend_decl = vcx.mk_function(
                extend,
                (
                    k_decl,
                    params.ty_decls(),
                    params.const_decls(),
                    lo_decl,
                    hi_decl,
                ),
                vcx.alloc_slice(&[pre]),
                vcx.alloc_slice(&extend_posts),
                None,
                extend_body,
            );

            let graft_decl = vcx.mk_function(
                graft,
                (
                    s_decl,
                    params.ty_decls(),
                    params.const_decls(),
                    t_decl,
                    lo_decl,
                    hi_decl,
                ),
                &[],
                vcx.alloc_slice(&graft_posts),
                None,
                graft_body,
            );

            let shallow_decl = vcx.mk_function(
                shallow,
                (s_decl, params.ty_decls(), params.const_decls()),
                &[],
                &[],
                None,
                shallow_body,
            );

            Ok(((predicate_decl, extend_decl, graft_decl, shallow_decl), ()))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for (predicate, extend, graft, shallow) in Self::all_outputs_local_no_errors(program) {
            program.add_predicate(predicate);
            program.add_function(extend);
            program.add_function(graft);
            program.add_function(shallow);
        }
    }
}

/// Tests that verify Viper code using the indirect predicates with Silicon.
#[cfg(test)]
mod verification_tests {
    use prusti_rustc_interface::span::DUMMY_SP;
    use task_encoder::TaskEncoder;

    use super::IndirectPredEnc;
    use crate::encoders::ty::{
        RustTyDecomposition, generics::IndirectCastersEnc, test_util::verify,
    };

    /// Verifies the Viper code `viper` together with the indirect predicate
    /// of the type alias `X` defined in `src`.
    fn verify_with_pred(src: &str, viper: &str) -> Result<(), String> {
        verify(
            src,
            |ty| {
                IndirectPredEnc::encode(ty.ty, true, DUMMY_SP).unwrap();
            },
            viper,
        )
    }

    /// Like `verify_with_pred`, also with the casts of `X`'s type arguments.
    fn verify_with_casts(src: &str, viper: &str) -> Result<(), String> {
        verify(
            src,
            |ty| {
                IndirectPredEnc::encode(ty.ty, true, DUMMY_SP).unwrap();
                for arg in ty.args.args() {
                    let arg = RustTyDecomposition::from_ty(arg.expect_ty(), ty.args.context());
                    IndirectCastersEnc::encode(
                        (RustTyDecomposition::param(), arg.ty),
                        true,
                        DUMMY_SP,
                    )
                    .unwrap();
                }
            },
            viper,
        )
    }

    fn assert_verifies(src: &str, viper: &str) {
        if let Err(output) = verify_with_pred(src, viper) {
            panic!("verification failed:\n{output}");
        }
    }

    /// Asserts that the verification fails with an error containing `error`.
    fn assert_fails(src: &str, viper: &str, error: &str) {
        match verify_with_pred(src, viper) {
            Ok(()) => panic!("verification succeeded"),
            Err(output) => assert!(
                output.contains(error),
                "verification failed differently:\n{output}"
            ),
        }
    }

    /// `&'a mut U`: position 0 is `'a`, the referent's positions are
    /// `[1, 1 + arity(U))`.
    #[test]
    fn mutable_reference() {
        let src = "type X<'a, T> = &'a mut T;";
        assert_verifies(
            src,
            r#"
            // 'a holds the referent.
            method lifetime(k: s_Ref_mutable, U: Type)
              requires p_Ref_mutable_ind(k, U, 0, 1)
              ensures p_Param(k.s_Ref_mutable_0, U)
            {
              unfold p_Ref_mutable_ind(k, U, 0, 1)
            }

            // The referent's positions hold those of the referent, on the
            // snapshot stored in `k`.
            method referent_positions(k: s_Ref_mutable, U: Type)
              requires 0 < arity(U)
              requires p_Ref_mutable_ind(k, U, 1, 1 + arity(U))
              ensures p_Param_ind(k.s_Ref_mutable_2, U, 0, arity(U))
            {
              unfold p_Ref_mutable_ind(k, U, 1, 1 + arity(U))
            }

            // With 'a, they are on the referent's current snapshot.
            method all_positions(k: s_Ref_mutable, U: Type)
              requires 0 < arity(U)
              requires p_Ref_mutable_ind(k, U, 0, 1 + arity(U))
              ensures p_Param(k.s_Ref_mutable_0, U)
              ensures p_Param_ind(p_Param_snap(k.s_Ref_mutable_0, U), U, 0, arity(U))
            {
              unfold p_Ref_mutable_ind(k, U, 0, 1 + arity(U))
            }

            // Extending by 'a reads the referent's snapshot and keeps the address.
            method extend(k: s_Ref_mutable, U: Type)
              requires p_Ref_mutable_ind(k, U, 0, 1)
              ensures p_Ref_mutable_ind(k, U, 0, 1)
              ensures extend_Ref_mutable(k, U, 0, 1).s_Ref_mutable_0 == k.s_Ref_mutable_0
              ensures extend_Ref_mutable(k, U, 0, 1).s_Ref_mutable_2 ==
                (unfolding p_Ref_mutable_ind(k, U, 0, 1) in p_Param_snap(k.s_Ref_mutable_0, U))
            {
            }

            // Split off 'a, the referent's positions are on the snapshot
            // extended by 'a.
            method split(k: s_Ref_mutable, U: Type)
              requires p_Ref_mutable_ind(k, U, 0, 1 + arity(U))
              ensures p_Ref_mutable_ind(k, U, 0, 1)
              ensures p_Ref_mutable_ind(extend_Ref_mutable(k, U, 0, 1), U, 1, 1 + arity(U))
            {
              unfold p_Ref_mutable_ind(k, U, 0, 1 + arity(U))
              fold p_Ref_mutable_ind(k, U, 0, 1)
              fold p_Ref_mutable_ind(extend_Ref_mutable(k, U, 0, 1), U, 1, 1 + arity(U))
            }
            "#,
        );
        // The referent's positions do not hold the referent.
        assert_fails(
            src,
            r#"
            method test(k: s_Ref_mutable, U: Type)
              requires p_Ref_mutable_ind(k, U, 1, 1 + arity(U))
              ensures p_Param(k.s_Ref_mutable_0, U)
            {
              unfold p_Ref_mutable_ind(k, U, 1, 1 + arity(U))
            }
            "#,
            "insufficient permission to access p_Param",
        );
        // 'a does not hold the referent's positions.
        assert_fails(
            src,
            r#"
            method test(k: s_Ref_mutable, U: Type)
              requires 0 < arity(U)
              requires p_Ref_mutable_ind(k, U, 0, 1)
              ensures p_Param(k.s_Ref_mutable_0, U)
              ensures p_Param_ind(p_Param_snap(k.s_Ref_mutable_0, U), U, 0, arity(U))
            {
              unfold p_Ref_mutable_ind(k, U, 0, 1)
            }
            "#,
            "insufficient permission to access p_Param_ind",
        );
    }

    /// `&'a U` has no positions: its predicate holds nothing.
    #[test]
    fn shared_reference() {
        let src = "type X<'a, T> = &'a T;";
        assert_verifies(
            src,
            r#"
            method test(k: s_Ref_immutable, U: Type)
              ensures p_Ref_immutable_ind(k, U, 0, 1)
              ensures extend_Ref_immutable(k, U, 0, 1) == k
            {
              fold p_Ref_immutable_ind(k, U, 0, 1)
            }
            "#,
        );
        assert_fails(
            src,
            r#"
            method test(k: s_Ref_immutable, U: Type)
              requires p_Ref_immutable_ind(k, U, 0, 1)
            {
              unfold p_Ref_immutable_ind(k, U, 0, 1)
              assert false
            }
            "#,
            "Assert might fail",
        );
    }

    /// `'static` is not a position of a struct: no position holds the
    /// referent of a `&'static mut`.
    #[test]
    fn static_mutable_reference() {
        let src = "struct S<'a> { s: &'static mut i32, t: &'a mut i32 }
                   type X<'a> = S<'a>;";
        assert_verifies(
            src,
            r#"
            method test(k: s_S)
              requires p_S_ind(k, 0, 1)
              ensures p_Ref_mutable_ind(k.s_S_1, s_Int_i32_type(), 0, 1)
            {
              unfold p_S_ind(k, 0, 1)
            }
            "#,
        );
        assert_fails(
            src,
            r#"
            method test(k: s_S)
              requires p_S_ind(k, 0, 1)
              ensures p_Ref_mutable_ind(k.s_S_0, s_Int_i32_type(), 0, 1)
            {
              unfold p_S_ind(k, 0, 1)
            }
            "#,
            "insufficient permission to access p_Ref_mutable_ind",
        );
    }

    /// A struct passes its positions on to its fields: `'a @ 0` and
    /// `T @ [1, 1 + arity(T))`.
    #[test]
    fn struct_fields() {
        let src = "struct S<'a, T> { x: &'a mut i32, y: T }
                   type X<'a, T> = S<'a, T>;";
        assert_verifies(
            src,
            r#"
            method lifetime(k: s_S, T: Type)
              requires p_S_ind(k, T, 0, 1)
              ensures p_Ref_mutable_ind(k.s_S_0, s_Int_i32_type(), 0, 1)
            {
              unfold p_S_ind(k, T, 0, 1)
            }

            method type_param(k: s_S, T: Type)
              requires 0 < arity(T)
              requires p_S_ind(k, T, 1, 1 + arity(T))
              ensures p_Param_ind(k.s_S_1, T, 0, arity(T))
            {
              unfold p_S_ind(k, T, 1, 1 + arity(T))
            }

            method split(k: s_S, T: Type)
              requires p_S_ind(k, T, 0, 1 + arity(T))
              ensures p_S_ind(k, T, 0, 1) && p_S_ind(k, T, 1, 1 + arity(T))
            {
              unfold p_S_ind(k, T, 0, 1 + arity(T))
              fold p_S_ind(k, T, 0, 1)
              fold p_S_ind(k, T, 1, 1 + arity(T))
            }

            method merge(k: s_S, T: Type)
              requires p_S_ind(k, T, 0, 1) && p_S_ind(k, T, 1, 1 + arity(T))
              ensures p_S_ind(k, T, 0, 1 + arity(T))
            {
              unfold p_S_ind(k, T, 0, 1)
              unfold p_S_ind(k, T, 1, 1 + arity(T))
              fold p_S_ind(k, T, 0, 1 + arity(T))
            }
            "#,
        );
        // The positions of T do not hold `x`.
        assert_fails(
            src,
            r#"
            method test(k: s_S, T: Type)
              requires p_S_ind(k, T, 1, 1 + arity(T))
              ensures p_Ref_mutable_ind(k.s_S_0, s_Int_i32_type(), 0, 1)
            {
              unfold p_S_ind(k, T, 1, 1 + arity(T))
            }
            "#,
            "insufficient permission to access p_Ref_mutable_ind",
        );
        // 'a does not hold `y`.
        assert_fails(
            src,
            r#"
            method test(k: s_S, T: Type)
              requires 0 < arity(T)
              requires p_S_ind(k, T, 0, 1)
              ensures p_Param_ind(k.s_S_1, T, 0, arity(T))
            {
              unfold p_S_ind(k, T, 0, 1)
            }
            "#,
            "insufficient permission to access p_Param_ind",
        );
    }

    /// The positions of a tuple are those of its elements in turn.
    #[test]
    fn tuple() {
        let src = "type X<T, U> = (T, U);";
        assert_verifies(
            src,
            r#"
            method elements(k: s_2_Tuple, T: Type, U: Type)
              requires 0 < arity(T) && 0 < arity(U)
              requires p_2_Tuple_ind(k, T, U, 0, arity(T) + arity(U))
              ensures p_Param_ind(k.s_2_Tuple_0, T, 0, arity(T))
              ensures p_Param_ind(k.s_2_Tuple_1, U, 0, arity(U))
            {
              unfold p_2_Tuple_ind(k, T, U, 0, arity(T) + arity(U))
            }

            method split(k: s_2_Tuple, T: Type, U: Type)
              requires p_2_Tuple_ind(k, T, U, 0, arity(T) + arity(U))
              ensures p_2_Tuple_ind(k, T, U, 0, arity(T))
              ensures p_2_Tuple_ind(k, T, U, arity(T), arity(T) + arity(U))
            {
              unfold p_2_Tuple_ind(k, T, U, 0, arity(T) + arity(U))
              fold p_2_Tuple_ind(k, T, U, 0, arity(T))
              fold p_2_Tuple_ind(k, T, U, arity(T), arity(T) + arity(U))
            }
            "#,
        );
        // The positions of T do not hold those of U.
        assert_fails(
            src,
            r#"
            method test(k: s_2_Tuple, T: Type, U: Type)
              requires 0 < arity(U)
              requires p_2_Tuple_ind(k, T, U, 0, arity(T))
              ensures p_Param_ind(k.s_2_Tuple_1, U, 0, arity(U))
            {
              unfold p_2_Tuple_ind(k, T, U, 0, arity(T))
            }
            "#,
            "insufficient permission to access p_Param_ind",
        );
    }

    /// In the field type `Three<'a, 'b, 'a>`, `'a` occurs at positions 0
    /// and 2, with `'b` in between.
    #[test]
    fn duplicate_lifetimes() {
        let src = "struct Three<'x, 'y, 'z>(&'x mut i32, &'y mut i32, &'z mut i32);
                   struct S<'a, 'b> { f: Three<'a, 'b, 'a> }
                   type X<'a, 'b> = S<'a, 'b>;";
        assert_verifies(
            src,
            r#"
            method test(k: s_S)
              requires p_S_ind(k, 0, 1)
              ensures p_Three_ind(k.s_S_0, 0, 1) && p_Three_ind(k.s_S_0, 2, 3)
            {
              unfold p_S_ind(k, 0, 1)
            }
            "#,
        );
        // 'a does not hold the position of 'b.
        assert_fails(
            src,
            r#"
            method test(k: s_S)
              requires p_S_ind(k, 0, 1)
              ensures p_Three_ind(k.s_S_0, 1, 2)
            {
              unfold p_S_ind(k, 0, 1)
            }
            "#,
            "insufficient permission to access p_Three_ind",
        );
    }

    /// In the field type `(T, U, T)`, `T` occurs twice, with `U` in between.
    #[test]
    fn duplicate_type_params() {
        let src = "struct S<T, U> { f: (T, U, T) }
                   type X<T, U> = S<T, U>;";
        assert_verifies(
            src,
            r#"
            method test(k: s_S, T: Type, U: Type)
              requires 0 < arity(T)
              requires p_S_ind(k, T, U, 0, arity(T))
              ensures p_Param_ind(k.s_S_0.s_3_Tuple_0, T, 0, arity(T))
              ensures p_Param_ind(k.s_S_0.s_3_Tuple_2, T, 0, arity(T))
            {
              unfold p_S_ind(k, T, U, 0, arity(T))
              unfold p_3_Tuple_ind(k.s_S_0, T, U, T, 0, arity(T))
              unfold p_3_Tuple_ind(k.s_S_0, T, U, T, arity(T) + arity(U), 2 * arity(T) + arity(U))
            }
            "#,
        );
        // The positions of T do not hold those of U.
        assert_fails(
            src,
            r#"
            method test(k: s_S, T: Type, U: Type)
              requires 0 < arity(T) && 0 < arity(U)
              requires p_S_ind(k, T, U, 0, arity(T))
              ensures p_Param_ind(k.s_S_0.s_3_Tuple_1, U, 0, arity(U))
            {
              unfold p_S_ind(k, T, U, 0, arity(T))
              unfold p_3_Tuple_ind(k.s_S_0, T, U, T, 0, arity(T))
              unfold p_3_Tuple_ind(k.s_S_0, T, U, T, arity(T) + arity(U), 2 * arity(T) + arity(U))
            }
            "#,
            "insufficient permission to access p_Param_ind",
        );
    }

    /// In `&'a mut &'b mut i32`, the position of `'b` lies behind `'a`. Split
    /// off `'a`, it is on the snapshot extended by `'a`.
    #[test]
    fn nested_mutable_references() {
        let src = "struct S<'a, 'b> { r: &'a mut &'b mut i32 }
                   type X<'a, 'b> = S<'a, 'b>;";
        assert_verifies(
            src,
            r#"
            method split(k: s_S)
              requires p_S_ind(k, 0, 2)
              ensures p_S_ind(k, 0, 1) && p_S_ind(extend_S(k, 0, 1), 1, 2)
            {
              unfold p_S_ind(k, 0, 2)
              fold p_S_ind(k, 0, 1)
              fold p_S_ind(extend_S(k, 0, 1), 1, 2)
            }
            "#,
        );
        // Not on the snapshot that is not extended.
        assert_fails(
            src,
            r#"
            method split(k: s_S)
              requires p_S_ind(k, 0, 2)
              ensures p_S_ind(k, 0, 1) && p_S_ind(k, 1, 2)
            {
              unfold p_S_ind(k, 0, 2)
              fold p_S_ind(k, 0, 1)
              fold p_S_ind(k, 1, 2)
            }
            "#,
            "insufficient permission to access p_Ref_mutable_ind",
        );
    }

    /// In `N<'b, 'a>`, `'b` comes first but lies behind `'a`. Extending by both
    /// positions is extending by `'a`, then by `'b` on the result.
    #[test]
    fn enclosed_lifetime_first() {
        let src = "struct N<'b, 'a>(&'a mut &'b mut i32);
                   type X<'b, 'a> = N<'b, 'a>;";
        assert_verifies(
            src,
            r#"
            method test(k: s_N)
              requires p_N_ind(k, 0, 2)
            {
              var all: s_N := extend_N(k, 0, 2)
              unfold p_N_ind(k, 0, 2)
              fold p_N_ind(k, 1, 2)
              fold p_N_ind(extend_N(k, 1, 2), 0, 1)
              assert all == extend_N(extend_N(k, 1, 2), 0, 1)
            }
            "#,
        );
        assert_fails(
            src,
            r#"
            method test(k: s_N)
              requires p_N_ind(k, 0, 2)
            {
              var all: s_N := extend_N(k, 0, 2)
              unfold p_N_ind(k, 0, 2)
              fold p_N_ind(k, 1, 2)
              fold p_N_ind(extend_N(k, 1, 2), 0, 1)
              assert false
            }
            "#,
            "Assert might fail",
        );
    }

    /// In `&'a mut &'a mut i32`, the predicate puts the inner `'a` on the
    /// snapshot extended by the outer one.
    #[test]
    fn nested_mutable_references_same_lifetime() {
        let src = "struct E<'a>(&'a mut &'a mut i32);
                   type X<'a> = E<'a>;";
        assert_verifies(
            src,
            r#"
            method test(k: s_E)
              requires p_E_ind(k, 0, 1)
              ensures p_Ref_mutable_ind(k.s_E_0, s_Ref_mutable_type(s_Int_i32_type()), 0, 1)
              ensures p_Ref_mutable_ind(extend_Ref_mutable(k.s_E_0, s_Ref_mutable_type(s_Int_i32_type()), 0, 1), s_Ref_mutable_type(s_Int_i32_type()), 1, 2)
            {
              unfold p_E_ind(k, 0, 1)
            }
            "#,
        );
        // Not on the field snapshot that is not extended.
        assert_fails(
            src,
            r#"
            method test(k: s_E)
              requires p_E_ind(k, 0, 1)
              ensures p_Ref_mutable_ind(k.s_E_0, s_Ref_mutable_type(s_Int_i32_type()), 1, 2)
            {
              unfold p_E_ind(k, 0, 1)
            }
            "#,
            "insufficient permission to access p_Ref_mutable_ind",
        );
    }

    /// An enum passes its positions on to the fields of the variant it is
    /// in: `'a @ 0` to variant `A`, `T @ [1, 1 + arity(T))` to variant `B`.
    #[test]
    fn enum_variants() {
        let src = "enum E<'a, T> { A(&'a mut i32), B(T), C }
                   type X<'a, T> = E<'a, T>;";
        assert_verifies(
            src,
            r#"
            method variant_a(k: s_E, T: Type)
              requires p_E_ind(k, T, 0, 1) && k.iss_E_0_cons
              ensures p_Ref_mutable_ind(k.s_E_0_0, s_Int_i32_type(), 0, 1)
            {
              unfold p_E_ind(k, T, 0, 1)
            }

            method variant_b(k: s_E, T: Type)
              requires 0 < arity(T)
              requires p_E_ind(k, T, 1, 1 + arity(T)) && k.iss_E_1_cons
              ensures p_Param_ind(k.s_E_1_0, T, 0, arity(T))
            {
              unfold p_E_ind(k, T, 1, 1 + arity(T))
            }

            // Variant `C` holds nothing.
            method variant_c(k: s_E, T: Type)
              requires k.iss_E_2_cons
              ensures p_E_ind(k, T, 0, 1 + arity(T))
            {
              fold p_E_ind(k, T, 0, 1 + arity(T))
            }

            // Extending keeps the variant.
            method extend(k: s_E, T: Type)
              requires p_E_ind(k, T, 0, 1)
              ensures p_E_ind(k, T, 0, 1)
              ensures s_E_discr(extend_E(k, T, 0, 1)) == s_E_discr(k)
            {
            }

            method split(k: s_E, T: Type)
              requires p_E_ind(k, T, 0, 1 + arity(T))
              ensures p_E_ind(k, T, 0, 1) && p_E_ind(k, T, 1, 1 + arity(T))
            {
              unfold p_E_ind(k, T, 0, 1 + arity(T))
              fold p_E_ind(k, T, 0, 1)
              fold p_E_ind(k, T, 1, 1 + arity(T))
            }
            "#,
        );
        // Without knowing the variant, the field is not held.
        assert_fails(
            src,
            r#"
            method test(k: s_E, T: Type)
              requires p_E_ind(k, T, 0, 1)
              ensures p_Ref_mutable_ind(k.s_E_0_0, s_Int_i32_type(), 0, 1)
            {
              unfold p_E_ind(k, T, 0, 1)
            }
            "#,
            "insufficient permission to access p_Ref_mutable_ind",
        );
        // In variant `B`, 'a holds nothing.
        assert_fails(
            src,
            r#"
            method test(k: s_E, T: Type)
              requires p_E_ind(k, T, 0, 1) && k.iss_E_1_cons
              ensures p_Ref_mutable_ind(k.s_E_0_0, s_Int_i32_type(), 0, 1)
            {
              unfold p_E_ind(k, T, 0, 1)
            }
            "#,
            "insufficient permission to access p_Ref_mutable_ind",
        );
    }

    /// The casts let `Option<&'a mut U>`'s predicate be unfolded down to the
    /// referent.
    #[test]
    fn casts() {
        let src = "type X<'a, T> = Option<&'a mut T>;";
        let result = verify_with_casts(
            src,
            r#"
            // The snapshot of the field of `Some` is that of a reference
            // once its type is known.
            method unfold_option(k: s_Option, U: Type)
              requires p_Option_ind(k, s_Ref_mutable_type(U), 0, 1) && k.iss_Option_1_cons
              requires s_Param_typ(k.s_Option_1_0) == s_Ref_mutable_type(U)
              ensures p_Param(k.s_Option_1_0.make_concrete_s_Ref_mutable.s_Ref_mutable_0, U)
            {
              unfold p_Option_ind(k, s_Ref_mutable_type(U), 0, 1)
              make_concrete_ind_Ref_mutable(k.s_Option_1_0.make_concrete_s_Ref_mutable, U, 0, 1)
              unfold p_Ref_mutable_ind(k.s_Option_1_0.make_concrete_s_Ref_mutable, U, 0, 1)
            }

            // Making a snapshot shallow is the same on both sides of a cast.
            method shallow(k: s_Ref_mutable, U: Type)
              requires p_Ref_mutable_ind(k, U, 0, 1)
            {
              make_generic_ind_Ref_mutable(k, U, 0, 1)
              assert shallow_Param(make_generic_s_Ref_mutable(k, U), s_Ref_mutable_type(U)) ==
                make_generic_s_Ref_mutable(shallow_Ref_mutable(k, U), U)
            }

            // Casting back and forth keeps what extending reads.
            method round_trip(k: s_Ref_mutable, U: Type)
              requires p_Ref_mutable_ind(k, U, 0, 1)
              ensures p_Ref_mutable_ind(k, U, 0, 1)
              ensures extend_Ref_mutable(k, U, 0, 1) == old(extend_Ref_mutable(k, U, 0, 1))
            {
              make_generic_ind_Ref_mutable(k, U, 0, 1)
              make_concrete_ind_Ref_mutable(k, U, 0, 1)
            }

            // What extending the option reads is what extending the reference reads.
            method extend_option(k: s_Option, U: Type)
              requires p_Option_ind(k, s_Ref_mutable_type(U), 0, 1) && k.iss_Option_1_cons
              requires s_Param_typ(k.s_Option_1_0) == s_Ref_mutable_type(U)
              ensures p_Ref_mutable_ind(k.s_Option_1_0.make_concrete_s_Ref_mutable, U, 0, 1)
              ensures old(extend_Option(k, s_Ref_mutable_type(U), 0, 1)).s_Option_1_0 ==
                make_generic_s_Ref_mutable(extend_Ref_mutable(
                  k.s_Option_1_0.make_concrete_s_Ref_mutable, U, 0, 1), U)
            {
              unfold p_Option_ind(k, s_Ref_mutable_type(U), 0, 1)
              make_concrete_ind_Ref_mutable(k.s_Option_1_0.make_concrete_s_Ref_mutable, U, 0, 1)
            }
            "#,
        );
        if let Err(output) = result {
            panic!("verification failed:\n{output}");
        }
        // The casts do not make the state inconsistent.
        let result = verify_with_casts(
            src,
            r#"
            method test(k: s_Ref_mutable, U: Type)
              requires p_Ref_mutable_ind(k, U, 0, 1)
            {
              make_generic_ind_Ref_mutable(k, U, 0, 1)
              make_concrete_ind_Ref_mutable(k, U, 0, 1)
              assert false
            }
            "#,
        );
        match result {
            Ok(()) => panic!("verification succeeded"),
            Err(output) => assert!(
                output.contains("Assert might fail"),
                "verification failed differently:\n{output}"
            ),
        }
    }

    /// 'a encloses 'b in `B` but not in `A`, so `A`'s predicate for 'b is on
    /// the shallow field snapshot.
    #[test]
    fn field_with_fewer_enclosing_positions() {
        let src = "struct A<'a, 'b>(&'a mut i32, &'b mut i32);
                   struct B<'a, 'b>(A<'a, 'b>, &'a mut &'b mut i32);
                   type X<'a, 'b> = B<'a, 'b>;";
        assert_verifies(
            src,
            r#"
            method test(k: s_B)
              requires p_B_ind(k, 0, 1)
              requires p_B_ind(extend_B(k, 0, 1), 1, 2)
              ensures p_A_ind(shallow_A(k.s_B_0), 1, 2)
            {
              unfold p_B_ind(extend_B(k, 0, 1), 1, 2)
            }
            "#,
        );
        assert_fails(
            src,
            r#"
            method test(k: s_B)
              requires p_B_ind(k, 0, 1)
              requires p_B_ind(extend_B(k, 0, 1), 1, 2)
            {
              unfold p_B_ind(extend_B(k, 0, 1), 1, 2)
              assert false
            }
            "#,
            "Assert might fail",
        );
        // The field's snapshot itself may carry what extending by 'a read.
        assert_fails(
            src,
            r#"
            method test(k: s_B)
              requires p_B_ind(k, 0, 1)
              requires p_B_ind(extend_B(k, 0, 1), 1, 2)
              ensures p_A_ind(k.s_B_0, 1, 2)
            {
              unfold p_B_ind(extend_B(k, 0, 1), 1, 2)
            }
            "#,
            "insufficient permission to access p_A_ind",
        );
    }

    /// Like `field_with_fewer_enclosing_positions`, with a recursive field
    /// type: the field of an unextended snapshot gets `List`'s predicate for
    /// 'b on its own snapshot, though the predicate is on the shallow field
    /// after extending by 'a.
    #[test]
    fn recursive_field_with_fewer_enclosing_positions() {
        let src = "struct List<'a, 'b> {
                       x: &'a mut i32,
                       y: &'b mut i32,
                       next: Option<Box<List<'a, 'b>>>,
                   }
                   struct Outer<'a, 'b>(List<'a, 'b>, &'a mut &'b mut i32);
                   type X<'a, 'b> = Outer<'a, 'b>;";
        assert_verifies(
            src,
            r#"
            method test(k: s_Outer)
              requires shallow_Outer(k) == k
              requires p_Outer_ind(k, 0, 1) && p_Outer_ind(extend_Outer(k, 0, 1), 1, 2)
              ensures p_List_ind(k.s_Outer_0, 1, 2)
            {
              unfold p_Outer_ind(extend_Outer(k, 0, 1), 1, 2)
            }
            "#,
        );
        assert_fails(
            src,
            r#"
            method test(k: s_Outer)
              requires shallow_Outer(k) == k
              requires p_Outer_ind(k, 0, 1) && p_Outer_ind(extend_Outer(k, 0, 1), 1, 2)
            {
              unfold p_Outer_ind(extend_Outer(k, 0, 1), 1, 2)
              assert false
            }
            "#,
            "Assert might fail",
        );
    }

    /// `List`'s predicate for 'b on the shallow field of `Outer` can be followed
    /// into the next element: the snapshot in `Option` is shallow, which the
    /// cast of `Box<List>` turns into the shallow concrete snapshot.
    #[test]
    fn recursive_field_into_next() {
        // The tuple's second element provides the casts of `Box<List>`.
        let src = "struct List<'a, 'b> {
                       x: &'a mut i32,
                       y: &'b mut i32,
                       next: Option<Box<List<'a, 'b>>>,
                   }
                   struct Outer<'a, 'b>(List<'a, 'b>, &'a mut &'b mut i32);
                   type X<'a, 'b> = (Outer<'a, 'b>, Box<List<'a, 'b>>);";
        let result = verify_with_casts(
            src,
            r#"
            method test(k: s_Outer, b: s_Box)
              requires p_Outer_ind(k, 0, 1) && p_Outer_ind(extend_Outer(k, 0, 1), 1, 2)
              requires k.s_Outer_0.s_List_2 ==
                s_Option_1_cons(make_generic_s_Box(b, s_List_type(), s_Global_type()))
              ensures p_Box_ind(shallow_Box(b, s_List_type(), s_Global_type()), s_List_type(), s_Global_type(), 1, 2)
            {
              unfold p_Outer_ind(extend_Outer(k, 0, 1), 1, 2)
              unfold p_List_ind(shallow_List(extend_Outer(k, 0, 1).s_Outer_0), 1, 2)
              unfold p_Option_ind(shallow_List(extend_Outer(k, 0, 1).s_Outer_0).s_List_2,
                s_Box_type(s_List_type(), s_Global_type()), 1, 2)
              make_concrete_ind_Box(shallow_Box(b, s_List_type(), s_Global_type()),
                s_List_type(), s_Global_type(), 1, 2)
            }
            "#,
        );
        if let Err(output) = result {
            panic!("verification failed:\n{output}");
        }
        let result = verify_with_casts(
            src,
            r#"
            method test(k: s_Outer, b: s_Box)
              requires p_Outer_ind(k, 0, 1) && p_Outer_ind(extend_Outer(k, 0, 1), 1, 2)
              requires k.s_Outer_0.s_List_2 ==
                s_Option_1_cons(make_generic_s_Box(b, s_List_type(), s_Global_type()))
            {
              unfold p_Outer_ind(extend_Outer(k, 0, 1), 1, 2)
              unfold p_List_ind(shallow_List(extend_Outer(k, 0, 1).s_Outer_0), 1, 2)
              unfold p_Option_ind(shallow_List(extend_Outer(k, 0, 1).s_Outer_0).s_List_2,
                s_Box_type(s_List_type(), s_Global_type()), 1, 2)
              make_concrete_ind_Box(shallow_Box(b, s_List_type(), s_Global_type()),
                s_List_type(), s_Global_type(), 1, 2)
              assert false
            }
            "#,
        );
        match result {
            Ok(()) => panic!("verification succeeded"),
            Err(output) => assert!(
                output.contains("Assert might fail"),
                "verification failed differently:\n{output}"
            ),
        }
    }

    /// 'y encloses 'z in `C` but not in `F`, and lies behind 'x in both: `C`
    /// gives `F`'s predicates for 'y and 'z on one snapshot, the field
    /// extended by 'x, as `F` has them.
    #[test]
    fn field_with_fewer_enclosing_positions_behind_reference() {
        let src = "struct F<'x, 'y, 'z>(&'x mut (&'y mut i32, &'z mut i32));
                   struct C<'x, 'y, 'z>(F<'x, 'y, 'z>, &'y mut &'z mut i32);
                   type X<'x, 'y, 'z> = C<'x, 'y, 'z>;";
        let keys = r#"
            // The keys of 'y and 'z: `k` extended by their ancestors.
            define KY(k) extend_C(k, 0, 1)
            define KZ(k) extend_C(KY(k), 1, 2)
            "#;
        assert_verifies(
            src,
            &format!(
                "{keys}{}",
                r#"
            method test(k: s_C)
              requires p_C_ind(k, 0, 1) && p_C_ind(KY(k), 1, 2) && p_C_ind(KZ(k), 2, 3)
              ensures p_C_ind(k, 0, 1)
              ensures p_F_ind(KY(k).s_C_0, 1, 2) && p_F_ind(KY(k).s_C_0, 2, 3)
            {
              // 'z first: its key reads through the predicate of 'y.
              unfold p_C_ind(KZ(k), 2, 3)
              unfold p_C_ind(KY(k), 1, 2)
            }
            "#
            ),
        );
        assert_fails(
            src,
            &format!(
                "{keys}{}",
                r#"
            method test(k: s_C)
              requires p_C_ind(k, 0, 1) && p_C_ind(KY(k), 1, 2) && p_C_ind(KZ(k), 2, 3)
            {
              unfold p_C_ind(KZ(k), 2, 3)
              unfold p_C_ind(KY(k), 1, 2)
              assert false
            }
            "#
            ),
            "Assert might fail",
        );
    }

    /// 'l encloses `T` in `D` but not in the tuple, so `D` resets the tuple's
    /// 'l, but leaves all positions of `T` as its key has them: `D` does not
    /// know which of them enclose the interval, the instantiation does. With
    /// `T = S<'a, 'b, 'c>`, where 'b and 'c enclose 'a, the key of 'a is
    /// extended by 'b and 'c, which come after 'a.
    #[test]
    fn type_param_field_with_fewer_enclosing_positions() {
        let src = "struct D<'l, T> { c: &'l mut T, d: (&'l mut i32, T) }
                   type X<'l, T> = D<'l, T>;";
        let tuple = r#"
            define R s_Ref_mutable_type(s_Int_i32_type())
            define KEEP_T(k, T) graft_2_Tuple(shallow_2_Tuple(k.s_D_1, R, T), R, T,
              k.s_D_1, 1, 1 + arity(T))
            "#;
        assert_verifies(
            src,
            &format!(
                "{tuple}{}",
                r#"
            method test(k: s_D, T: Type)
              requires 1 <= arity(T)
              requires p_D_ind(k, T, 1, 2)
              ensures p_2_Tuple_ind(KEEP_T(k, T), R, T, 1, 2)
            {
              unfold p_D_ind(k, T, 1, 2)
            }
            "#
            ),
        );
        assert_fails(
            src,
            &format!(
                "{tuple}{}",
                r#"
            method test(k: s_D, T: Type)
              requires 1 <= arity(T)
              requires p_D_ind(k, T, 1, 2)
            {
              unfold p_D_ind(k, T, 1, 2)
              assert false
            }
            "#
            ),
            "Assert might fail",
        );
    }

    /// Resetting a mutable reference gives it the snapshot its direct
    /// predicate gives it.
    #[test]
    fn shallow_mutable_reference() {
        let src = "type X<'a, T> = &'a mut T;";
        assert_verifies(
            src,
            r#"
            method unextended(a: Ref, m: s_Param, U: Type)
              ensures shallow_Ref_mutable(p_Ref_mutable_arbitrary_value(a, m, U), U) ==
                p_Ref_mutable_arbitrary_value(a, m, U)
            {
            }

            method extended(k: s_Ref_mutable, U: Type)
              requires p_Ref_mutable_ind(k, U, 0, 1)
              ensures p_Ref_mutable_ind(k, U, 0, 1)
              ensures shallow_Ref_mutable(extend_Ref_mutable(k, U, 0, 1), U) == shallow_Ref_mutable(k, U)
            {
            }
            "#,
        );
        // An arbitrary snapshot need not be unextended.
        assert_fails(
            src,
            r#"
            method test(k: s_Ref_mutable, U: Type)
              ensures shallow_Ref_mutable(k, U) == k
            {
            }
            "#,
            "Postcondition of test might not hold",
        );
    }

    #[test]
    fn graft_empty_interval() {
        let src = "struct S<'a, 'b, T>(&'a mut &'b mut i32, Option<&'a mut T>);
                   type X<'a, 'b, T> = S<'a, 'b, T>;";
        assert_verifies(
            src,
            r#"
            method test(s: s_S, t: s_S, T: Type, lo: Int, hi: Int)
              requires hi <= lo
              ensures graft_S(s, T, t, lo, hi) == s
            {
            }
            "#,
        );
        // Plugging a position may change the snapshot.
        assert_fails(
            src,
            r#"
            method test(s: s_S, t: s_S, T: Type)
              ensures graft_S(s, T, t, 0, 1) == s
            {
            }
            "#,
            "Postcondition of test might not hold",
        );
        assert_fails(
            src,
            r#"
            method test(s: s_S, t: s_S, T: Type, lo: Int, hi: Int)
              requires hi <= lo
            {
              assert graft_S(s, T, t, lo, hi) == s
              assert false
            }
            "#,
            "Assert might fail",
        );
    }
}
