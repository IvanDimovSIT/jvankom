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
            .expect_utf8(field.name_index)
            .to_owned();
        let descriptor = class_file.constant_pool.expect_utf8(field.descriptor_index);

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
            .expect_utf8(f.descriptor_index);

        let field_name = class
            .class_file
            .constant_pool
            .expect_utf8(f.name_index)
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

/// uses the first character of the descritor to determine the type
fn parse_field_descriptor(descriptor: &str) -> DescriptorType {
    descriptor
        .chars()
        .next()
        .expect("Field descriptor is empty")
        .into()
}

#[cfg(test)]
mod tests {
    use crate::{
        class_loader::{ClassLoader, ClassSource},
        jvm_model::{OBJECT_CLASS_NAME, STRING_CLASS_NAME},
    };

    use super::*;

    #[test]
    fn test_determine_static_fields_string() {
        test_determine_static_fields_helper(
            STRING_CLASS_NAME,
            &[
                ("serialVersionUID", DescriptorType::Long),
                ("serialPersistentFields", DescriptorType::Reference),
                ("CASE_INSENSITIVE_ORDER", DescriptorType::Reference),
            ],
        );
    }

    #[test]
    fn test_determine_static_fields_object() {
        test_determine_static_fields_helper(OBJECT_CLASS_NAME, &[]);
    }

    #[test]
    fn test_determine_static_fields_math() {
        test_determine_static_fields_helper(
            "java/lang/Void",
            &[("TYPE", DescriptorType::Reference)],
        );
    }

    fn test_determine_static_fields_helper(
        class_name: &str,
        expected_fields: &[(&str, DescriptorType)],
    ) {
        let mut class_loader =
            ClassLoader::new(vec![ClassSource::Jar("java_libraries/rt.jar".to_owned())]).unwrap();

        let class = class_loader.get(class_name).unwrap();

        let fields = determine_static_fields(&class.class_file);

        assert_eq!(expected_fields.len(), fields.len());
        for (name, desc) in expected_fields {
            let present = fields
                .iter()
                .find(|field| field.name == *name && field.descriptor_type == *desc)
                .is_some();
            assert!(present);
        }
    }
}
