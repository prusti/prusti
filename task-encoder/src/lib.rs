#![feature(rustc_private)]
#![feature(associated_type_defaults)]

use core::panic;
use prusti_rustc_interface::{
    data_structures::fx::{FxHashSet, FxIndexMap},
    span::Span,
};
use std::cell::RefCell;

mod cache;
mod dependencies;
mod result;
mod triggers;

pub use cache::*;
pub use dependencies::*;
pub use result::*;
pub use triggers::*;

#[derive(Debug, Default)]
pub struct Program<'vir> {
    fields: Vec<vir::FieldDyn<'vir>>,
    adts: Vec<vir::Adt<'vir>>,
    domains: Vec<vir::Domain<'vir>>,
    predicates: Vec<vir::Predicate<'vir>>,
    functions: Vec<vir::Function<'vir>>,
    methods: Vec<vir::Method<'vir>>,
    literal_inverses: Vec<vir::simplify::LiteralInverse<'vir>>,
    /// The names of the adts, domains and functions that encode what the user
    /// writes, which the simplifier never folds away (see `add_user_adt`).
    user_level: FxHashSet<&'vir str>,
    /// The operators with a body and no pre- or postcondition (see
    /// `add_user_operator`). [`Self::all_domains`] makes them the domain
    /// [`OPERATORS_DOMAIN`], placed
    /// after the domains in `domains`.
    operators: Vec<Operator<'vir>>,

    code: Vec<CodeItem>,
    encoder_errors: Vec<(String, Span)>,
}

/// An entry of [`Program::code`], in the order the program was built: a
/// section header, or an item by its index.
#[derive(Debug)]
enum CodeItem {
    Header(String),
    Field(usize),
    Adt(usize),
    Domain(usize),
    Predicate(usize),
    Function(usize),
    Method(usize),
    /// The domain [`OPERATORS_DOMAIN`].
    Operators,
}

/// An operator with a body and no pre- or postcondition, as a domain function
/// and the axiom that defines it (see [`Program::add_user_operator`]).
#[derive(Debug)]
struct Operator<'vir> {
    function: vir::DomainFunction<'vir>,
    definition: vir::DomainAxiom<'vir>,
}

impl<'vir> Operator<'vir> {
    /// `function`, whose body is `body`, as a domain function with the same
    /// name, arguments and `no_auto_trigger`, defined by the axiom
    /// `forall args :: { function(args) } function(args) == body`.
    fn new(function: vir::Function<'vir>, body: vir::ExprDyn<'vir>) -> Self {
        vir::with_vcx(|vcx| {
            let name = vir::ViperIdent::new(function.name);
            let tys = vcx.alloc_slice(&function.args.iter().map(|arg| arg.ty).collect::<Vec<_>>());
            let idn = vir::FunctionIdn::<vir::ManyDyn, _>::new(name, tys, function.ret);
            let args = function
                .args
                .iter()
                .map(|&arg| vcx.mk_local_ex(arg))
                .collect::<Vec<vir::ExprDyn<'vir>>>();
            let app = idn.call()(&args);
            let definition = vcx.mk_domain_axiom(
                vir::ViperIdent::new(vcx.alloc_str(&format!("{}_definition", function.name))),
                vcx.mk_forall_expr(
                    function.args,
                    vcx.alloc_slice(&[vcx.mk_trigger(&[app])]),
                    vcx.mk_eq_expr(app, body),
                ),
            );
            let function = vcx.alloc(vir::DomainFunctionData {
                unique: false,
                name,
                args: tys,
                ret: function.ret,
                interpretation: None,
                no_auto_trigger: function.no_auto_trigger,
            });
            Operator {
                function,
                definition,
            }
        })
    }
}

/// The name of the domain of the [`Operator`]s.
const OPERATORS_DOMAIN: &str = "operators";

impl<'vir> Program<'vir> {
    pub fn header(&mut self, title: &str) {
        self.code.push(CodeItem::Header(title.to_string()));
    }

    pub fn add_field(&mut self, field: vir::FieldDyn<'vir>) {
        self.code.push(CodeItem::Field(self.fields.len()));
        self.fields.push(field);
    }

