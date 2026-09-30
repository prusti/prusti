use crate::*;

#[extern_spec]
impl<T> Option<T> {
    #[trusted]
    #[pure]
    #[ensures(result == matches!(self, Some(_)))]
    pub fn is_some(&self) -> bool;

    #[trusted]
    #[pure]
    #[ensures(result == matches!(self, None))]
    pub fn is_none(&self) -> bool;
}
