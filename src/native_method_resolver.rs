use std::{collections::HashMap, rc::Rc};

use crate::{
    class_file::ClassFile,
    jvm_model::{JvmError, JvmHeap, JvmResult, JvmThread, JvmValue},
    native_method_resolver::object_methods::{object_constructor, register_natives},
};

mod object_methods;

type NativeMethodHandler = fn(&mut JvmThread, &mut JvmHeap, Vec<JvmValue>) -> JvmResult<()>;

const NATIVE_METHODS: [(&str, &str, &str, NativeMethodHandler); 2] = [
    ("java/lang/Object", "<init>", "()V", object_constructor),
    (
        "java/lang/Object",
        "registerNatives",
        "()V",
        register_natives,
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
    pub fn new(class: &Rc<ClassFile>, method_index: usize) -> Self {
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
        class: Rc<ClassFile>,
    ) -> JvmResult<()> {
        let call_key = NativeMethodCallKey::new(&class, method_index);
        if let Some(index) = self.call_map.get(&call_key) {
            return self.handlers[*index](thread, heap, params);
        }

        let method_name = class
            .constant_pool
            .get_utf8(class.methods[method_index].name_index)
            .expect("Expected method name")
            .to_owned();
        let descriptor = class
            .constant_pool
            .get_utf8(class.methods[method_index].descriptor_index)
            .expect("Expected method descriptor")
            .to_owned();
        let name_key = NativeMethodNameKey {
            class_name: class.get_class_name().unwrap().to_owned(),
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
