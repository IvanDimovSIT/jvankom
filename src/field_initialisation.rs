use std::rc::Rc;

use crate::{
    class_file::{ClassFile, FieldAccessFlags},
    jvm_model::{DescriptorType, FieldInfo, HeapObject, JvmClass, JvmResult, StaticFieldInfo},
};

pub fn determine_static_fields(class_file: &ClassFile) -> Vec<StaticFieldInfo> {
    let mut static_fields = vec![];
    for (index, field) in class_file.fields.iter().enumerate() {
        if !field.access_flags.check_flag(FieldAccessFlags::STATIC_FLAG) {
            continue;
        }

        let name = class_file
            .constant_pool
            .get_utf8(field.name_index)
            .expect("Invalid name index: Fields should be verified")
            .to_owned();
        let descriptor = class_file
            .constant_pool
            .get_utf8(field.descriptor_index)
            .expect("Invalid descriptor index: Fields should be verified");

        let descriptor_type = parse_field_descriptor(descriptor);
        let value = descriptor_type.create_default_value();

        let field_info = StaticFieldInfo {
            name,
            descriptor_type,
            value,
            field_class_file_index: index,
        };

        static_fields.push(field_info);
    }

    static_fields
}

/// initialises an object with all it's fields
pub fn initialise_object_fields(class: Rc<JvmClass>, field_infos: &[FieldInfo]) -> HeapObject {
    let mut fields = Vec::with_capacity(field_infos.len());
    for field_info in field_infos {
        fields.push(field_info.descriptor_type.create_default_value());
    }

    HeapObject::Object { class, fields }
}

pub fn determine_non_static_field_types(class: &Rc<JvmClass>) -> JvmResult<Vec<FieldInfo>> {
    if let Some(non_static) = class.state.borrow().non_static_fields.clone() {
        return Ok(non_static);
    }
    let mut field_infos = vec![];

    for (i, f) in class.class_file.fields.iter().enumerate() {
        if f.access_flags.check_flag(FieldAccessFlags::STATIC_FLAG) {
            continue;
        }

        let descriptor = class
            .class_file
            .constant_pool
            .get_utf8(f.descriptor_index)
            .expect("Invalid descriptor index");

        let field_name = class
            .class_file
            .constant_pool
            .get_utf8(f.name_index)
            .expect("Expected field name")
            .to_owned();

        let field_info = FieldInfo {
            name: field_name,
            class: class.clone(),
            descriptor_type: parse_field_descriptor(descriptor),
            field_class_file_index: i,
        };

        field_infos.push(field_info);
    }

    if class.class_file.super_class_index.is_some() {
        let super_class = class
            .state
            .borrow()
            .super_class
            .clone()
            .expect("Class should be initialised");
        let mut super_types = determine_non_static_field_types(&super_class)?;
        super_types.extend(field_infos);
        return Ok(super_types);
    }

    Ok(field_infos)
}

fn parse_field_descriptor(descriptor: &str) -> DescriptorType {
    match descriptor
        .chars()
        .next()
        .expect("Field descriptor is empty")
    {
        'I' => DescriptorType::Integer,
        'J' => DescriptorType::Long,
        'F' => DescriptorType::Float,
        'D' => DescriptorType::Double,
        'B' => DescriptorType::Byte,
        'C' => DescriptorType::Character,
        'S' => DescriptorType::Short,
        'Z' => DescriptorType::Boolean,
        _ => DescriptorType::Reference,
    }
}
