use std::rc::Rc;

use crate::{
    field_initialisation::{determine_non_static_field_types, initialise_object_fields},
    jvm_model::{CLASS_CLASS_NAME, HeapObject, JvmClass, JvmResult, STRING_CLASS_NAME},
};

pub fn create_string_object(string_class: &Rc<JvmClass>) -> JvmResult<HeapObject> {
    debug_assert_eq!(STRING_CLASS_NAME, string_class.class_file.get_class_name());

    if let Some(str) = string_class.state.borrow().default_object.clone() {
        Ok(str)
    } else {
        let non_static_field_types = determine_non_static_field_types(string_class)?;
        let str = initialise_object_fields(string_class.clone(), &non_static_field_types);
        let mut state = string_class.state.borrow_mut();
        state.non_static_fields = Some(non_static_field_types);
        state.default_object = Some(str.clone());

        Ok(str)
    }
}

pub fn create_class_object(class_class: &Rc<JvmClass>, _class_name: &str) -> JvmResult<HeapObject> {
    debug_assert_eq!(CLASS_CLASS_NAME, class_class.class_file.get_class_name());

    if let Some(cl) = class_class.state.borrow().default_object.clone() {
        Ok(cl)
    } else {
        let non_static_field_types = determine_non_static_field_types(class_class)?;
        let str = initialise_object_fields(class_class.clone(), &non_static_field_types);
        let mut state = class_class.state.borrow_mut();
        state.non_static_fields = Some(non_static_field_types);
        state.default_object = Some(str.clone());

        Ok(str)
    }

    // TODO: initialise Class object
}
