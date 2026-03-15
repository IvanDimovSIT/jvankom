#[cfg(debug_assertions)]
use std::cell::Cell;
use std::{mem, num::NonZeroUsize, rc::Rc};

use crate::{
    jvm_cache::method_call_cache::{
        InterfaceMethodCallInfo, StaticMethodCallInfo, VirtualMethodCallInfo,
    },
    jvm_model::{JvmClass, ObjectArrayType},
};

#[derive(Debug, Clone)]
pub struct FieldAccessInfo {
    pub target_class: Rc<JvmClass>,
    pub field_index: usize,
}

#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub object_or_array: ObjectArrayType,
    /// dimension if array
    pub dimension: usize,
}

#[derive(Debug, Clone)]
pub enum CacheEntry {
    StaticMethodCall(StaticMethodCallInfo),
    VirtualMethodCall(VirtualMethodCallInfo),
    InterfaceMethodCall(InterfaceMethodCallInfo),
    Type(TypeInfo),
    StaticFieldAccess(FieldAccessInfo),
    NonStaticFieldAccess(FieldAccessInfo),
    StringPoolRef(NonZeroUsize),
}

/// class specific runtime cached information
#[derive(Debug)]
pub struct ClassCache {
    offset: i64,
    array: Vec<Option<Rc<CacheEntry>>>,
    // only used for tests
    #[cfg(debug_assertions)]
    cache_hits: Cell<usize>,
}
impl ClassCache {
    pub fn new() -> Self {
        Self {
            offset: 0,
            array: vec![],
            #[cfg(debug_assertions)]
            cache_hits: Cell::new(0),
        }
    }

    pub fn register(&mut self, index: u16, entry: Rc<CacheEntry>) {
        let offset_index = index as i64 - self.offset;

        if offset_index >= 0 && (offset_index as usize) < self.array.len() {
            debug_assert!(self.array[offset_index as usize].is_none());
            self.array[offset_index as usize] = Some(entry);
        } else if self.array.is_empty() {
            debug_assert_eq!(0, self.offset);
            self.offset = index as i64;
            self.array.push(Some(entry));
        } else if offset_index < 0 {
            let number_to_extend = -offset_index;
            self.offset = index as i64;
            let mut new_arr = vec![None; number_to_extend as usize];
            new_arr[0] = Some(entry);
            new_arr.extend(mem::take(&mut self.array));
            self.array = new_arr;
        } else {
            let number_to_extend = offset_index as usize - self.array.len() + 1;
            let mut extention = vec![None; number_to_extend];
            extention[number_to_extend - 1] = Some(entry);
            self.array.extend(extention);
        }
    }

    pub fn get_type(&self, index: u16) -> Option<&TypeInfo> {
        let entry = self.get_entry(index)?;
        match entry.as_ref() {
            CacheEntry::Type(info) => {
                #[cfg(debug_assertions)]
                {
                    self.cache_hits.set(self.cache_hits.get() + 1);
                }
                Some(info)
            }
            _ => None,
        }
    }

    pub fn get_non_static_field_access(&self, index: u16) -> Option<&FieldAccessInfo> {
        let entry = self.get_entry(index)?;
        match entry.as_ref() {
            CacheEntry::NonStaticFieldAccess(info) => {
                #[cfg(debug_assertions)]
                {
                    self.cache_hits.set(self.cache_hits.get() + 1);
                }

                Some(info)
            }
            _ => None,
        }
    }

    pub fn get_static_field_access(&self, index: u16) -> Option<&FieldAccessInfo> {
        let entry = self.get_entry(index)?;
        match entry.as_ref() {
            CacheEntry::StaticFieldAccess(info) => {
                #[cfg(debug_assertions)]
                {
                    self.cache_hits.set(self.cache_hits.get() + 1);
                }

                Some(info)
            }
            _ => None,
        }
    }

    pub fn get_virtual_method(&self, index: u16) -> Option<&VirtualMethodCallInfo> {
        let entry = self.get_entry(index)?;
        match entry.as_ref() {
            CacheEntry::VirtualMethodCall(info) => {
                #[cfg(debug_assertions)]
                {
                    self.cache_hits.set(self.cache_hits.get() + 1);
                }

                Some(info)
            }
            _ => None,
        }
    }

    pub fn get_interface_method(&self, index: NonZeroUsize) -> Option<&InterfaceMethodCallInfo> {
        debug_assert!(index.get() <= u16::MAX as usize);
        let entry = self.get_entry(index.get() as u16)?;
        match entry.as_ref() {
            CacheEntry::InterfaceMethodCall(info) => {
                #[cfg(debug_assertions)]
                {
                    self.cache_hits.set(self.cache_hits.get() + 1);
                }

                Some(info)
            }
            _ => None,
        }
    }

