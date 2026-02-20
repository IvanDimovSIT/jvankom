#[cfg(debug_assertions)]
use std::cell::Cell;
use std::{collections::HashMap, hash::Hash, rc::Rc};

use crate::jvm_model::{DescriptorType, JvmClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct MethodCallKey {
    method_ref_index: u16,
    caller_ptr: usize,
}
impl MethodCallKey {
    fn new(caller: &Rc<JvmClass>, method_ref_index: u16) -> Self {
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
    pub class: Rc<JvmClass>,
    pub method_index: usize,
    pub bytecode_index: Option<usize>,
    /// list of types in stack pop order (reversed)
    pub parameter_list: Vec<DescriptorType>,
}

#[derive(Debug, Clone)]
pub struct VirtualMethodCallInfo {
    pub method_name: String,
    pub descriptor: String,
    /// list of types in stack pop order (reversed)
    pub parameter_list: Vec<DescriptorType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VirtualMethodInfoKey {
    pub method_name: String,
    pub descriptor: String,
}
impl From<VirtualMethodCallInfo> for VirtualMethodInfoKey {
    fn from(value: VirtualMethodCallInfo) -> Self {
        Self {
            method_name: value.method_name,
            descriptor: value.descriptor,
        }
    }
}

#[derive(Debug)]
pub struct MethodCallCache {
    static_method_cache: HashMap<MethodCallKey, usize>,
    static_method_info_identity: HashMap<StaticMethodInfoKey, usize>,
    static_method_infos: Vec<StaticMethodCallInfo>,
    virtual_method_cache: HashMap<MethodCallKey, usize>,
    virtual_method_info_identity: HashMap<VirtualMethodInfoKey, usize>,
    virtual_method_infos: Vec<VirtualMethodCallInfo>,
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
            virtual_method_info_identity: HashMap::new(),
            virtual_method_infos: vec![],
        }
    }

    #[inline]
    fn get_call_info<'a, T>(
        &'a self,
        method_cache: &HashMap<MethodCallKey, usize>,
        info_vec: &'a [T],
        caller_class: &Rc<JvmClass>,
        method_ref_index: u16,
    ) -> Option<&'a T> {
        let key = MethodCallKey::new(caller_class, method_ref_index);
        let index = *method_cache.get(&key)?;
        #[cfg(debug_assertions)]
        {
            self.cache_hits.set(self.cache_hits.get() + 1);
        }

        Some(&info_vec[index])
    }

    pub fn get_static_call_info(
        &self,
        caller_class: &Rc<JvmClass>,
        method_ref_index: u16,
    ) -> Option<&StaticMethodCallInfo> {
        self.get_call_info(
            &self.static_method_cache,
            &self.static_method_infos,
            caller_class,
            method_ref_index,
        )
    }

    pub fn get_virtual_call_info(
        &self,
        caller_class: &Rc<JvmClass>,
        method_ref_index: u16,
    ) -> Option<&VirtualMethodCallInfo> {
        self.get_call_info(
            &self.virtual_method_cache,
            &self.virtual_method_infos,
            caller_class,
            method_ref_index,
        )
    }

    #[inline]
    fn register_call_info<C, I>(
        method_call_info: C,
        method_identity_info: &mut HashMap<I, usize>,
        method_cache: &mut HashMap<MethodCallKey, usize>,
        method_infos: &mut Vec<C>,
        method_ref_index: u16,
        caller_class: &Rc<JvmClass>,
    ) where
        C: Clone,
        I: Hash + From<C> + Eq,
    {
        let caller_key = MethodCallKey::new(caller_class, method_ref_index);
        let info_identity_key = I::from(method_call_info.clone());

        if let Some(info_index) = method_identity_info.get(&info_identity_key) {
            let ignored_result = method_cache.insert(caller_key, *info_index);
            debug_assert!(ignored_result.is_none());
        } else {
            let info_index = method_infos.len();
            method_infos.push(method_call_info);
            method_cache.insert(caller_key, info_index);
            method_identity_info.insert(info_identity_key, info_index);
        }
    }

    pub fn register_static_call_info(
        &mut self,
        static_method_call_info: StaticMethodCallInfo,
        method_ref_index: u16,
        caller_class: &Rc<JvmClass>,
    ) {
        Self::register_call_info(
            static_method_call_info,
            &mut self.static_method_info_identity,
            &mut self.static_method_cache,
            &mut self.static_method_infos,
            method_ref_index,
            caller_class,
        );
    }

    pub fn register_virtual_call_info(
        &mut self,
        virtual_method_call_info: VirtualMethodCallInfo,
        method_ref_index: u16,
        caller_class: &Rc<JvmClass>,
    ) {
        Self::register_call_info(
            virtual_method_call_info,
            &mut self.virtual_method_info_identity,
            &mut self.virtual_method_cache,
            &mut self.virtual_method_infos,
            method_ref_index,
            caller_class,
        );
    }

    #[cfg(debug_assertions)]
    pub fn get_cache_hits(&self) -> usize {
        self.cache_hits.get()
    }
}
