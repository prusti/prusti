# Reporting stage

- The Prusti client reports compilation and verification errors.

In this final stage, any errors and warnings that occurred during verification are reported to the user.

> - [`prusti-server/src/lib.rs` - `handle_stream`](https://github.com/prusti/prusti/blob/main/prusti-server/src/lib.rs) - collection of errors from the verification results.
> - [`prusti-interface/src/prusti_error.rs` - `PrustiError::emit`](https://github.com/prusti/prusti/blob/main/prusti-interface/src/prusti_error.rs) - emission of Prusti errors as messages and source file spans into the compiler environment.
