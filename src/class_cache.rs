#[cfg(debug_assertions)]
use std::cell::Cell;
use std::{mem, rc::Rc};

use crate::{
    jvm_model::{JvmClass, ObjectArrayType},
    method_call_cache::{StaticMethodCallInfo, VirtualMethodCallInfo},
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
    Type(TypeInfo),
    StaticFieldAccess(FieldAccessInfo),
    NonStaticFieldAccess(FieldAccessInfo),
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

    #[inline]
    fn get_entry(&self, index: u16) -> Option<&Rc<CacheEntry>> {
        let offset_index = index as i64 - self.offset;
        if offset_index < 0 || offset_index as usize >= self.array.len() {
            return None;
        }

        self.array[offset_index as usize].as_ref()
    }

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
    use super::*;

    #[test]
    fn insert_at_end() {
        let mut cache = ClassCache::new();

        cache.register(
            5,
            Rc::new(CacheEntry::VirtualMethodCall(mock_virtual_method("end"))),
        );

        let result = cache.get_virtual_method(5);
        assert_eq!("end", result.unwrap().method_name);
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
            Rc::new(CacheEntry::VirtualMethodCall(mock_virtual_method("first"))),
        );

        cache.register(
            5,
            Rc::new(CacheEntry::VirtualMethodCall(mock_virtual_method("start"))),
        );

        let result_start = cache.get_virtual_method(5);
        let result_old = cache.get_virtual_method(10);

        assert_eq!("start", result_start.unwrap().method_name);
        assert_eq!("first", result_old.unwrap().method_name);
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
            Rc::new(CacheEntry::VirtualMethodCall(mock_virtual_method("low"))),
        );

        cache.register(
            10,
            Rc::new(CacheEntry::VirtualMethodCall(mock_virtual_method("high"))),
        );

        cache.register(
            5,
            Rc::new(CacheEntry::VirtualMethodCall(mock_virtual_method("middle"))),
        );

        let low = cache.get_virtual_method(1);
        let mid = cache.get_virtual_method(5);
        let high = cache.get_virtual_method(10);

        assert_eq!("low", low.unwrap().method_name);
        assert_eq!("middle", mid.unwrap().method_name);
        assert_eq!("high", high.unwrap().method_name);
        assert_eq!(3, cache.get_cache_hits());

        let (used, total) = cache.get_storage_efficiency();
        assert_eq!(3, used);
        assert_eq!(10, total);
    }

    fn mock_virtual_method(name: &str) -> VirtualMethodCallInfo {
        VirtualMethodCallInfo {
            method_name: name.to_owned(),
            descriptor: "()V".to_owned(),
            parameter_list: vec![],
        }
    }
}
