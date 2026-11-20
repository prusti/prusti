use prusti_rustc_interface::span::Span;

use super::*;

/// The result of an `encode` call.
pub type EncodeResult<'vir, E /*: TaskEncoder + 'vir + ?Sized*/> = Result<
    Option<(
        <E as TaskEncoder>::OutputRef<'vir>,
        <E as TaskEncoder>::OutputFullLocal<'vir>,
        <E as TaskEncoder>::OutputFullDependency<'vir>,
    )>,
    TaskEncoderError<E>,
>;

/// The result of the actual encoder implementation (`do_encode_full`).
pub type EncodeFullResult<'vir, E /*: TaskEncoder + 'vir + ?Sized*/> = Result<
    (
        <E as TaskEncoder>::OutputFullLocal<'vir>,
        <E as TaskEncoder>::OutputFullDependency<'vir>,
    ),
    EncodeFullError<'vir, E>,
>;

/// Records whether the failure of a task has been reported to the user.
///
/// The dependency error that passes the failure on to a dependent refers to
/// the record. A dependent that reports a dependency error marks the records
/// it refers to (see [EncodeFullError::mark_reported]). The final error
/// collection reports each failure whose record is not marked (see
/// [TaskEncoder::all_outputs_local_no_errors]), so a failure is only left out
/// once it was reported, and a dependent that drops the error cannot hide it.
#[derive(Clone, Debug, Default)]
pub struct ErrorReport(std::rc::Rc<std::cell::OnceCell<vir::Reported>>);

impl ErrorReport {
    /// The record of a failure that was reported when it occurred.
    pub(crate) fn reported(reported: vir::Reported) -> Self {
        let report = Self::default();
        report.mark(reported);
        report
    }

    fn mark(&self, reported: vir::Reported) {
        // Marking it once is enough to prove that the failure was reported.
        let _ = self.0.set(reported);
    }

    pub(crate) fn is_marked(&self) -> bool {
        self.0.get().is_some()
    }
}

/// One link of a dependency error: the encoder, the task it was encoding (or,
/// for the last link, the error itself), the spans involved, optionally
/// advice for the user on how to avoid the error, and, for the last link,
/// whether the error is a mistake in the user's program rather than a
/// limitation of the encoder, and whether it is an internal error (a panic, a
/// cycle or an enqueueing error), which is reported as such, also when
/// unsupported features are only warnings. The link for the failed dependency
/// itself, the first of the chain, refers to the record of that failure.
#[derive(Clone)]
pub struct DependencyErrorLink {
    pub encoder: &'static str,
    pub message: String,
    pub spans: Vec<Span>,
    pub help: Option<String>,
    pub incorrect: bool,
    pub internal: bool,
    pub(crate) report: Option<ErrorReport>,
}

// Manual implementation, so that the record (whose state depends on what has
// been reported so far) does not show up in error messages.
impl std::fmt::Debug for DependencyErrorLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DependencyErrorLink")
            .field("encoder", &self.encoder)
            .field("message", &self.message)
            .field("spans", &self.spans)
            .field("help", &self.help)
            .field("incorrect", &self.incorrect)
            .field("internal", &self.internal)
            .finish()
    }
}

impl DependencyErrorLink {
    pub fn new(encoder: &'static str, message: String, spans: Vec<Span>) -> Self {
        Self {
            encoder,
            message,
            spans,
            help: None,
            incorrect: false,
            internal: false,
            report: None,
        }
    }

    #[must_use]
    pub fn with_help(mut self, help: String) -> Self {
        self.help = Some(help);
        self
    }

    /// Marks the error as a mistake in the user's program, such as a
    /// specification that calls an impure function.
    #[must_use]
    pub fn incorrect(mut self) -> Self {
        self.incorrect = true;
        self
    }

    /// Marks the error as an internal error.
    #[must_use]
    pub(crate) fn internal(mut self) -> Self {
        self.internal = true;
        self
    }
}

