use prusti_rustc_interface::{
    index::{Idx, IndexVec},
    middle::ty,
};
use task_encoder::{EncodeFullError, EncodeFullResult, TaskEncoder, TaskEncoderDependencies};
use vir::CastType;

use crate::encoders::ty::{
    RustParamData, RustTyDatas, RustTyDecomposition, TySpecifics,
    data::TyData,
    generics::{
        GParams,
        arity::{TyArityEnc, add},
    },
};

/// Encodes the lifetime positions of a type and which of them enclose which.
pub struct LifetimePositionEnc;

/// Single lifetime, or all lifetimes in a type parameter (similar to `GeneralizedLifetime` in the PCG)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeneralizedLifetime<'tcx> {
    Region(ty::Region<'tcx>),
    RegionsIn(RustTyDecomposition<'tcx>),
}

/// The n-th generalized lifetime of a type, in position order; only meaningful in its graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LifetimePosition(usize);

impl Idx for LifetimePosition {
    fn new(index: usize) -> Self {
        LifetimePosition(index)
    }

    fn index(self) -> usize {
        self.0
    }
}

/// The lifetime positions of a type, in order, and which of them enclose
/// which, read off the outlives requirements that rustc infers.
#[derive(Debug, Clone)]
pub struct EnclosureGraph<'vir> {
    lifetimes: IndexVec<LifetimePosition, GeneralizedLifetime<'vir>>,
    intervals: IndexVec<LifetimePosition, (vir::ExprInt<'vir>, vir::ExprInt<'vir>)>,
    /// For each position, the positions enclosing it, transitively, outermost first.
    ancestors: IndexVec<LifetimePosition, Vec<LifetimePosition>>,
    /// For each position, the indices of the generic arguments that lead from
    /// the type to its lifetime: the last one is a lifetime argument, or the
    /// path ends at a type parameter, whose regions the position covers.
    paths: IndexVec<LifetimePosition, Vec<usize>>,
}

impl<'vir> EnclosureGraph<'vir> {
    pub fn positions(&self) -> impl Iterator<Item = LifetimePosition> {
        self.lifetimes.indices()
    }

    /// The positions, each after the positions enclosing it. A position
    /// enclosing another one has fewer ancestors, since ancestors are
    /// transitive and the graph has no cycles.
    pub fn positions_outermost_first(&self) -> impl Iterator<Item = LifetimePosition> {
        let mut positions = self.positions().collect::<Vec<_>>();
        positions.sort_by_key(|&p| self.ancestors[p].len());
        positions.into_iter()
    }

    pub fn is_empty(&self) -> bool {
        self.lifetimes.is_empty()
    }

    pub fn lifetime(&self, p: LifetimePosition) -> GeneralizedLifetime<'vir> {
        self.lifetimes[p]
    }

    /// The lifetime of `p` in `ty`, a type whose decomposition this graph is
    /// for but whose regions may differ from the decomposition's: the
    /// decomposition erases inference regions, e.g. those of a body's types.
    /// The type parameter of a `RegionsIn` lifetime is decomposed in `context`.
    pub fn lifetime_in(
        &self,
        p: LifetimePosition,
        mut ty: ty::Ty<'vir>,
        context: GParams<'vir>,
    ) -> GeneralizedLifetime<'vir> {
        for &arg in &self.paths[p] {
            let (_, args) = TyData::<RustTyDatas>::identity_for_ty(ty, false);
            match args[arg].kind() {
                ty::GenericArgKind::Lifetime(region) => return GeneralizedLifetime::Region(region),
                ty::GenericArgKind::Type(arg_ty) => ty = arg_ty,
                ty::GenericArgKind::Const(_) => unreachable!("a path leads through a constant"),
            }
        }
        GeneralizedLifetime::RegionsIn(RustTyDecomposition::from_ty(ty, context))
    }

    /// The lifetime position numbers that `p` covers: `[x, x + 1)` for a
    /// region, `[x, x + arity(T))` for the regions in a type parameter `T`.
    pub fn interval(&self, p: LifetimePosition) -> (vir::ExprInt<'vir>, vir::ExprInt<'vir>) {
        self.intervals[p]
    }

    /// The positions enclosing `p`, transitively, outermost first.
    pub fn ancestors(&self, p: LifetimePosition) -> &[LifetimePosition] {
        &self.ancestors[p]
    }

    /// The positions of the generic argument `arg`, in the order of the
    /// argument type's own positions.
    pub fn arg_positions(&self, arg: usize) -> Vec<LifetimePosition> {
        self.positions()
            .filter(|&p| self.paths[p].first() == Some(&arg))
            .collect()
    }
}

