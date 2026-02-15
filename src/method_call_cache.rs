#[cfg(debug_assertions)]
use std::{cell::Cell, sync::atomic::AtomicUsize};
use std::{collections::HashMap, rc::Rc};

use crate::{class_file::ClassFile, jvm_model::DescriptorType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct StaticMethodCallKey {
    method_ref_index: u16,
    caller_ptr: usize,
}
impl StaticMethodCallKey {
    fn new(caller: &Rc<ClassFile>, method_ref_index: u16) -> Self {
        Self {
            method_ref_index,
            caller_ptr: Rc::as_ptr(caller) as usize,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StaticMethodInfoKey {
    class_ptr: usize,
    method_index: usize,
    parameter_list: Vec<DescriptorType>,
}
impl From<StaticMethodCallInfo> for StaticMethodInfoKey {
    fn from(value: StaticMethodCallInfo) -> Self {
        Self {
            class_ptr: Rc::as_ptr(&value.class) as usize,
            method_index: value.method_index,
            parameter_list: value.parameter_list,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StaticMethodCallInfo {
    pub class: Rc<ClassFile>,
    pub method_index: usize,
    pub bytecode_index: usize,
    /// list of types in stack pop order (reversed)
    pub parameter_list: Vec<DescriptorType>,
}

#[derive(Debug)]
pub struct MethodCallCache {
    static_method_cache: HashMap<StaticMethodCallKey, usize>,
    static_method_info_identity: HashMap<StaticMethodInfoKey, usize>,
    static_method_infos: Vec<StaticMethodCallInfo>,
    // only used for tests
    #[cfg(debug_assertions)]
    cache_hits: Cell<usize>,
}
impl MethodCallCache {
    pub fn new() -> Self {
        Self {
            static_method_cache: HashMap::new(),
            static_method_info_identity: HashMap::new(),
            static_method_infos: vec![],
            #[cfg(debug_assertions)]
            cache_hits: Cell::new(0),
        }
    }

    pub fn get_static_call_info(
        &self,
        caller_class: &Rc<ClassFile>,
        method_ref_index: u16,
    ) -> Option<&StaticMethodCallInfo> {
        let key = StaticMethodCallKey::new(caller_class, method_ref_index);
        let index = *self.static_method_cache.get(&key)?;
        #[cfg(debug_assertions)]
        {
            self.cache_hits.set(self.cache_hits.get() + 1);
        }

        Some(&self.static_method_infos[index])
    }

    pub fn register_static_call_info(
        &mut self,
        static_method_call_info: StaticMethodCallInfo,
        method_ref_index: u16,
        caller_class: &Rc<ClassFile>,
    ) {
        let caller_key = StaticMethodCallKey::new(caller_class, method_ref_index);
        let info_identity_key = StaticMethodInfoKey::from(static_method_call_info.clone());

        if let Some(info_index) = self.static_method_info_identity.get(&info_identity_key) {
            let _ignored_result = self.static_method_cache.insert(caller_key, *info_index);
        } else {
            let info_index = self.static_method_infos.len();
            self.static_method_infos.push(static_method_call_info);
            self.static_method_cache.insert(caller_key, info_index);
            self.static_method_info_identity
                .insert(info_identity_key, info_index);
        }
    }

    #[cfg(debug_assertions)]
    pub fn get_cache_hits(&self) -> usize {
        self.cache_hits.get()
    }
}