    pub fn add_adt(&mut self, adt: vir::Adt<'vir>) {
        self.code.push(CodeItem::Adt(self.adts.len()));
        self.adts.push(adt);
    }

    pub fn add_domain(&mut self, domain: vir::Domain<'vir>) {
        self.code.push(CodeItem::Domain(self.domains.len()));
        self.domains.push(domain);
    }

    /// Adds an adt whose constructors and fields encode what the user writes,
    /// such as the snapshot of a Rust struct. [`Self::simplify`] never folds
    /// away an application of such a user-level adt, domain or function, so
    /// that a trigger of a quantifier the user wrote still finds the terms the
    /// user wrote. What rustc adds to the MIR, or the encoding introduces, is
    /// not user-level and may be folded away.
    ///
    /// - User-level: the snapshots of structs, enums, arrays and slices, pure
    ///   functions, operators, constants, ghost collection operations and
    ///   value casts, which come from `as`.
    /// - Not user-level: references, boxes, raw pointers, closures and
    ///   tuples, which rustc and Prusti build and take apart themselves (e.g.
    ///   auto-refs, the arguments of a closure call, the trigger sets of a
    ///   quantifier); the snapshots of primitives and generic parameters,
    ///   which wrap values for the encoding; and pointer coercions, which are
    ///   mostly implicit (one written with `as` is encoded as the same
    ///   function).
    pub fn add_user_adt(&mut self, adt: vir::Adt<'vir>) {
        self.user_level.insert(adt.name);
        self.add_adt(adt);
    }

    /// Adds a domain whose functions are user-level (see
    /// [`Self::add_user_adt`]).
    pub fn add_user_domain(&mut self, domain: vir::Domain<'vir>) {
        self.user_level.insert(domain.name);
        self.add_domain(domain);
    }

    /// Adds a user-level function (see [`Self::add_user_adt`]).
    pub fn add_user_function(&mut self, function: vir::Function<'vir>) {
        self.user_level.insert(function.name);
        self.add_function(function);
    }

    /// Adds a user-level function that encodes an operator or a cast (see
    /// [`Self::add_user_adt`]), marked `no_auto_trigger`: a trigger on the
    /// result of `i - 1` matches the subtraction that a recursive function
    /// performs, and can instantiate its quantifier without end.
    ///
    /// An operator with a body but no pre- or postcondition is added instead
    /// as a domain function of the user-level domain [`OPERATORS_DOMAIN`],
    /// with an axiom for its body (see [`Operator::new`]). Silicon applies the body of
    /// a function only to the applications it has evaluated, which in a
    /// quantifier are those a trigger matches; an axiom applies to every
    /// application.
    pub fn add_user_operator(&mut self, function: vir::Function<'vir>) {
        vir::with_vcx(|vcx| {
            let function = vcx.alloc(vir::FunctionGenData {
                no_auto_trigger: true,
                ..*function
            });
            let (Some(body), [], []) = (function.expr, function.pres, function.posts) else {
                self.add_user_function(function);
                return;
            };
            if self.operators.is_empty() {
                self.user_level.insert(OPERATORS_DOMAIN);
                self.code.push(CodeItem::Operators);
            }
            self.operators.push(Operator::new(function, body));
        });
    }

