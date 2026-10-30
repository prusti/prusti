use prusti_contracts::*;

#[extern_spec]
impl<T> std::vec::Vec<T> {
    /// Wrong number of arguments
    #[ensures(self.len() == old(self.len()) + 1)]
    fn push(&mut self); //~ERROR: this function takes 2 arguments but 1 argument was supplied
}

#[extern_spec]
impl<T> std::vec::Vec<T> {
    /// Wrong method name
    #[ensures(self.len() == 0)]
    fn clears(&mut self); //~ERROR: no associated function or constant named `clears`
}