/// An unsuccessful result occurring in `do_encode_full`.
pub enum EncodeFullError<'vir, E: TaskEncoder + 'vir + ?Sized> {
    /// Indicates that the current task has already been encoded. This can
    /// occur when there are cyclic dependencies between multiple encoders.
    /// This error is specifically returned when one encoder depends on
    /// another encoder (using e.g. `TaskEncoderDependencies::require_ref`),
    /// that latter encoder then depending on the former again, causing the
    /// former encoder to complete its full encoding in the inner invocation.
    /// The outer invocation remains on the stack, but will be aborted early
    /// as soon as the control flow returns to it.
    AlreadyEncoded,

    /// An actual error occurred during encoding.
    EncodingError(
        <E as TaskEncoder>::EncodingError,
        Option<Box<E::OutputFullDependency<'vir>>>,
    ),

    DependencyError(Vec<DependencyErrorLink>),

    /// The task cannot be encoded, and this was already reported to the user.
    /// It is passed on to the encoders that depend on the task, so that they
    /// do not report it again.
    Reported(vir::Reported),
}

impl<'vir, E: TaskEncoder + 'vir + ?Sized> EncodeFullError<'vir, E> {
    /// The error for a feature that `E` does not support, which `message`
    /// describes, at `spans`.
    pub fn unsupported(message: String, spans: Vec<Span>) -> Self {
        Self::DependencyError(vec![DependencyErrorLink::new(
            E::ENCODER_NAME,
            message,
            spans,
        )])
    }

    /// Records that this error has been reported, as `reported` proves: the
    /// failures of the dependencies it contains are then not reported again.
    pub fn mark_reported(&self, reported: vir::Reported) {
        if let Self::DependencyError(chain) = self {
            for report in chain.iter().filter_map(|link| link.report.as_ref()) {
                report.mark(reported);
            }
        }
    }
}

// Manual implementation, since neither `E` nor `E::OutputFullDependency` are
// required to be `Debug`.
impl<'vir, E: TaskEncoder + 'vir + ?Sized> std::fmt::Debug for EncodeFullError<'vir, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyEncoded => write!(f, "AlreadyEncoded"),
            Self::EncodingError(err, _output_dep) => f
                .debug_tuple("EncodingError")
                .field(err) /*.field(output_dep)*/
                .finish(),
            Self::DependencyError(contents) => write!(f, "DependencyError({contents:?})"),
            Self::Reported(reported) => write!(f, "Reported({reported:?})"),
        }
    }
}

pub enum TaskEncoderError<E: TaskEncoder + ?Sized> {
    EnqueueingError(<E as TaskEncoder>::EnqueueingError),
    EncodingError(<E as TaskEncoder>::EncodingError),
    DependencyError(Vec<DependencyErrorLink>),
    CyclicError,
    PanicError(String),
    Reported(vir::Reported),
}

impl<E: TaskEncoder + ?Sized> std::fmt::Debug for TaskEncoderError<E>
where
    <E as TaskEncoder>::EncodingError: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut helper = f.debug_struct("TaskEncoderError");
        match self {
            Self::EncodingError(err) => helper.field("EncodingError", err),
            Self::EnqueueingError(err) => helper.field("EnqueueingError", err),
            Self::DependencyError(stack) => helper.field("DependencyError", stack),
            Self::CyclicError => helper.field("CyclicError", &""),
            Self::PanicError(msg) => helper.field("PanicError", msg),
            Self::Reported(reported) => helper.field("Reported", reported),
        };
        helper.finish()
    }
}

// manual implementation because derive adds Clone on all generic parameters
impl<E: TaskEncoder + ?Sized> Clone for TaskEncoderError<E> {
    fn clone(&self) -> Self {
        match self {
            Self::EncodingError(err) => Self::EncodingError(err.clone()),
            Self::EnqueueingError(err) => Self::EnqueueingError(err.clone()),
            Self::DependencyError(stack) => Self::DependencyError(stack.clone()),
            Self::CyclicError => Self::CyclicError,
            Self::PanicError(msg) => Self::PanicError(msg.clone()),
            Self::Reported(reported) => Self::Reported(*reported),
        }
    }
}
