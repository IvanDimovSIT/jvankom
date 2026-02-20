use std::{collections::HashMap, rc::Rc};

use crate::jvm_model::{DescriptorType, JvmClass};

#[derive(Debug, Clone)]
pub struct VTableEntry {
    pub resolved_class: Rc<JvmClass>,
    pub method_index: usize,
    pub bytecode_index: Option<usize>,
}
impl VTableEntry {
    pub fn new(
        resolved_class: Rc<JvmClass>,
        method_index: usize,
        bytecode_index: Option<usize>,
    ) -> Self {
        Self {
            resolved_class,
            method_index,
            bytecode_index,
        }
    }
}

#[derive(Debug)]
pub struct VTable {
    methods: HashMap<String, HashMap<String, VTableEntry>>,
}
impl VTable {
    pub fn new() -> Self {
        Self {
            methods: HashMap::new(),
        }
    }

    pub fn get(&self, method_name: &str, descriptor: &str) -> Option<VTableEntry> {
        self.methods.get(method_name)?.get(descriptor).cloned()
    }

    pub fn register(&mut self, method_name: &str, descriptor: &str, entry: VTableEntry) {
        if let Some(overloaded_method) = self.methods.get_mut(method_name) {
            overloaded_method.insert(descriptor.to_owned(), entry);
        } else {
            let mut new_method = HashMap::with_capacity(1);
            new_method.insert(descriptor.to_owned(), entry);
            self.methods.insert(method_name.to_owned(), new_method);
        }
    }
}
