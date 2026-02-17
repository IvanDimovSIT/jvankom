#[cfg(debug_assertions)]
use std::cell::Cell;
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
    pub bytecode_index: Option<usize>,
    /// list of types in stack pop order (reversed)
    pub parameter_list: Vec<DescriptorType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct VirtualMethodCallKey {
    method_name: String,
    method_descriptor: String,
    object_class_ptr: usize,
}
impl VirtualMethodCallKey {
    fn new(
        object_class: &Rc<ClassFile>,
        method_name: impl Into<String>,
        method_descriptor: impl Into<String>,
    ) -> Self {
        Self {
            object_class_ptr: Rc::as_ptr(object_class) as usize,
            method_name: method_name.into(),
            method_descriptor: method_descriptor.into(),
        }
    }
}

#[derive(Debug)]
pub struct VirtualMethodCallInfo {
    pub bytecode_index: Option<usize>,
    pub method_index: usize,
    pub resolved_class: Rc<ClassFile>,
    pub types: Vec<DescriptorType>,
}

#[derive(Debug)]
pub struct MethodCallCache {
    static_method_cache: HashMap<StaticMethodCallKey, usize>,
    static_method_info_identity: HashMap<StaticMethodInfoKey, usize>,
    static_method_infos: Vec<StaticMethodCallInfo>,
    virtual_method_cache: HashMap<VirtualMethodCallKey, VirtualMethodCallInfo>,
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
            virtual_method_cache: HashMap::new(),
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

    pub fn get_virtual_call_info(
        &self,
        object_class: &Rc<ClassFile>,
        method_name: &str,
        method_descriptor: &str,
    ) -> Option<&VirtualMethodCallInfo> {
        let key = VirtualMethodCallKey::new(object_class, method_name, method_descriptor);
        let info = self.virtual_method_cache.get(&key)?;

        #[cfg(debug_assertions)]
        {
            self.cache_hits.set(self.cache_hits.get() + 1);
        }

        Some(info)
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
            let ignored_result = self.static_method_cache.insert(caller_key, *info_index);
            debug_assert!(ignored_result.is_none());
        } else {
            let info_index = self.static_method_infos.len();
            self.static_method_infos.push(static_method_call_info);
            self.static_method_cache.insert(caller_key, info_index);
            self.static_method_info_identity
                .insert(info_identity_key, info_index);
        }
    }

    pub fn register_virtual_call_info(
        &mut self,
        object_class: &Rc<ClassFile>,
        method_name: &str,
        method_descriptor: &str,
        virtual_method_call_info: VirtualMethodCallInfo,
    ) {
        let key = VirtualMethodCallKey::new(object_class, method_name, method_descriptor);
        let ignored_result = self
            .virtual_method_cache
            .insert(key, virtual_method_call_info);
        debug_assert!(ignored_result.is_none());
    }

    #[cfg(debug_assertions)]
    pub fn get_cache_hits(&self) -> usize {
        self.cache_hits.get()
    }
}
