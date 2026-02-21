use std::{collections::HashMap, rc::Rc};

use crate::jvm_model::JvmClass;

#[derive(Debug)]
pub struct FieldAccessInfo {
    pub target_class: Rc<JvmClass>,
    pub field_index: usize,
}

#[derive(Debug)]
pub struct FieldAccessCache {
    cache: HashMap<u16, FieldAccessInfo>,
}
impl FieldAccessCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn get(&self, method_ref_index: u16) -> Option<&FieldAccessInfo> {
        self.cache.get(&method_ref_index)
    }

    pub fn register(&mut self, method_ref_index: u16, field_access_info: FieldAccessInfo) {
        let old_value = self.cache.insert(method_ref_index, field_access_info);
        debug_assert!(old_value.is_none());
    }
}
