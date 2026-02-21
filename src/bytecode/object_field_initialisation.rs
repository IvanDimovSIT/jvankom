use std::rc::Rc;

use crate::{
    class_file::FieldAccessFlags,
    jvm_model::{DescriptorType, FieldInfo, HeapObject, JvmClass, JvmResult},
};

/// initialises an object with all it's fields
pub fn initialise_object_fields(class: Rc<JvmClass>, field_infos: &[FieldInfo]) -> HeapObject {
    let mut fields = Vec::with_capacity(field_infos.len());
    for field_info in field_infos {
        fields.push(field_info.descriptor_type.create_default_value());
    }

    HeapObject::Object { class, fields }
}

pub fn determine_non_static_field_types(class: &Rc<JvmClass>) -> JvmResult<Vec<FieldInfo>> {
    let mut field_infos = vec![];

    for f in &class.class_file.fields {
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

        let class_name = class
            .class_file
            .get_class_name()
            .expect("Expected class name")
            .to_owned();

        let field_info = FieldInfo {
            name: field_name,
            class: class_name,
            descriptor_type: parse_field_descriptor(descriptor),
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
        let super_types = determine_non_static_field_types(&super_class)?;
        field_infos.extend(super_types);
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
