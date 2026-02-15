use std::rc::Rc;

use crate::{
    class_file::{ClassFile, FieldAccessFlags},
    class_loader::ClassLoader,
    jvm_model::{DescriptorType, FieldInfo, HeapObject, JvmResult},
};

/// initialises an object with all it's fields
pub fn initialise_object_fields(class: Rc<ClassFile>, field_infos: &[FieldInfo]) -> HeapObject {
    let mut fields = Vec::with_capacity(field_infos.len());
    for field_info in field_infos {
        fields.push(field_info.descriptor_type.create_default_value());
    }

    HeapObject::Object { class, fields }
}

pub fn determine_field_types(
    class: &ClassFile,
    class_loader: &mut ClassLoader,
) -> JvmResult<Vec<FieldInfo>> {
    let mut field_infos = vec![];

    for f in &class.fields {
        if f.access_flags.check_flag(FieldAccessFlags::STATIC_FLAG) {
            continue;
        }

        let descriptor = class
            .constant_pool
            .get_utf8(f.descriptor_index)
            .expect("Invalid descriptor index");

        let field_name = class
            .constant_pool
            .get_utf8(f.name_index)
            .expect("Expected field name")
            .to_owned();

        let class_name = class
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

    if let Some(super_class_name) = class.get_super_class_name()
        && super_class_name != "java/lang/Object"
    {
        let super_class = class_loader.get(super_class_name)?;
        let super_types = determine_field_types(&super_class.class, class_loader)?;
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