    /// The domain [`OPERATORS_DOMAIN`], if an operator was added to it.
    fn operators_domain(&self) -> Option<vir::Domain<'vir>> {
        if self.operators.is_empty() {
            return None;
        }
        let (functions, definitions): (Vec<_>, Vec<_>) = self
            .operators
            .iter()
            .map(|op| (op.function, op.definition))
            .unzip();
        Some(vir::with_vcx(|vcx| {
            vcx.mk_domain(
                vir::ViperIdent::new(OPERATORS_DOMAIN),
                &[],
                vcx.alloc_slice(&definitions),
                vcx.alloc_slice(&functions),
                None,
            )
        }))
    }

    /// The domains of the program: those in `domains`, then the domain
    /// [`OPERATORS_DOMAIN`].
    fn all_domains(&self) -> Vec<vir::Domain<'vir>> {
        self.domains
            .iter()
            .copied()
            .chain(self.operators_domain())
            .collect()
    }

    /// Declares a literal inverse, which [`Self::simplify`] then folds.
    pub fn add_literal_inverse(&mut self, literal_inverse: vir::simplify::LiteralInverse<'vir>) {
        self.literal_inverses.push(literal_inverse);
    }

    pub fn add_predicate(&mut self, predicate: vir::Predicate<'vir>) {
        self.code.push(CodeItem::Predicate(self.predicates.len()));
        self.predicates.push(predicate);
    }

    pub fn add_function(&mut self, function: vir::Function<'vir>) {
        self.code.push(CodeItem::Function(self.functions.len()));
        self.functions.push(function);
    }

    pub fn add_method(&mut self, method: vir::Method<'vir>) {
        self.code.push(CodeItem::Method(self.methods.len()));
        self.methods.push(method);
    }

    /// The program in Viper syntax, with section headers.
    pub fn code(&self) -> String {
        let mut code = String::new();
        for item in &self.code {
            match item {
                CodeItem::Header(title) => {
                    code.push_str("// -----------------------------\n");
                    code.push_str(&format!("// {title}\n"));
                    code.push_str("// -----------------------------\n");
                }
                CodeItem::Field(idx) => code.push_str(&format!("{:?}\n", self.fields[*idx])),
                CodeItem::Adt(idx) => code.push_str(&format!("{:?}\n", self.adts[*idx])),
                CodeItem::Domain(idx) => code.push_str(&format!("{:?}\n", self.domains[*idx])),
                CodeItem::Predicate(idx) => {
                    code.push_str(&format!("{:?}\n", self.predicates[*idx]))
                }
                CodeItem::Function(idx) => code.push_str(&format!("{:?}\n", self.functions[*idx])),
                CodeItem::Method(idx) => code.push_str(&format!("{:?}\n", self.methods[*idx])),
                CodeItem::Operators => {
                    if let Some(domain) = self.operators_domain() {
                        code.push_str(&format!("{domain:?}\n"));
                    }
                }
            }
        }
        code
    }

    /// Replaces the `let`s in the triggers of all items added so far by the
    /// values they bind (see [`vir::inline_trigger_lets`]).
    pub fn inline_trigger_lets(&mut self) {
        vir::with_vcx(|vcx| self.fold(|program| vir::inline_trigger_lets(vcx, program)));
    }

    /// Simplifies the expressions of all domain axioms, predicates, functions
    /// and methods added so far (see [`vir::simplify`]).
    pub fn simplify(&mut self) {
        vir::with_vcx(|vcx| {
            let ctx = vir::simplify::SimplifyCtx::new(
                &self.adts,
                &self.all_domains(),
                &self.functions,
                &self.literal_inverses,
                &self.user_level,
            );
            self.fold(|program| vir::simplify::simplify(vcx, &ctx, program));
        });
    }

    /// Applies `f` to the program made of the items added so far and keeps
    /// the items of the result. `f` must keep every item at its index, as a
    /// fold does, since the field `code` refers to items by index.
    fn fold(&mut self, f: impl FnOnce(vir::Program<'vir>) -> vir::Program<'vir>) {
        let program = f(self.mk_program());
        self.fields = program.fields.to_vec();
        self.adts = program.adts.to_vec();
        let (domains, operators) = program.domains.split_at(self.domains.len());
        self.domains = domains.to_vec();
        if let [operators] = operators {
            self.operators = (operators.functions.iter().zip(operators.axioms))
                .map(|(&function, &definition)| Operator {
                    function,
                    definition,
                })
                .collect();
        }
        self.predicates = program.predicates.to_vec();
        self.functions = program.functions.to_vec();
        self.methods = program.methods.to_vec();
    }

    pub fn mk_program(&self) -> vir::Program<'vir> {
        vir::with_vcx(|vcx| {
            vcx.mk_program(
                vcx.alloc_slice(&self.fields),
                vcx.alloc_slice(&self.adts),
                vcx.alloc_slice(&self.all_domains()),
                vcx.alloc_slice(&self.predicates),
                vcx.alloc_slice(&self.functions),
                vcx.alloc_slice(&self.methods),
            )
        })
    }

    pub fn encoder_errors(&mut self) -> &mut Vec<(String, Span)> {
        &mut self.encoder_errors
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NeverError {}

pub trait OutputRefAny {}
impl OutputRefAny for () {}

pub trait TaskEncoder {
    /// Description of a task to be performed. Should be easily obtained by
    /// clients of this encoder.
    type TaskDescription<'vir>: std::hash::Hash + Eq + Clone + std::fmt::Debug;

    /// Cache key for a task to be performed. May differ from `TaskDescription`,
    /// for example if the description should be normalised or some non-trivial
    /// resolution needs to happen. In other words, multiple descriptions may
    /// lead to the same key and hence the same output.
    type TaskKey<'vir>: std::hash::Hash + Eq + Clone + std::fmt::Debug =
        Self::TaskDescription<'vir>;

    /// A reference to an encoded item. Should be non-unit for tasks which can
    /// be "referred" to from other parts of a program, as opposed to tasks
    /// where only the full output matters.
    type OutputRef<'vir>: Clone + std::fmt::Debug + OutputRefAny
        = ()
    where
        Self: 'vir;

    /// Fully encoded output for this task. When encoding items which can be
    /// dependencies (such as methods), this output should only be emitted in
    /// one Viper program.
    type OutputFullLocal<'vir>: Clone
        = ()
    where
        Self: 'vir;

    /// Fully encoded output for this task for dependents. When encoding items
    /// which can be dependencies (such as methods), this output should be
    /// emitted in each Viper program that depends on this task.
    type OutputFullDependency<'vir>: Clone
        = ()
    where
        Self: 'vir;

    type EnqueueingError: Clone + std::fmt::Debug = NeverError;
    type EncodingError: Clone + std::fmt::Debug = NeverError;

    /// User-presentable name of this encoder.
    const ENCODER_NAME: &'static str;

    fn describe_task<'vir>(task: Self::TaskDescription<'vir>) -> String {
        format!("{task:?}")
    }

    fn describe_error(error: Self::EncodingError) -> String {
        format!("{error:?}")
    }

    /// Enters the given function with a reference to the cache for this
    /// encoder.
    fn with_cache<'vir, F, R>(f: F) -> R
    where
        Self: 'vir,
        F: FnOnce(&'vir CacheRef<'vir, Self>) -> R;

    /// Enters the given function with a reference to the trigger watchers for
    /// this encoder. These are continuations registered via [`Self::on_task_requested`],
    /// waiting for a task of this encoder to be requested.
    fn with_watchers<'vir, F, R>(f: F) -> R
    where
        Self: 'vir,
        F: FnOnce(&'vir WatchersRef<'vir, Self>) -> R;

    /// Queues `f` to run in [`drain_triggers`] once the given task of this
    /// encoder has been requested (enqueued, started, or encoded - not
    /// necessarily successfully). If it already has been, `f` is queued
    /// immediately. Continuations of tasks that are never requested do not
    /// run.
    ///
    /// Triggers must be registered during the encoding phase:
    /// [`drain_triggers`] runs before outputs are emitted, so continuations
    /// registered during `emit_outputs` would never run. From within an
    /// in-flight encoder that already knows the task is needed, use a plain
    /// `deps.require_*` instead.
    fn on_task_requested<'vir>(key: Self::TaskKey<'vir>, f: impl FnOnce() + 'vir)
    where
        Self: Sized + 'vir,
    {
        let f = mk_continuation(f);
        let requested = Self::with_cache(|cache| cache.borrow().contains_key(&key));
        if requested {
            triggers::queue(f);
        } else {
            Self::with_watchers(move |watchers| {
                watchers.borrow_mut().entry(key).or_default().push(f)
            });
        }
    }

    /// Queues `f` to run once *every* task in `keys` has been requested (the
    /// conjunction of [`Self::on_task_requested`]). An empty `keys` runs `f`
    /// unconditionally.
    fn on_all_requested<'vir>(mut keys: Vec<Self::TaskKey<'vir>>, f: impl FnOnce() + 'vir)
    where
        Self: Sized + 'vir,
    {
        match keys.pop() {
            None => queue(mk_continuation(f)),
            Some(key) => Self::on_task_requested(key, move || Self::on_all_requested(keys, f)),
        }
    }

    /// Queues `f` to run once *any* task in `keys` has been requested (the
    /// disjunction of [`Self::on_task_requested`]). `f` runs at most once, on
    /// the first such task; an empty `keys` never runs `f`.
    fn on_any_requested<'vir>(keys: Vec<Self::TaskKey<'vir>>, f: impl FnOnce() + 'vir)
    where
        Self: Sized + 'vir,
    {
        // The continuations share ownership of `f` and race to take it, so that
        // it runs exactly once regardless of how many keys become requested.
        let slot: std::rc::Rc<std::cell::Cell<Option<triggers::Continuation>>> =
            std::rc::Rc::new(std::cell::Cell::new(Some(mk_continuation(f))));
        for key in keys {
            let slot = slot.clone();
            Self::on_task_requested(key, move || {
                if let Some(f) = slot.take() {
                    f();
                }
            });
        }
    }

    //fn get_all_outputs() -> Self::CacheRef<'vir> {
    //  todo!()
    //  // while ... { process() } // process items in the queue
    //  //Self::cache()
    //}

    /*
    fn enqueue<'vir>(task: Self::TaskDescription<'vir>)
    where
        Self: 'vir,
    {
        let task_key = Self::task_to_key(&task);
        let task_key_clone = task_key.clone(); // TODO: remove?

        if Self::with_cache(move |cache| cache.borrow().contains_key(&task_key_clone)) {
            return;
        }

        // enqueue, expecting no entry (we just checked)
        triggers::fire_watchers::<Self>(&task_key);
        let old = Self::with_cache(move |cache| {
            let v = TaskEncoderCacheState::Enqueued;
            cache.borrow_mut().insert(task_key, v)
        });
        assert!(old.is_none());
    }
    */

    fn encode_ref<'vir>(
        task: Self::TaskDescription<'vir>,
        span: Span,
    ) -> Result<Self::OutputRef<'vir>, TaskEncoderError<Self>>
    where
        Self: 'vir,
    {
        let task_key = Self::task_to_key(&task);

        // is there an output ref available already?
        if let Some(output_ref) = cached_output_ref::<Self>(&task_key) {
            return Ok(output_ref);
        }

        // Otherwise, we need to start the encoding. Note that this is done
        // even if the encoding was started previously, i.e. if the same task
        // was (recursively) requested from the same encoder before its first
        // invocation reached a call to `emit_output_ref` (see
        // `TaskEncoderCacheState::Encoding`).
        match Self::encode(task, false, span) {
            // A successful encoding has emitted the output ref.
            Ok(_) => {
                Ok(cached_output_ref::<Self>(&task_key)
                    .expect("output ref not found after encoding"))
            }
            // The output ref may have been emitted before the error. A
            // requester then gets it, as later requesters do from the cache,
            // and fails only if it needs the part that could not be encoded.
            Err(err @ (TaskEncoderError::DependencyError(..) | TaskEncoderError::Reported(..))) => {
                cached_output_ref::<Self>(&task_key).ok_or(err)
            }
            Err(err) => Err(err),
        }
    }

    fn encode<'vir>(
        task: Self::TaskDescription<'vir>,
        need_output: bool,
        span: Span,
    ) -> EncodeResult<'vir, Self>
    where
        Self: 'vir,
    {
        let task_key = Self::task_to_key(&task);

        let in_cache = Self::with_cache(|cache| {
            let mut cache = cache.borrow_mut();

            let new_state = match cache.get(&task_key) {
                Some(
                    TaskEncoderCacheState::ErrorBeforeRef { failure }
                    | TaskEncoderCacheState::ErrorEncode { failure, .. },
                ) => return Some(Err(failure.error.clone())),
                Some(TaskEncoderCacheState::Encoded {
                    output_ref,
                    output_local,
                    output_dep,
                    ..
                }) => {
                    return if need_output {
                        Some(Ok(Some((
                            output_ref.clone(),
                            output_local.clone(),
                            output_dep.clone(),
                        ))))
                    } else {
                        Some(Ok(None))
                    };
                }
                // A re-run is already in progress; another one could not make
                // progress over it (see `TaskEncoderCacheState`).
                Some(
                    TaskEncoderCacheState::ReEncoding | TaskEncoderCacheState::Restarted { .. },
                ) => return Some(Err(TaskEncoderError::CyclicError)),
                // Start the first run.
                None => {
                    triggers::fire_watchers::<Self>(&task_key);
                    TaskEncoderCacheState::Encoding
                }
                // In progress: re-run the encoding (see
                // `TaskEncoderCacheState::Started`).
                Some(TaskEncoderCacheState::Encoding) => TaskEncoderCacheState::ReEncoding,
                Some(TaskEncoderCacheState::Started { output_ref }) => {
                    TaskEncoderCacheState::Restarted {
                        output_ref: output_ref.clone(),
                    }
                }
            };
            cache.insert(task_key.clone(), new_state);
            None
        });
        if let Some(in_cache) = in_cache {
            return in_cache;
        }

        let value = task_key.clone();
        // The span stack is isolated across the encoder-context boundary: the
        // encoding is demand-driven and cached, so its spans must not chain
        // up to whatever span is ambient at the (first) demand site.
        let catch_result = vir::with_vcx(|vcx| {
            vcx.with_span_stack_isolated(|| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    let mut deps = TaskEncoderDependencies::new();
                    let encode_result = Self::do_encode_full(&value, &mut deps);
                    (encode_result, deps)
                }))
            })
        });

        let (encode_result, deps) = catch_result.map_err(|panic_payload| {
            // There was a panic within the encoder. We want to report it
            // and return an error to the caller.
            let msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "<unknown panic>".to_string()
            };
            let error = TaskEncoderError::PanicError(msg);
            record_error::<Self>(
                task_key.clone(),
                error.clone(),
                TaskEncoderDependencies::new(),
                None,
                span,
            );
            error
        })?;

        match encode_result {
            Ok((output_local, output_dep)) => {
                let output_ref = cached_output_ref::<Self>(&task_key).unwrap_or_else(|| {
                    panic!("encoder did not provide output ref for task {task_key:?}")
                });
                if need_output {
                    Self::with_cache(|cache| {
                        cache.borrow_mut().insert(
                            task_key,
                            TaskEncoderCacheState::Encoded {
                                output_ref: output_ref.clone(),
                                deps,
                                output_local: output_local.clone(),
                                output_dep: output_dep.clone(),
                            },
                        )
                    });
                    Ok(Some((output_ref, output_local, output_dep)))
                } else {
                    Self::with_cache(|cache| {
                        cache.borrow_mut().insert(
                            task_key,
                            TaskEncoderCacheState::Encoded {
                                output_ref,
                                deps,
                                output_local,
                                output_dep,
                            },
                        )
                    });
                    Ok(None)
                }
            }
            Err(EncodeFullError::AlreadyEncoded) => {
                Self::with_cache(|cache| match cache.borrow().get(&task_key).unwrap() {
                    TaskEncoderCacheState::Encoded {
                        output_ref,
                        output_local,
                        output_dep,
                        ..
                    } => {
                        if need_output {
                            Ok(Some((
                                // TODO: does it even make sense for an encoder to request the full encoding
                                //   when a cycle can occur?
                                output_ref.clone(),
                                output_local.clone(),
                                output_dep.clone(),
                            )))
                        } else {
                            Ok(None)
                        }
                    }
                    TaskEncoderCacheState::ErrorBeforeRef { failure }
                    | TaskEncoderCacheState::ErrorEncode { failure, .. } => {
                        Err(failure.error.clone())
                    }
                    TaskEncoderCacheState::Encoding
                    | TaskEncoderCacheState::ReEncoding
                    | TaskEncoderCacheState::Started { .. }
                    | TaskEncoderCacheState::Restarted { .. } => {
                        panic!("encoder did not finish for task {task_key:?}")
                    }
                })
            }
            Err(EncodeFullError::DependencyError(stack)) => {
                let owned_stack = std::iter::once(DependencyErrorLink::passing_on(
                    Self::ENCODER_NAME,
                    Self::describe_task(task),
                    Vec::new(),
                    &stack,
                ))
                .chain(stack)
                .collect::<Vec<_>>();
                record_error::<Self>(
                    task_key,
                    TaskEncoderError::DependencyError(owned_stack.clone()),
                    deps,
                    None,
                    span,
                );
                Err(TaskEncoderError::DependencyError(owned_stack))
            }
            Err(EncodeFullError::EncodingError(err, maybe_output_dep)) => {
                record_error::<Self>(
                    task_key,
                    TaskEncoderError::EncodingError(err.clone()),
                    deps,
                    maybe_output_dep.map(|dep| *dep),
                    span,
                );
                Err(TaskEncoderError::EncodingError(err))
            }
            Err(EncodeFullError::Reported(reported)) => {
                record_error::<Self>(
                    task_key,
                    TaskEncoderError::Reported(reported),
                    deps,
                    None,
                    span,
                );
                Err(TaskEncoderError::Reported(reported))
            }
        }
    }
    /*
        /// Given a task description for this encoder, enqueue it and return the
        /// reference to the output. If the task is already enqueued, the output
        /// reference already exists.
        fn encode<'vir>(task: Self::TaskDescription<'vir>) -> Self::OutputRef<'vir>
            where Self: 'vir
        {
            let task_key = Self::task_to_key(&task);
            let task_key_clone = task_key.clone();
            if let Some(output_ref) = Self::with_cache(move |cache| match cache.borrow().get(&task_key_clone) {
                Some(TaskEncoderCacheState::Enqueued { output_ref })
                | Some(TaskEncoderCacheState::Started { output_ref, .. })
                | Some(TaskEncoderCacheState::Encoded { output_ref, .. })
                | Some(TaskEncoderCacheState::ErrorEncode { output_ref, .. }) => Some(output_ref.clone()),
                _ => None,
            }) {
                return output_ref;
            }
            let task_ref = Self::task_to_output_ref(&task);
            let task_key_clone = task_key.clone();
            let task_ref_clone = task_ref.clone();
            assert!(Self::with_cache(move |cache| cache.borrow_mut().insert(
                task_key_clone,
                TaskEncoderCacheState::Enqueued { output_ref: task_ref_clone },
            ).is_none()));
            task_ref
        }

        // TODO: this function should not be needed
        fn encode_eager<'vir>(task: Self::TaskDescription<'vir>) -> Result<(
            Self::OutputRef<'vir>,
            Self::OutputFullLocal<'vir>,
            Self::OutputFullDependency<'vir>,
        ), TaskEncoderError<Self>>
            where Self: 'vir
        {
            let task_key = Self::task_to_key(&task);
            // enqueue
            let output_ref = Self::encode(task);
            // process
            Self::encode_full(task_key)
                .map(|(output_full_local, output_full_dep)| (output_ref, output_full_local, output_full_dep))
        }

        /// Given a task key, fully encode the given task. If this task was already
        /// finished, the encoding is not repeated. If this task was enqueued, but
        /// not finished, return a `CyclicError`.
        fn encode_full<'vir>(task_key: Self::TaskKey<'vir>) -> Result<(
            Self::OutputFullLocal<'vir>,
            Self::OutputFullDependency<'vir>,
        ), TaskEncoderError<Self>>
            where Self: 'vir
        {
            let mut output_ref_opt = None;
            let ret = Self::with_cache(|cache| {
                // should be queued by now
                match cache.borrow().get(&task_key).unwrap() {
                    TaskEncoderCacheState::Enqueued { output_ref } => {
                        output_ref_opt = Some(output_ref.clone());
                        None
                    }
                    TaskEncoderCacheState::Started { .. } => Some(Err(TaskEncoderError::CyclicError)),
                    TaskEncoderCacheState::Encoded { output_local, output_dep, .. } =>
                        Some(Ok((
                            output_local.clone(),
                            output_dep.clone(),
                        ))),
                    TaskEncoderCacheState::ErrorEncode { error, .. } =>
                        Some(Err(error.clone())),
                }
            });
            if let Some(ret) = ret {
                return ret;
            }
            let output_ref = output_ref_opt.unwrap();

            let mut deps: TaskEncoderDependencies<'vir> = Default::default();
            match Self::do_encode_full(&task_key, &mut deps) {
                Ok((output_local, output_dep)) => {
                    Self::with_cache(|cache| cache.borrow_mut().insert(task_key, TaskEncoderCacheState::Encoded {
                        output_ref: output_ref.clone(),
                        deps,
                        output_local: output_local.clone(),
                        output_dep: output_dep.clone(),
                    }));
                    Ok((
                        output_local,
                        output_dep,
                    ))
                }
                Err((err, maybe_output_dep)) => {
                    Self::with_cache(|cache| cache.borrow_mut().insert(task_key, TaskEncoderCacheState::ErrorEncode {
                        output_ref: output_ref.clone(),
                        deps,
                        error: TaskEncoderError::EncodingError(err.clone()),
                        output_dep: maybe_output_dep,
                    }));
                    Err(TaskEncoderError::EncodingError(err))
                }
            }
        }
    */
    /// Given a task description, create a key for storing it in the cache.
    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir>;
    /*
        /// Given a task description, create a reference to the output.
        fn task_to_output_ref<'vir>(task: &Self::TaskDescription<'vir>) -> Self::OutputRef<'vir>;
    */
    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self>;

    /// The local outputs of the successfully encoded tasks of this encoder.
    /// The failures of the other tasks that have not been reported (see
    /// [ErrorReport]) are added to `program` as encoder errors.
    fn all_outputs_local<'vir>(program: &mut Program<'vir>) -> Vec<Self::OutputFullLocal<'vir>>
    where
        Self: 'vir,
    {
        Self::with_cache(|cache| {
            let mut outputs = Vec::new();
            for (key, cache_state) in cache.borrow().iter() {
                if let TaskEncoderCacheState::Encoded { output_local, .. } = cache_state {
                    outputs.push(output_local.clone());
                    continue;
                }
                let failure = cache_state
                    .failure()
                    .unwrap_or_else(|| panic!("task encoder not completed: {key:?}"));
                if failure.report.is_marked() {
                    continue;
                }
                let span = failure
                    .spans
                    .iter()
                    .copied()
                    .find(|span| !span.is_dummy())
                    .unwrap_or(prusti_rustc_interface::span::DUMMY_SP);
                let msg = match &failure.error {
                    TaskEncoderError::EncodingError(err) => Self::describe_error(err.clone()),
                    other => format!(
                        "encoder '{}' failed to encode {:?}:\n {:?}",
                        Self::ENCODER_NAME,
                        key,
                        other
                    ),
                };
                program.encoder_errors.push((msg, span));
            }
            outputs
        })
    }

    fn emit_outputs<'vir>(_program: &mut Program<'vir>) {}
}

