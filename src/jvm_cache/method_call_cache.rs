use std::{collections::HashMap, hash::Hash, num::NonZeroUsize, rc::Rc};

use crate::{
    class_cache::CacheEntry,
    jvm_cache::method_signature_cache::MethodSignatureId,
    jvm_model::{DescriptorType, JvmClass},
};

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
    pub method_signature_id: MethodSignatureId,
    /// list of types in stack pop order (reversed)
    pub parameter_list: Vec<DescriptorType>,
}

#[derive(Debug, Clone)]
pub struct InterfaceMethodCallInfo {
    pub interface: Rc<JvmClass>,
    pub method_signature_id: MethodSignatureId,
    /// list of types in stack pop order (reversed)
    pub parameter_list: Vec<DescriptorType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VirtualMethodInfoKey {
    pub signature_id: MethodSignatureId,
}
impl From<VirtualMethodCallInfo> for VirtualMethodInfoKey {
    fn from(value: VirtualMethodCallInfo) -> Self {
        Self {
            signature_id: value.method_signature_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InterfaceMethodInfoKey {
    pub method_signature_id: MethodSignatureId,
    pub interface_ptr: usize,
}
impl From<InterfaceMethodCallInfo> for InterfaceMethodInfoKey {
    fn from(value: InterfaceMethodCallInfo) -> Self {
        Self {
            method_signature_id: value.method_signature_id,
            interface_ptr: Rc::as_ptr(&value.interface) as usize,
        }
    }
}

/// for registering and deduplication of method call cache entries
#[derive(Debug)]
pub struct MethodCallCache {
    static_method_info_identity: HashMap<StaticMethodInfoKey, Rc<CacheEntry>>,
    virtual_method_info_identity: HashMap<VirtualMethodInfoKey, Rc<CacheEntry>>,
    interface_method_info_identity: HashMap<InterfaceMethodInfoKey, Rc<CacheEntry>>,
}
impl MethodCallCache {
    pub fn new() -> Self {
        Self {
            static_method_info_identity: HashMap::new(),
            virtual_method_info_identity: HashMap::new(),
            interface_method_info_identity: HashMap::new(),
        }
    }

    fn register_generic_call_info<C, I, F>(
        map: &mut HashMap<I, Rc<CacheEntry>>,
        call_info: C,
        method_ref_index: u16,
        caller_class: &Rc<JvmClass>,
        wrap: F,
    ) where
        C: Clone,
        I: Hash + From<C> + Eq,
        F: FnOnce(C) -> CacheEntry,
    {
        let key = I::from(call_info.clone());
        let call_info = if let Some(existing) = map.get(&key) {
            existing.clone()
        } else {
            let call_info = Rc::new(wrap(call_info));
            map.insert(key, call_info.clone());
            call_info
        };

        caller_class
            .state
            .borrow_mut()
            .cache
            .register(method_ref_index, call_info);
    }

    pub fn register_virtual_call_info(
        &mut self,
        virtual_method_call_info: VirtualMethodCallInfo,
        method_ref_index: u16,
        caller_class: &Rc<JvmClass>,
    ) {
        Self::register_generic_call_info(
            &mut self.virtual_method_info_identity,
            virtual_method_call_info,
            method_ref_index,
            caller_class,
            CacheEntry::VirtualMethodCall,
        )
    }

    pub fn register_static_call_info(
        &mut self,
        static_method_call_info: StaticMethodCallInfo,
        method_ref_index: u16,
        caller_class: &Rc<JvmClass>,
    ) {
        Self::register_generic_call_info(
            &mut self.static_method_info_identity,
            static_method_call_info,
            method_ref_index,
            caller_class,
            CacheEntry::StaticMethodCall,
        )
    }

    pub fn register_interface_call_info(
        &mut self,
        interface_method_call_info: InterfaceMethodCallInfo,
        method_ref_index: NonZeroUsize,
        caller_class: &Rc<JvmClass>,
    ) {
        debug_assert!(method_ref_index.get() <= u16::MAX as usize);
        Self::register_generic_call_info(
            &mut self.interface_method_info_identity,
            interface_method_call_info,
            method_ref_index.get() as u16,
            caller_class,
            CacheEntry::InterfaceMethodCall,
        )
    }
}
