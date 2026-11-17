use prusti_rustc_interface::middle::mir;
use prusti_utils::config;

/// The error reported when the condition of an `Assert` terminator with the
/// message `msg` might not hold, or `None` if the condition is assumed rather
/// than checked.
pub(crate) fn assert_error_message(msg: &mir::AssertMessage<'_>) -> Option<&'static str> {
    match *msg {
        mir::AssertMessage::BoundsCheck { .. } => {
            Some("the array or slice index may be out of bounds")
        }
        mir::AssertMessage::Overflow(..) | mir::AssertMessage::OverflowNeg(..)
            if !config::check_overflows() =>
        {
            // If we are not checking for overflows, encode an overflow-checking
            // assertion as an assume instead.
            None
        }
        mir::AssertMessage::Overflow(op, ..) => Some(overflow_message(op)),
        mir::AssertMessage::OverflowNeg(..) => {
            Some("assertion might fail with \"attempt to negate with overflow\"")
        }
        mir::AssertMessage::DivisionByZero(..) => {
            Some("assertion might fail with \"attempt to divide by zero\"")
        }
        mir::AssertMessage::RemainderByZero(..) => Some(
            "assertion might fail with \"attempt to calculate the remainder with a divisor of zero\"",
        ),
        mir::AssertMessage::ResumedAfterReturn(..) => Some("execution may continue after return"),
        mir::AssertMessage::ResumedAfterPanic(..) => Some("execution may continue after panic"),
        mir::AssertMessage::MisalignedPointerDereference { .. } => {
            Some("misaligned pointer may be dereferenced")
        }
        mir::AssertKind::ResumedAfterDrop(..) => Some("execution may continue after drop"),
        mir::AssertKind::NullPointerDereference => Some("null pointer may be dereferenced"),
        mir::AssertKind::InvalidEnumConstruction(..) => Some("invalid enum construction may occur"),
        mir::AssertKind::NullReferenceConstructed => Some("null reference may be constructed"),
    }
}

/// The error for an overflow of `op`. MIR building checks for overflow the
/// arithmetic `Add`, `Sub` and `Mul`, shifts by at least the bit width (`Shl`,
/// `Shr`), and the signed `MIN / -1` (`Div`, `Rem`); no other operation is
/// checked for overflow.
fn overflow_message(op: mir::BinOp) -> &'static str {
    match op {
        mir::BinOp::Add => "assertion might fail with \"attempt to add with overflow\"",
        mir::BinOp::Sub => "assertion might fail with \"attempt to subtract with overflow\"",
        mir::BinOp::Mul => "assertion might fail with \"attempt to multiply with overflow\"",
        mir::BinOp::Div => "assertion might fail with \"attempt to divide with overflow\"",
        mir::BinOp::Rem => {
            "assertion might fail with \"attempt to calculate the remainder with overflow\""
        }
        mir::BinOp::Shl => "assertion might fail with \"attempt to shift left with overflow\"",
        mir::BinOp::Shr => "assertion might fail with \"attempt to shift right with overflow\"",
        mir::BinOp::AddUnchecked
        | mir::BinOp::AddWithOverflow
        | mir::BinOp::SubUnchecked
        | mir::BinOp::SubWithOverflow
        | mir::BinOp::MulUnchecked
        | mir::BinOp::MulWithOverflow
        | mir::BinOp::BitXor
        | mir::BinOp::BitAnd
        | mir::BinOp::BitOr
        | mir::BinOp::ShlUnchecked
        | mir::BinOp::ShrUnchecked
        | mir::BinOp::Eq
        | mir::BinOp::Lt
        | mir::BinOp::Le
        | mir::BinOp::Ne
        | mir::BinOp::Ge
        | mir::BinOp::Gt
        | mir::BinOp::Cmp
        | mir::BinOp::Offset => unreachable!("`{op:?}` is not checked for overflow"),
    }
}