/// The enclosure graph of a type has a cycle between distinct positions, e.g.
/// for `struct S<'a, 'b>(&'a mut &'b mut i32, &'b mut &'a mut i32)`.
#[derive(Debug, Clone)]
pub struct CyclicEnclosure;

impl TaskEncoder for LifetimePositionEnc {
    task_encoder::encoder_cache!(LifetimePositionEnc);
    const ENCODER_NAME: &'static str = "lifetime position encoder";

    type TaskDescription<'vir> = RustTyDecomposition<'vir>;
    type OutputFullDependency<'vir> = EnclosureGraph<'vir>;
    type EncodingError = CyclicEnclosure;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;
        let mut lifetimes = IndexVec::new();
        let mut intervals = IndexVec::new();
        let mut direct: IndexVec<LifetimePosition, Vec<LifetimePosition>> = IndexVec::new();
        let mut paths: IndexVec<LifetimePosition, Vec<usize>> = IndexVec::new();
        match task_key.ty.specifics {
            TySpecifics::ImmRef(_) => {}
            TySpecifics::Param(RustParamData::Generic) => {
                let arity = deps.require_dep::<TyArityEnc>(*task_key)?;
                lifetimes.push(GeneralizedLifetime::RegionsIn(*task_key));
                intervals.push((int(0), arity));
                direct.push(Vec::new());
                paths.push(Vec::new());
            }
            _ => {
                // The positions of each generic argument, by its index.
                let mut arg_positions = Vec::new();
                let mut offset = int(0);
                for (arg_index, arg) in task_key.args.args().iter().enumerate() {
                    let base = direct.len();
                    match arg.kind() {
                        ty::GenericArgKind::Lifetime(region) => {
                            lifetimes.push(GeneralizedLifetime::Region(region));
                            intervals.push((offset, add(offset, int(1))));
                            direct.push(Vec::new());
                            paths.push(vec![arg_index]);
                            offset = add(offset, int(1));
                        }
                        ty::GenericArgKind::Type(arg_ty) => {
                            let arg_ty =
                                RustTyDecomposition::from_ty(arg_ty, task_key.args.context());
                            let arg_graph = deps.require_dep::<Self>(arg_ty)?;
                            let shift =
                                |p: LifetimePosition| LifetimePosition::new(base + p.index());
                            for p in arg_graph.positions() {
                                let (lo, hi) = arg_graph.interval(p);
                                lifetimes.push(arg_graph.lifetime(p));
                                intervals.push((add(offset, lo), add(offset, hi)));
                                direct.push(
                                    arg_graph.ancestors(p).iter().copied().map(shift).collect(),
                                );
                                paths.push(
                                    std::iter::once(arg_index)
                                        .chain(arg_graph.paths[p].iter().copied())
                                        .collect(),
                                );
                            }
                            offset = add(offset, deps.require_dep::<TyArityEnc>(arg_ty)?);
                        }
                        ty::GenericArgKind::Const(_) => {}
                    }
                    arg_positions.push(
                        (base..direct.len())
                            .map(LifetimePosition::new)
                            .collect::<Vec<_>>(),
                    );
                }
                // If a generic argument outlives another one, the positions of
                // the latter enclose those of the former.
                for (longer, shorter) in outlives(*task_key) {
                    for &p in &arg_positions[longer] {
                        direct[p].extend(arg_positions[shorter].iter().copied());
                    }
                }
            }
        }
        let ancestors = transitive_closure(direct)
            .map_err(|error| EncodeFullError::EncodingError(error, None))?;
        Ok((
            (),
            EnclosureGraph {
                lifetimes,
                intervals,
                ancestors,
                paths,
            },
        ))
    }
}