/// The task's output ref, if its encoding (or a re-run of it) emitted one.
fn cached_output_ref<'vir, E: TaskEncoder + 'vir + ?Sized>(
    task_key: &E::TaskKey<'vir>,
) -> Option<E::OutputRef<'vir>> {
    E::with_cache(|cache| {
        let cache = cache.borrow();
        cache
            .get(task_key)
            .and_then(TaskEncoderCacheState::output_ref)
            .cloned()
    })
}

/// Records that encoding the task failed with `error`. If the encoding (or a
/// re-run of it) emitted an output ref, the ref stays available to dependents
/// (`ErrorEncode`); otherwise the task has none (`ErrorBeforeRef`). The spans
/// of an error recorded earlier for the task are kept.
fn record_error<'vir, E: TaskEncoder + 'vir + ?Sized>(
    task_key: E::TaskKey<'vir>,
    error: TaskEncoderError<E>,
    deps: TaskEncoderDependencies<'vir, E>,
    output_dep: Option<E::OutputFullDependency<'vir>>,
    span: Span,
) {
    let output_ref = cached_output_ref::<E>(&task_key);
    E::with_cache(|cache| {
        let mut cache = cache.borrow_mut();
        let mut spans = cache
            .get(&task_key)
            .and_then(TaskEncoderCacheState::failure)
            .map_or_else(Vec::new, |failure| failure.spans.clone());
        spans.push(span);
        let report = match error {
            TaskEncoderError::Reported(reported) => ErrorReport::reported(reported),
            _ => ErrorReport::default(),
        };
        let failure = TaskFailure {
            error,
            spans,
            report,
        };
        let new_state = match output_ref {
            Some(output_ref) => TaskEncoderCacheState::ErrorEncode {
                output_ref,
                deps,
                output_dep,
                failure,
            },
            None => TaskEncoderCacheState::ErrorBeforeRef { failure },
        };
        cache.insert(task_key, new_state);
    });
}
