use error_chain::ChainedError;
use jni_gen::*;
use std::{env, path::Path};

fn main() {
    env_logger::init();
    let generated_dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("gen");

    WrapperGenerator::new()
        .wrap(java_class!("java.lang.Object"))
        .wrap_all(vec![
            java_class!("java.lang.Integer", vec![
                constructor!("(I)V"),
                field!("value"),
                method!("compareTo", "(Ljava/lang/Integer;)I"),
            ]),
            java_class!("java.util.Arrays", vec![
                method!("binarySearch", "([Ljava/lang/Object;Ljava/lang/Object;)I"),
            ]),
            java_class!("java.lang.Error", vec![
                constructor!("(Ljava/lang/String;)V"),
                method!("getMessage"),
                field!("detailMessage"),
            ]),
            java_class!("java.math.BigInteger", vec![
                method!("probablePrime"),
            ]),
        ])
        .generate(&generated_dir)
        .unwrap_or_else(|e| {
            panic!("{}", e.display_chain());
        });
}
