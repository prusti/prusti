// © 2019, ETH Zurich
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::errors::*;
use jni::{
    objects::{JObject, JValue},
    strings::JavaStr,
    JNIEnv,
};
use std::ffi::CStr;

pub fn get_return_signature(signature: &str) -> String {
    let splitted: Vec<&str> = signature.split(')').collect();
    splitted[1].to_string()
}

pub fn generate_jni_type(signature: &str) -> String {
    match signature.chars().next().unwrap() {
        '[' | 'L' => "JObject".to_string(),
        'B' => "jbyte".to_string(),
        'C' => "jchar".to_string(),
        'S' => "jshort".to_string(),
        'I' => "jint".to_string(),
        'J' => "jlong".to_string(),
        'Z' => "bool".to_string(),
        'F' => "jfloat".to_string(),
        'D' => "jdouble".to_string(),
        'V' => "()".to_string(),
        x => unreachable!("{}", x),
    }
}

pub fn generate_return_jni_type(signature: &str) -> String {
    match signature.chars().next().unwrap() {
        '[' | 'L' => "JObject<'a>".to_string(),
        'B' => "jbyte".to_string(),
        'C' => "jchar".to_string(),
        'S' => "jshort".to_string(),
        'I' => "jint".to_string(),
        'J' => "jlong".to_string(),
        'Z' => "bool".to_string(),
        'F' => "jfloat".to_string(),
        'D' => "jdouble".to_string(),
        'V' => "()".to_string(),
        x => unreachable!("{}", x),
    }
}

pub fn generate_jni_type_char(signature: &str) -> String {
    match signature.chars().next().unwrap() {
        '[' => "l".to_string(),
        x => x.to_lowercase().to_string(),
    }
}

pub fn generate_jvalue_wrapper(par: &str, signature: &str) -> String {
    match signature.chars().next().unwrap() {
        '[' | 'L' => format!("JValue::Object({par})"),
        'B' => format!("JValue::Byte({par})"),
        'C' => format!("JValue::Char({par})"),
        'S' => format!("JValue::Short({par})"),
        'I' => format!("JValue::Int({par})"),
        'J' => format!("JValue::Long({par})"),
        'Z' => format!("JValue::Bool({par} as jboolean)"),
        'F' => format!("JValue::Float({par})"),
        'D' => format!("JValue::Double({par})"),
        x => unreachable!("{}", x),
    }
}

pub fn java_str_to_string(string: &JavaStr) -> Result<String> {
    unsafe { Ok(CStr::from_ptr(string.get_raw()).to_str()?.to_string()) }
}

pub fn java_str_to_valid_rust_argument_name(string: &JavaStr) -> Result<String> {
    Ok(format!(
        "arg_{}",
        java_identifier_to_rust(&java_str_to_string(string)?)
    ))
}

/// Returns the JNI type descriptor of the `java.lang.Class` `class`, e.g. `I` or
/// `Ljava/lang/String;`.
pub fn class_descriptor<'a>(env: &JNIEnv<'a>, class: JObject<'a>) -> Result<String> {
    let method_type = env
        .call_static_method(
            "java/lang/invoke/MethodType",
            "methodType",
            "(Ljava/lang/Class;)Ljava/lang/invoke/MethodType;",
            &[JValue::Object(class)],
        )?
        .l()?;
    // The descriptor of a method without parameters that returns `class`.
    let method_descriptor = method_type_descriptor(env, method_type)?;
    Ok(method_descriptor["()".len()..].to_string())
}

/// Returns the JNI descriptor of the `java.lang.reflect.Method` `method`.
pub fn method_descriptor<'a>(env: &JNIEnv<'a>, method: JObject<'a>) -> Result<String> {
    let return_type = env
        .call_method(method, "getReturnType", "()Ljava/lang/Class;", &[])?
        .l()?;
    executable_descriptor(env, method, return_type)
}

