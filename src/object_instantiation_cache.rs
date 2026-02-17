use std::{collections::HashMap, rc::Rc};

use crate::{class_file::ClassFile, jvm_model::FieldInfo};

#[derive(Debug)]
pub struct ClassFieldInfo {
    pub field_infos: Vec<FieldInfo>,
    pub class: Rc<ClassFile>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct ClassFieldKey {
    caller_class_ptr: usize,
    cp_index: u16,
}
impl ClassFieldKey {
    pub fn new(caller_class: &Rc<ClassFile>, cp_index: u16) -> Self {
        Self {
            caller_class_ptr: Rc::as_ptr(caller_class) as usize,
            cp_index,
        }
    }
}

#[derive(Debug)]
pub struct ClassFieldCache {
    class_field_infos: Vec<ClassFieldInfo>,
    class_field_map: HashMap<usize, usize>,
    caller_map: HashMap<ClassFieldKey, usize>,
}
impl ClassFieldCache {
    pub fn new() -> Self {
        Self {
            class_field_infos: vec![],
            class_field_map: HashMap::new(),
            caller_map: HashMap::new(),
        }
    }

    pub fn get_by_index(&self, index: usize) -> &ClassFieldInfo {
        &self.class_field_infos[index]
    }

    /// returns non-static field infos
    pub fn get_object_instatiation_info_from_class(
        &self,
        class: &Rc<ClassFile>,
    ) -> Option<&ClassFieldInfo> {
        let index = *self.class_field_map.get(&(Rc::as_ptr(class) as usize))?;
        Some(&self.class_field_infos[index])
    }

    pub fn get_class_field_info(
        &self,
        caller_class: &Rc<ClassFile>,
        cp_index: u16,
    ) -> Option<&ClassFieldInfo> {
        let key = ClassFieldKey::new(caller_class, cp_index);
        let index = *self.caller_map.get(&key)?;
        Some(&self.class_field_infos[index])
    }

    /// returns the object info index upon initialisation
    pub fn register_object_instantiation_info(
        &mut self,
        object_instantiation_info: ClassFieldInfo,
        caller_class: &Rc<ClassFile>,
        cp_index: u16,
    ) -> Option<usize> {
        let caller_key = ClassFieldKey::new(caller_class, cp_index);
        let called_class_ptr = Rc::as_ptr(&object_instantiation_info.class) as usize;

        if let Some(info_index) = self.class_field_map.get(&called_class_ptr) {
            let _ignored_result = self.caller_map.insert(caller_key, *info_index);

            None
        } else {
            let info_index = self.class_field_infos.len();
            self.class_field_infos.push(object_instantiation_info);
            self.class_field_map.insert(called_class_ptr, info_index);
            self.caller_map.insert(caller_key, info_index);

            Some(info_index)
        }
    }
}
