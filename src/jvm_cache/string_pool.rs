use std::{collections::HashMap, num::NonZeroUsize};

use crate::{
    jvm_heap::JvmHeap,
    jvm_model::{ClassState, DescriptorType, HeapObject, JvmValue, STRING_CLASS_NAME},
};

/// field holding the character array
const VALUE_FIELD_NAME: &str = "value";

#[derive(Debug)]
pub struct StringPool {
    string_value_index: Option<usize>,
    string_references: HashMap<String, NonZeroUsize>,
}
impl StringPool {
    pub fn new() -> Self {
        Self {
            string_value_index: None,
            string_references: HashMap::new(),
        }
    }

    pub fn find_string(&self, string: &str) -> Option<NonZeroUsize> {
        self.string_references.get(string).copied()
    }

    pub fn register(&mut self, string: impl Into<String>, reference: NonZeroUsize) {
        let old_value = self.string_references.insert(string.into(), reference);
        debug_assert!(old_value.is_none());
    }

    pub fn get_string_references(&self) -> impl Iterator<Item = NonZeroUsize> {
        self.string_references.values().copied()
    }

    pub fn initialise_string_fields(
        &mut self,
        string: &str,
        string_obj: &mut HeapObject,
        jvm_heap: &mut JvmHeap,
    ) {
        if let HeapObject::Object { class, fields } = string_obj {
            debug_assert_eq!(STRING_CLASS_NAME, class.class_file.get_class_name());
            let arr = HeapObject::CharacterArray(Self::string_to_arr(string));
            let arr_ref = jvm_heap.allocate(arr);

            if let Some(data_index) = self.string_value_index {
                fields[data_index] = JvmValue::Reference(Some(arr_ref));
            } else {
                let data_index = Self::find_value_field(&class.state.borrow());
                self.string_value_index = Some(data_index);
                fields[data_index] = JvmValue::Reference(Some(arr_ref));
            }
        } else {
            panic!("passed heap object is not a standard object")
        }
    }

    pub fn find_value_field(string_state: &ClassState) -> usize {
        if let Some(fields) = &string_state.non_static_fields {
            for (i, field) in fields.iter().enumerate() {
                if field.name == VALUE_FIELD_NAME {
                    assert_eq!(DescriptorType::Reference, field.descriptor_type);
                    return i;
                }
            }
        } else {
            panic!("String class not initialised")
        }

        panic!("String class lacks a value field")
    }

    fn string_to_arr(string: &str) -> Vec<u16> {
        string.chars().map(|c| c as u16).collect()
    }
}
