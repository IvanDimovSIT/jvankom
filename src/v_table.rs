use std::{collections::HashMap, rc::Rc};

use crate::{jvm_cache::method_signature_cache::MethodSignatureId, jvm_model::JvmClass};

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
    methods: HashMap<MethodSignatureId, VTableEntry>,
}
impl VTable {
    pub fn new() -> Self {
        Self {
            methods: HashMap::new(),
        }
    }

    pub fn get(&self, id: MethodSignatureId) -> Option<VTableEntry> {
        self.methods.get(&id).cloned()
    }

    pub fn register(&mut self, id: MethodSignatureId, entry: VTableEntry) {
        let old_entry = self.methods.insert(id, entry);
        debug_assert!(old_entry.is_none());
    }
}