/// The pairs `(longer, shorter)` of generic argument indices of `ty` where `longer` outlives `shorter`.
fn outlives(ty: RustTyDecomposition<'_>) -> Vec<(usize, usize)> {
    // The referent of `&'a mut T` outlives `'a`.
    if let TySpecifics::MutRef(_) = ty.ty.specifics {
        return vec![(1, 0)];
    }
    let Some(adt) = ty.ty.data.adt else {
        return Vec::new();
    };
    // An ADT's parameter indices are its argument indices.
    vir::with_vcx(|vcx| {
        vcx.tcx()
            .inferred_outlives_of(adt)
            .iter()
            .filter_map(|(clause, _)| {
                let (longer, shorter) = match clause.kind().skip_binder() {
                    ty::ClauseKind::RegionOutlives(ty::OutlivesClause(longer, shorter)) => {
                        (early_param_index(longer)?, early_param_index(shorter)?)
                    }
                    ty::ClauseKind::TypeOutlives(ty::OutlivesClause(longer, shorter)) => {
                        let ty::Param(longer) = longer.kind() else {
                            return None;
                        };
                        (longer.index as usize, early_param_index(shorter)?)
                    }
                    _ => return None,
                };
                // A lifetime outlives itself, e.g. `'a` in a field of type `&'a mut Node<'a>`.
                (longer != shorter).then_some((longer, shorter))
            })
            .collect()
    })
}

fn early_param_index(region: ty::Region<'_>) -> Option<usize> {
    match region.kind() {
        ty::ReEarlyParam(param) => Some(param.index as usize),
        _ => None,
    }
}

/// The transitive closure of the direct enclosers `direct` of each position, outermost first.
fn transitive_closure(
    direct: IndexVec<LifetimePosition, Vec<LifetimePosition>>,
) -> Result<IndexVec<LifetimePosition, Vec<LifetimePosition>>, CyclicEnclosure> {
    let mut ancestors = IndexVec::new();
    for p in direct.indices() {
        let mut reached = Vec::new();
        let mut stack = direct[p].clone();
        while let Some(q) = stack.pop() {
            if !reached.contains(&q) {
                reached.push(q);
                stack.extend(direct[q].iter().copied());
            }
        }
        if reached.contains(&p) {
            return Err(CyclicEnclosure);
        }
        ancestors.push(reached);
    }
    // Sort outermost first: a position has fewer ancestors than the positions it encloses.
    let counts = ancestors
        .iter()
        .map(Vec::len)
        .collect::<IndexVec<LifetimePosition, _>>();
    for list in ancestors.iter_mut() {
        list.sort_by_key(|&q| (counts[q], q));
    }
    Ok(ancestors)
}

fn int<'vir>(n: usize) -> vir::ExprInt<'vir> {
    vir::with_vcx(|vcx| {
        vcx.mk_const_expr(vir::ConstData::Int(n as u128))
            .downcast_ty()
    })
}

#[cfg(test)]
mod tests {
    use prusti_rustc_interface::span::DUMMY_SP;
    use task_encoder::{TaskEncoder, TaskEncoderError};

    use prusti_rustc_interface::index::Idx;

    use super::{CyclicEnclosure, GeneralizedLifetime, LifetimePositionEnc};
    use crate::encoders::ty::test_util::{simplified, with_type_alias};

    /// The positions of the type alias `X` defined in `src`, as
    /// `lifetime @ position`, followed by `behind [i, ..]` with the indices
    /// of the positions enclosing it, if any.
    fn positions(src: &str) -> Vec<String> {
        with_type_alias(src, |ty| {
            let (_, _, graph) = LifetimePositionEnc::encode(ty, true, DUMMY_SP)
                .unwrap()
                .unwrap();
            graph
                .positions()
                .map(|p| {
                    let position = simplified(graph.interval(p).0);
                    let lifetime = match graph.lifetime(p) {
                        GeneralizedLifetime::Region(region) => format!("{region}"),
                        GeneralizedLifetime::RegionsIn(param) => {
                            format!("{}", param.args.args()[0])
                        }
                    };
                    let ancestors = graph
                        .ancestors(p)
                        .iter()
                        .map(|q| q.index())
                        .collect::<Vec<_>>();
                    if ancestors.is_empty() {
                        format!("{lifetime} @ {position:?}")
                    } else {
                        format!("{lifetime} @ {position:?} behind {ancestors:?}")
                    }
                })
                .collect()
        })
    }

    /// Whether the positions of the type alias `X` defined in `src` enclose
    /// each other.
    fn is_cyclic(src: &str) -> bool {
        with_type_alias(src, |ty| {
            matches!(
                LifetimePositionEnc::encode(ty, true, DUMMY_SP),
                Err(TaskEncoderError::EncodingError(CyclicEnclosure))
            )
        })
    }

    #[test]
    fn lifetime_after_type_param() {
        assert_eq!(
            positions("type X<'a, T> = (T, &'a mut i32);"),
            ["T @ 0", "'a @ arity(T$1)"],
        );
    }

