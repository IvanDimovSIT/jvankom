use std::{collections::HashMap, rc::Rc};

use crate::jvm_model::JvmClass;

/// cache used by classes that create objects in their methods
#[derive(Debug)]
pub struct ObjectCreationCache {
    cache: HashMap<u16, Rc<JvmClass>>,
}
impl ObjectCreationCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn get(&self, unvalidated_index: u16) -> Option<Rc<JvmClass>> {
        self.cache.get(&unvalidated_index).cloned()
    }

    pub fn register(&mut self, unvalidated_index: u16, created_object_class: Rc<JvmClass>) {
        let old_value = self.cache.insert(unvalidated_index, created_object_class);
        debug_assert!(old_value.is_none());
    }
}
