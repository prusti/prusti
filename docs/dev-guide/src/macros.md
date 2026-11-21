# Macros

Prusti makes use of [procedural macros](https://doc.rust-lang.org/reference/procedural-macros.html) as the interface for user-provided specifications.

## Specification syntax

Prusti [specification syntax](https://prusti.github.io/prusti/user-guide/syntax.html) is a superset of Rust boolean expressions. The parsing of Prusti-specific syntax is achieved with a custom preparser. Parsing of Rust syntax is delegated to the [`syn`](https://crates.io/crates/syn) crate.

 - [`prusti-contracts/prusti-specs/src/specifications/preparser.rs`](https://github.com/prusti/prusti/blob/main/prusti-contracts/prusti-specs/src/specifications/preparser.rs)

The preparser accepts the following EBNF grammar:

```ebnf
assertion ::= prusti_expr ;
pledge ::= pledge_lhs, ",", prusti_expr ;
pledge_lhs ::= [ ? actual rust expression ?, "=>" ], prusti_expr ;
 
prusti_expr ::= conjunction, [ "==>", prusti_expr ] ;
conjunction ::= entailment, { "&&", entailment } ;
entailment ::= primary | ? actual rust expression ?, [ "|=", [ "|", ? args as parsed by syn2 ?, "|" ], "[", [ ( requires | ensures ), { ",", ( requires | ensures ) } ], "]" ] ;
primary ::= "(", prusti_expr, ")"
          | "forall", "(", "|", ? one or more args as parsed by syn2 ?, "|", prusti_expr, [ ",", "triggers", "=", ? array as parsed by syn2 ? ] ")"
          ;
requires ::= "requires", "(", prusti_expr, ")" ;
ensures ::= "ensures", "(", prusti_expr, ")" ;
```

## Specification functions

Procedural macros only transform tokens, so they cannot pass data to the rest of the [Prusti pipeline](pipeline/summary.md) directly. Instead, the macros turn each specification into Rust code that the compiler type-checks like any other code: an assertion such as a precondition becomes the body of a generated spec function, marked with `#[prusti::spec_only]` and identified by a `#[prusti::spec_id = "..."]` attribute with a fresh UUID. The annotated item gets an attribute that refers to that ID, e.g. `#[prusti::pre_spec_id_ref = "..."]` for a precondition.

Boolean specifications inside a function body, such as `prusti_assert!` or `body_invariant!`, become closures instead, also marked with `#[prusti::spec_only]` and a `#[prusti::spec_id = "..."]`, and passed to `prusti_contracts::spec_block` at the place where they are written. After type checking, a HIR visitor matches the references with the spec functions and collects the spec closures; their type-checked MIR bodies are later encoded.

 - [`prusti-contracts/prusti-specs/src/lib.rs`](https://github.com/prusti/prusti/blob/main/prusti-contracts/prusti-specs/src/lib.rs) - the specification macros, which attach the attributes that refer to the spec functions.
 - [`prusti-contracts/prusti-specs/src/rewriter.rs`](https://github.com/prusti/prusti/blob/main/prusti-contracts/prusti-specs/src/rewriter.rs) - rewriter, generates the spec functions and the spec closures.
 - [`prusti-interface/src/specs/mod.rs` - `SpecCollector`](https://github.com/prusti/prusti/blob/main/prusti-interface/src/specs/mod.rs) - specification collector, implements [`intravisit::Visitor`](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_hir/intravisit/trait.Visitor.html) for walking the HIR.