    #[test]
    fn lifetimes_of_struct() {
        assert_eq!(
            positions(
                "struct Pair<'c, 'd>(&'c mut i32, &'d mut i32);
                 type X<'a, 'b> = Pair<'b, 'a>;"
            ),
            ["'b @ 0", "'a @ 1"],
        );
    }

    #[test]
    fn shared_reference() {
        assert_eq!(positions("type X<'a, 'b> = &'a &'b mut i32;"), [""; 0]);
    }

    #[test]
    fn repeated_lifetime() {
        assert_eq!(
            positions(
                "struct Pair<'c, 'd>(&'c mut i32, &'d mut i32);
                 type X<'a, 'b> = (&'a mut i32, Pair<'a, 'b>);"
            ),
            ["'a @ 0", "'a @ 1", "'b @ 2"],
        );
    }

    #[test]
    fn same_reference_twice() {
        assert_eq!(
            positions("type X<'a, T> = (&'a mut T, &'a mut T);"),
            [
                "'a @ 0",
                "T @ 1 behind [0]",
                "'a @ (1) + (arity(T$1))",
                "T @ (arity(T$1)) + (2) behind [2]",
            ],
        );
    }

    #[test]
    fn nested_mutable_references() {
        assert_eq!(
            positions("type X<'a, 'b> = &'a mut &'b mut i32;"),
            ["'a @ 0", "'b @ 1 behind [0]"],
        );
    }

    #[test]
    fn static_lifetime() {
        assert_eq!(
            positions("type X<T> = &'static mut T;"),
            ["'static @ 0", "T @ 1 behind [0]"],
        );
    }

    #[test]
    fn array() {
        assert_eq!(
            positions("type X<'a, T, const N: usize> = [&'a mut T; N];"),
            ["'a @ 0", "T @ 1 behind [0]"],
        );
    }

    /// A lifetime encloses another one through the definition of a struct.
    #[test]
    fn enclosure_through_definition() {
        assert_eq!(
            positions(
                "struct Node<'a, 'b> { v: &'b mut i32, next: Option<&'a mut Node<'a, 'b>> }
                 type X<'a, 'b> = Node<'a, 'b>;"
            ),
            ["'a @ 0", "'b @ 1 behind [0]"],
        );
    }

    /// An enclosed lifetime may come first.
    #[test]
    fn enclosed_lifetime_first() {
        assert_eq!(
            positions(
                "struct Node<'b, 'a> { v: &'b mut i32, next: Option<&'a mut Node<'b, 'a>> }
                 type X<'a, 'b> = Node<'b, 'a>;"
            ),
            ["'b @ 0 behind [1]", "'a @ 1"],
        );
    }

    /// The enclosure in a type argument combines with that of the type.
    #[test]
    fn enclosure_in_type_argument() {
        assert_eq!(
            positions(
                "struct Node<'a, 'b> { v: &'b mut i32, next: Option<&'a mut Node<'a, 'b>> }
                 type X<'a, 'b> = Option<&'a mut Node<'a, 'b>>;"
            ),
            ["'a @ 0", "'a @ 1 behind [0]", "'b @ 2 behind [0, 1]"],
        );
    }

    /// A struct's enclosure is not that of the structs it contains.
    #[test]
    fn enclosure_of_field() {
        let src = "struct A<'a, 'b>(&'a mut i32, &'b mut i32);
                   struct B<'a, 'b>(A<'a, 'b>, &'a mut &'b mut i32);";
        assert_eq!(
            positions(&format!("{src} type X<'a, 'b> = A<'a, 'b>;")),
            ["'a @ 0", "'b @ 1"],
        );
        assert_eq!(
            positions(&format!("{src} type X<'a, 'b> = B<'a, 'b>;")),
            ["'a @ 0", "'b @ 1 behind [0]"],
        );
    }

    /// A lifetime that outlives another one because of a shared reference is
    /// enclosed by it as well, although no mutable reference encloses it.
    #[test]
    fn shared_reference_outlives() {
        assert_eq!(
            positions(
                "struct S<'a, 'b>(&'a mut i32, &'b mut i32, &'a &'b i32);
                 type X<'a, 'b> = S<'a, 'b>;"
            ),
            ["'a @ 0", "'b @ 1 behind [0]"],
        );
    }

    #[test]
    fn cyclic_enclosure() {
        assert!(is_cyclic(
            "struct S<'a, 'b>(&'a mut &'b mut i32, &'b mut &'a mut i32);
             type X<'a, 'b> = S<'a, 'b>;"
        ));
    }
}
