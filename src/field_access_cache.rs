use std::{collections::HashMap, rc::Rc};

use crate::jvm_model::JvmClass;

#[derive(Debug, Clone)]
pub struct FieldAccessInfo {
    pub target_class: Rc<JvmClass>,
    pub field_index: usize,
}

#[derive(Debug)]
pub struct FieldAccessCache {
    non_static_field_cache: HashMap<u16, FieldAccessInfo>,
    static_field_cache: HashMap<u16, FieldAccessInfo>,
}
impl FieldAccessCache {
    pub fn new() -> Self {
        Self {
            non_static_field_cache: HashMap::new(),
            static_field_cache: HashMap::new(),
        }
    }

    pub fn get_static(&self, method_ref_index: u16) -> Option<FieldAccessInfo> {
        self.static_field_cache.get(&method_ref_index).cloned()
    }

    pub fn register_static(&mut self, method_ref_index: u16, field_access_info: FieldAccessInfo) {
        self.register::<true>(method_ref_index, field_access_info);
    }

    pub fn get_non_static(&self, method_ref_index: u16) -> Option<&FieldAccessInfo> {
        self.non_static_field_cache.get(&method_ref_index)
    }

    pub fn register_non_static(
        &mut self,
        method_ref_index: u16,
        field_access_info: FieldAccessInfo,
    ) {
        self.register::<false>(method_ref_index, field_access_info);
    }

    #[inline]
    fn register<const IS_STATIC: bool>(
        &mut self,
        method_ref_index: u16,
        field_access_info: FieldAccessInfo,
    ) {
        let old_value = if IS_STATIC {
            self.static_field_cache
                .insert(method_ref_index, field_access_info)
        } else {
            self.non_static_field_cache
                .insert(method_ref_index, field_access_info)
        };
        debug_assert!(old_value.is_none());
    }
}