    pub fn get_static_method(&self, index: u16) -> Option<&StaticMethodCallInfo> {
        let entry = self.get_entry(index)?;
        match entry.as_ref() {
            CacheEntry::StaticMethodCall(info) => {
                #[cfg(debug_assertions)]
                {
                    self.cache_hits.set(self.cache_hits.get() + 1);
                }

                Some(info)
            }
            _ => None,
        }
    }

    pub fn get_string_pool_ref(&self, index: u16) -> Option<NonZeroUsize> {
        let entry = self.get_entry(index)?;
        match entry.as_ref() {
            CacheEntry::StringPoolRef(string_ref) => {
                #[cfg(debug_assertions)]
                {
                    self.cache_hits.set(self.cache_hits.get() + 1);
                }

                Some(*string_ref)
            }
            _ => None,
        }
    }

    #[inline]
    fn get_entry(&self, index: u16) -> Option<&Rc<CacheEntry>> {
        let offset_index = index as i64 - self.offset;
        if offset_index < 0 || offset_index as usize >= self.array.len() {
            return None;
        }

        self.array[offset_index as usize].as_ref()
    }

    /// for testing
    #[cfg(debug_assertions)]
    pub fn get_cache_hits(&self) -> usize {
        self.cache_hits.get()
    }

    /// returns (used slotes, total allocated slots)
    pub fn get_storage_efficiency(&self) -> (usize, usize) {
        (self.array.iter().flatten().count(), self.array.len())
    }
}

#[cfg(test)]
mod tests {
    use std::mem::zeroed;

    use crate::jvm_model::DescriptorType;

    use super::*;

    #[test]
    fn insert_at_end() {
        let mut cache = ClassCache::new();

        cache.register(
            5,
            Rc::new(CacheEntry::VirtualMethodCall(mock_virtual_method(vec![
                DescriptorType::Integer,
            ]))),
        );

        let result = cache.get_virtual_method(5);
        assert_eq!(
            vec![DescriptorType::Integer],
            result.unwrap().parameter_list
        );
        assert_eq!(1, cache.get_cache_hits());

        let (used, total) = cache.get_storage_efficiency();
        assert_eq!(1, used);
        assert_eq!(1, total);
    }

    #[test]
    fn insert_at_start_extends_offset() {
        let mut cache = ClassCache::new();

        cache.register(
            10,
            Rc::new(CacheEntry::VirtualMethodCall(mock_virtual_method(vec![
                DescriptorType::Integer,
            ]))),
        );

        cache.register(
            5,
            Rc::new(CacheEntry::VirtualMethodCall(mock_virtual_method(vec![
                DescriptorType::Long,
            ]))),
        );

        let result_start = cache.get_virtual_method(5);
        let result_old = cache.get_virtual_method(10);

        assert_eq!(
            vec![DescriptorType::Long],
            result_start.unwrap().parameter_list
        );
        assert_eq!(
            vec![DescriptorType::Integer],
            result_old.unwrap().parameter_list
        );
        assert_eq!(2, cache.get_cache_hits());

        let (used, total) = cache.get_storage_efficiency();
        assert_eq!(2, used);
        assert_eq!(6, total);
    }

    #[test]
    fn insert_in_middle_fills_gap() {
        let mut cache = ClassCache::new();

        cache.register(
            1,
            Rc::new(CacheEntry::VirtualMethodCall(mock_virtual_method(vec![
                DescriptorType::Byte,
            ]))),
        );

        cache.register(
            10,
            Rc::new(CacheEntry::VirtualMethodCall(mock_virtual_method(vec![
                DescriptorType::Long,
            ]))),
        );

        cache.register(
            5,
            Rc::new(CacheEntry::VirtualMethodCall(mock_virtual_method(vec![
                DescriptorType::Integer,
            ]))),
        );

        let low = cache.get_virtual_method(1);
        let mid = cache.get_virtual_method(5);
        let high = cache.get_virtual_method(10);

        assert_eq!(vec![DescriptorType::Byte], low.unwrap().parameter_list);
        assert_eq!(vec![DescriptorType::Integer], mid.unwrap().parameter_list);
        assert_eq!(vec![DescriptorType::Long], high.unwrap().parameter_list);
        assert_eq!(3, cache.get_cache_hits());

        let (used, total) = cache.get_storage_efficiency();
        assert_eq!(3, used);
        assert_eq!(10, total);
    }

    fn mock_virtual_method(parameter_list: Vec<DescriptorType>) -> VirtualMethodCallInfo {
        VirtualMethodCallInfo {
            method_signature_id: unsafe { zeroed() },
            parameter_list,
        }
    }
}
