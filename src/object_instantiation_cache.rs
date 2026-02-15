use std::{collections::HashMap, rc::Rc};

use crate::{class_file::ClassFile, jvm_model::FieldInfo};

#[derive(Debug)]
pub struct ObjectInstantiationInfo {
    pub field_infos: Vec<FieldInfo>,
    pub class: Rc<ClassFile>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct ObjectInstantiationKey {
    caller_class_ptr: usize,
    cp_index: u16,
}
impl ObjectInstantiationKey {
    pub fn new(caller_class: &Rc<ClassFile>, cp_index: u16) -> Self {
        Self {
            caller_class_ptr: Rc::as_ptr(caller_class) as usize,
            cp_index,
        }
    }
}

#[derive(Debug)]
pub struct ObjectInstantiationCache {
    object_field_infos: Vec<ObjectInstantiationInfo>,
    class_field_map: HashMap<usize, usize>,
    caller_map: HashMap<ObjectInstantiationKey, usize>,
}
impl ObjectInstantiationCache {
    pub fn new() -> Self {
        Self {
            object_field_infos: vec![],
            class_field_map: HashMap::new(),
            caller_map: HashMap::new(),
        }
    }

    /// returns non-static field infos
    pub fn get_object_instatiation_info_from_class(
        &self,
        class: &Rc<ClassFile>,
    ) -> Option<&ObjectInstantiationInfo> {
        let index = *self.class_field_map.get(&(Rc::as_ptr(class) as usize))?;
        Some(&self.object_field_infos[index])
    }

    pub fn get_object_instantiation_info(
        &self,
        caller_class: &Rc<ClassFile>,
        cp_index: u16,
    ) -> Option<&ObjectInstantiationInfo> {
        let key = ObjectInstantiationKey::new(caller_class, cp_index);
        let index = *self.caller_map.get(&key)?;
        Some(&self.object_field_infos[index])
    }

    pub fn register_object_instantiation_info(
        &mut self,
        object_instantiation_info: ObjectInstantiationInfo,
        caller_class: &Rc<ClassFile>,
        cp_index: u16,
    ) {
        let caller_key = ObjectInstantiationKey::new(caller_class, cp_index);
        let called_class_ptr = Rc::as_ptr(&object_instantiation_info.class) as usize;

        if let Some(info_index) = self.class_field_map.get(&called_class_ptr) {
            let _ignored_result = self.caller_map.insert(caller_key, *info_index);
        } else {
            let info_index = self.object_field_infos.len();
            self.object_field_infos.push(object_instantiation_info);
            self.class_field_map.insert(called_class_ptr, info_index);
            self.caller_map.insert(caller_key, info_index);
        }
    }
}
