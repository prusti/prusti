#[allow(non_snake_case)]
pub mod wrappers {
    include!(concat!(env!("OUT_DIR"), "/gen/mod.rs"));
}

use jni::JNIEnv;
use jni::JavaVM;
use jni::InitArgsBuilder;
use jni::JNIVersion;
use jni::errors::Result;
use std::sync::LazyLock;

pub fn print_exception(env: &JNIEnv) {
    let exception_occurred = env.exception_check().unwrap_or_else(|e| panic!("{e:?}"));
    if exception_occurred {
        env.exception_describe()
            .unwrap_or_else(|e| panic!("{e:?}"));
    }
}

/// The JVM shared by all tests. A process can create only one JVM.
static JVM: LazyLock<JavaVM> = LazyLock::new(|| {
    let jvm_args = InitArgsBuilder::new()
        .version(JNIVersion::V8)
        .option("-Xcheck:jni")
        .option("-Xdebug")
        .option("-XX:+CheckJNICalls")
        .build()
        .unwrap_or_else(|e| {
            panic!("{} source: {:?}", e, std::error::Error::source(&e));
        });

    JavaVM::new(jvm_args).unwrap_or_else(|e| {
        panic!("{} source: {:?}", e, std::error::Error::source(&e));
    })
});

pub fn get_jvm() -> Result<&'static JavaVM> {
    Ok(&JVM)
}