/// Returns the JNI descriptor of the `java.lang.reflect.Constructor` `constructor`.
pub fn constructor_descriptor<'a>(env: &JNIEnv<'a>, constructor: JObject<'a>) -> Result<String> {
    let void = env
        .get_static_field("java/lang/Void", "TYPE", "Ljava/lang/Class;")?
        .l()?;
    executable_descriptor(env, constructor, void)
}

/// Returns the JNI descriptor of the method or constructor `executable`, given its return type.
fn executable_descriptor<'a>(
    env: &JNIEnv<'a>,
    executable: JObject<'a>,
    return_type: JObject<'a>,
) -> Result<String> {
    let parameter_types = env
        .call_method(executable, "getParameterTypes", "()[Ljava/lang/Class;", &[])?
        .l()?;
    let method_type = env
        .call_static_method(
            "java/lang/invoke/MethodType",
            "methodType",
            "(Ljava/lang/Class;[Ljava/lang/Class;)Ljava/lang/invoke/MethodType;",
            &[JValue::Object(return_type), JValue::Object(parameter_types)],
        )?
        .l()?;
    method_type_descriptor(env, method_type)
}

fn method_type_descriptor<'a>(env: &JNIEnv<'a>, method_type: JObject<'a>) -> Result<String> {
    let descriptor = env
        .call_method(
            method_type,
            "toMethodDescriptorString",
            "()Ljava/lang/String;",
            &[],
        )?
        .l()?;
    java_str_to_string(&env.get_string(descriptor.into())?)
}

pub fn java_identifier_to_rust(name: &str) -> String {
    // identifier can only be composed of [a-zA-Z&_] characters - https://docs.oracle.com/javase/specs/jls/se7/html/jls-3.html#jls-3.8
    name.replace('_', "__").replace('$', "_dollar_")
}

// Runtime checks for objects are performed only on variables of class types. Those have signature starting with 'L'.
pub fn is_signature_of_class_type(signature: &str) -> bool {
    signature.starts_with('L')
}

/// Generates a runtime check that the object is of the expected class - applies only to object types (starting with L)
/// If this value is also the returned value (result), unwrap it first (since it is of type Result<T>)
fn generate_type_check(variable_name: &str, variable_type_name: &str, is_result: bool) -> String {
    let mut type_check: Vec<String> = vec![];
    type_check.push("    debug_assert!(".to_string());
    type_check.push(format!(
        "        self.env.is_instance_of({variable_name}, self.env.find_class({variable_type_name})?)?,"
    ));
    type_check.push(format!("        \"Java binding type failure. Expected object of class {{{variable_type_name}}}, but got {{}} instead\", get_jobject_class_name(self.env, {variable_name})"));
    type_check.push("    );".to_string());

    if is_result {
        let indented_type_check = type_check
            .iter()
            .map(|x| format!("    {x}"))
            .collect::<Vec<_>>()
            .join("\n");

        vec![
            "    if let Ok(result) = result {".to_string(),
            indented_type_check,
            "    }".to_string(),
        ]
    } else {
        type_check
    }
    .join("\n")
        + "\n"
}

/// Generates dynamic type checks to assert that the result (named result and wrapped in the Result enum) object is of an expected class.
/// Only to be called on object of java class types (having signature starting with L)
/// class_name_variable_name - needs to be a name of a variable in the generated code that holds the '/'-separated class name
pub fn generate_result_type_check(class_name_variable_name: &str) -> String {
    generate_type_check("result", class_name_variable_name, true)
}

/// Generates dynamic type checks to assert that an object (receiver or parameter) is of an expected class.
/// Only to be called on object of java class types (having signature starting with L)
/// class_name_variable_name - needs to be a name of a variable in the generated code that holds the '/'-separated class name
pub fn generate_variable_type_check(variable_name: &str, class_name_variable_name: &str) -> String {
    generate_type_check(variable_name, class_name_variable_name, false)
}
