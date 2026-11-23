# Viper verification stage

- (With Prusti server only) Send VIR to the server.
- VIR is encoded into Viper.
- The Viper verifier is called and the results are obtained.
- (With Prusti server only) Send verification results back to the client.

## Prusti server

[Prusti server](https://github.com/prusti/prusti/pull/43) is an optional component of Prusti that can significantly reduce verification times by running a background process. The background process keeps an instance of JVM open, which is what Viper backends use to perform verification of Viper code. With the server enabled, a client only needs to send VIR to the server and receive the results once they are ready.

> - [`prusti-server/src/lib.rs` - `verify_programs`](https://github.com/prusti/prusti/blob/main/prusti-server/src/lib.rs) - verification with or without the server.

## Encoding VIR to Viper

As noted in [the previous section](prusti.md#mir-and-vir), VIR is an intermediate representation separate from Viper AST. In this step the encoding from one to the other is performed.

> - [`prusti-server/src/verification_request.rs` - `VerificationRequest::build_request`](https://github.com/prusti/prusti/blob/main/prusti-server/src/verification_request.rs) - call to `program_to_viper`.
> - [`prusti-viper/src/lib.rs`](https://github.com/prusti/prusti/blob/main/prusti-viper/src/lib.rs) - conversion of VIR to Viper AST.
