use std::{collections::HashMap, rc::Rc};

use crate::{
    jvm::{
        CLASS_CLASS_NAME, DOUBLE_CLASS_NAME, FLOAT_CLASS_NAME, OBJECT_CLASS_NAME, SYSTEM_CLASS_NAME,
    },
    jvm_heap::JvmHeap,
    jvm_model::{JvmClass, JvmError, JvmResult, JvmThread, JvmValue},
};

mod class_methods;
mod double_methods;
mod float_methods;
mod object_methods;
mod system_methods;

type NativeMethodHandler = fn(&mut JvmThread, &mut JvmHeap, Vec<JvmValue>) -> JvmResult<()>;

const NATIVE_METHODS: [(&str, &str, &str, NativeMethodHandler); 10] = [
    (
        OBJECT_CLASS_NAME,
        "<init>",
        "()V",
        object_methods::object_constructor,
    ),
    (
        OBJECT_CLASS_NAME,
        "registerNatives",
        "()V",
        object_methods::register_natives,
    ),
    (
        SYSTEM_CLASS_NAME,
        "registerNatives",
        "()V",
        system_methods::register_natives,
    ),
    (
        SYSTEM_CLASS_NAME,
        "arraycopy",
        "(Ljava/lang/Object;ILjava/lang/Object;II)V",
        system_methods::array_copy,
    ),
    (
        CLASS_CLASS_NAME,
        "registerNatives",
        "()V",
        class_methods::register_natives,
    ),
    (
        CLASS_CLASS_NAME,
        "desiredAssertionStatus0",
        "(Ljava/lang/Class;)Z",
        class_methods::desired_assertion_status0,
    ),
    (
        CLASS_CLASS_NAME,
        "getPrimitiveClass",
        "(Ljava/lang/String;)Ljava/lang/Class;",
        class_methods::get_primitive_class,
    ),
    (
        FLOAT_CLASS_NAME,
        "floatToRawIntBits",
        "(F)I",
        float_methods::float_to_raw_int_bits,
    ),
    (
        DOUBLE_CLASS_NAME,
        "doubleToRawLongBits",
        "(D)J",
        double_methods::double_to_raw_long_bits,
    ),
    (
        DOUBLE_CLASS_NAME,
        "longBitsToDouble",
        "(J)D",
        double_methods::long_bits_to_double,
    ),
];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct NativeMethodNameKey {
    class_name: String,
    method_name: String,
    descriptor: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct NativeMethodCallKey {
    class_ptr: usize,
    method_index: usize,
}
impl NativeMethodCallKey {
    pub fn new(class: &Rc<JvmClass>, method_index: usize) -> Self {
        Self {
            class_ptr: Rc::as_ptr(class) as usize,
            method_index,
        }
    }
}

#[derive(Debug)]
pub struct NativeMethodResolver {
    handlers: Vec<NativeMethodHandler>,
    name_map: HashMap<NativeMethodNameKey, usize>,
    call_map: HashMap<NativeMethodCallKey, usize>,
}
impl NativeMethodResolver {
    pub fn new() -> Self {
        let mut name_map = HashMap::with_capacity(NATIVE_METHODS.len());
        let mut handlers = Vec::with_capacity(NATIVE_METHODS.len());
        for (class, method, desc, handler) in NATIVE_METHODS {
            let index = handlers.len();
            handlers.push(handler);
            let name_key = NativeMethodNameKey {
                class_name: class.to_owned(),
                method_name: method.to_owned(),
                descriptor: desc.to_owned(),
            };
            name_map.insert(name_key, index);
        }

        Self {
            handlers,
            name_map,
            call_map: HashMap::new(),
        }
    }

    pub fn execute_native_method(
        &mut self,
        thread: &mut JvmThread,
        heap: &mut JvmHeap,
        params: Vec<JvmValue>,
        method_index: usize,
        class: Rc<JvmClass>,
    ) -> JvmResult<()> {
        let call_key = NativeMethodCallKey::new(&class, method_index);
        if let Some(index) = self.call_map.get(&call_key) {
            return self.handlers[*index](thread, heap, params);
        }

        let method_name = class
            .class_file
            .constant_pool
            .get_utf8(class.class_file.methods[method_index].name_index)
            .expect("Expected method name")
            .to_owned();
        let descriptor = class
            .class_file
            .constant_pool
            .get_utf8(class.class_file.methods[method_index].descriptor_index)
            .expect("Expected method descriptor")
            .to_owned();
        let name_key = NativeMethodNameKey {
            class_name: class.class_file.get_class_name().unwrap().to_owned(),
            method_name,
            descriptor,
        };

        if let Some(index) = self.name_map.get(&name_key) {
            self.call_map.insert(call_key, *index);
            return self.handlers[*index](thread, heap, params);
        }

        Err(JvmError::NativeMethodImplementationNotFound {
            class_name: name_key.class_name,
            method_name: name_key.method_name,
            method_descriptor: name_key.descriptor,
        }
        .bx())
    }
}
