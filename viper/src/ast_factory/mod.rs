// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

#[macro_use]
mod macros;
mod ast_type;
mod expression;
mod position;
mod program;
mod statement;
mod structs;

use crate::jni_utils::JniUtils;
use jni::{objects::JObject, JNIEnv};
use viper_sys::wrappers::viper::silver::ast;

pub use self::{expression::*, structs::*};

#[derive(Clone, Copy)]
pub struct AstFactory<'a> {
    env: &'a JNIEnv<'a>,
    jni: JniUtils<'a>,
}

impl<'a> AstFactory<'a> {
    pub fn new(env: &'a JNIEnv) -> Self {
        let jni = JniUtils::new(env);
        AstFactory { env, jni }
    }

    // === Info ===

    fn no_info(&self) -> JObject<'_> {
        self.jni
            .unwrap_result(ast::NoInfo_object::with(self.env).singleton())
    }

    /// The info of a function or domain function: the annotation
    /// `@noAutoTrigger()` if `no_auto_trigger`, otherwise none.
    fn function_info(&self, no_auto_trigger: bool) -> JObject<'_> {
        if !no_auto_trigger {
            return self.no_info();
        }
        let annotations = self
            .jni
            .new_map(&[(self.jni.new_string("noAutoTrigger"), self.jni.new_seq(&[]))]);
        self.jni
            .unwrap_result(ast::AnnotationInfo::with(self.env).new(annotations))
    }

    fn simple_info(&self, comments: &[&str]) -> JObject<'_> {
        self.jni.unwrap_result(
            ast::SimpleInfo::with(self.env).new(
                self.jni.new_seq(
                    &comments
                        .iter()
                        .map(|x| self.jni.new_string(x))
                        .collect::<Vec<JObject>>(),
                ),
            ),
        )
    }

    fn no_trafos(&self) -> JObject<'_> {
        self.jni
            .unwrap_result(ast::NoTrafos_object::with(self.env).singleton())
    }
}
