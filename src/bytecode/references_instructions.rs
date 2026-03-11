use std::rc::Rc;

use crate::{
    class_cache::{CacheEntry, TypeInfo},
    class_loader::ClassLoader,
    field_initialisation::{determine_non_static_field_types, initialise_object_fields},
    initialise_class_and_rewind,
    jvm_model::{DescriptorType, JvmClass, ObjectArray, ObjectArrayType},
    throw_negative_array_size_exception, throw_null_pointer_exception,
};

use super::*;

pub mod field_instructions;
pub mod method_instructions;

const BOOLEAN_ARR: u8 = 4;
const CHAR_ARR: u8 = 5;
const FLOAT_ARR: u8 = 6;
const DOUBLE_ARR: u8 = 7;
const BYTE_ARR: u8 = 8;
const SHORT_ARR: u8 = 9;
const INT_ARR: u8 = 10;
const LONG_ARR: u8 = 11;

pub fn array_length_instruction(context: JvmContext) -> JvmResult<()> {
    const INSTRUCTION_SIZE: usize = 1;
    let frame = context.current_thread.top_frame();
    let array_ref = if let Some(array_ref) = pop_reference(frame)? {
        array_ref
    } else {
        throw_null_pointer_exception!(frame, context, INSTRUCTION_SIZE);
    };

    let array = context.heap.get(array_ref);
    let array_len = match array {
        HeapObject::IntArray(items) => items.len(),
        HeapObject::ByteArray(items) => items.len(),
        HeapObject::BooleanArray(items) => items.len(),
        HeapObject::CharacterArray(items) => items.len(),
        HeapObject::ShortArray(items) => items.len(),
        HeapObject::FloatArray(items) => items.len(),
        HeapObject::DoubleArray(items) => items.len(),
        HeapObject::LongArray(items) => items.len(),
        HeapObject::ObjectArray(arr) => arr.array.len(),
        _ => return Err(JvmError::ExpectedArray.bx()),
    } as i32;

    frame.operand_stack.push(JvmValue::Int(array_len));

    Ok(())
}

pub fn new_array_instruction(context: JvmContext) -> JvmResult<()> {
    const INSTRUCTION_SIZE: usize = 2;
    let frame = context.current_thread.top_frame();
    let array_type_value = read_u8_from_bytecode(frame);

    let operand_value = pop_int(frame)?;
    if operand_value < 0 {
        throw_negative_array_size_exception!(frame, context, INSTRUCTION_SIZE);
    }
    let array_size = operand_value as usize;

    let object = match array_type_value {
        BOOLEAN_ARR => HeapObject::BooleanArray(vec![false; array_size]),
        CHAR_ARR => HeapObject::CharacterArray(vec![0; array_size]),
        FLOAT_ARR => HeapObject::FloatArray(vec![0.0; array_size]),
        DOUBLE_ARR => HeapObject::DoubleArray(vec![0.0; array_size]),
        BYTE_ARR => HeapObject::ByteArray(vec![0; array_size]),
        SHORT_ARR => HeapObject::ShortArray(vec![0; array_size]),
        INT_ARR => HeapObject::IntArray(vec![0; array_size]),
        LONG_ARR => HeapObject::LongArray(vec![0; array_size]),
        _ => return Err(JvmError::InvalidArrayType(array_type_value).bx()),
    };

    let array_ref = context.heap.allocate(object);
    frame
        .operand_stack
        .push(JvmValue::Reference(Some(array_ref)));

    Ok(())
}

pub fn new_object_array_instruction(context: JvmContext) -> JvmResult<()> {
    const INSTRUCTION_SIZE: usize = 3;
    let frame = context.current_thread.top_frame();
    let unvalidated_index = read_u16_from_bytecode(frame);
    let type_info = if let Some(info) = frame.class.state.borrow().cache.get_type(unvalidated_index)
    {
        info.clone()
    } else {
        let array_type_ref = validate_cp_index(unvalidated_index)?;

        let arr_type = read_class_type(frame, array_type_ref)?;
        let (object_array_type, dimension) =
            determine_type_and_dimension_if_array(arr_type, context.class_loader)?;

        let type_info = TypeInfo {
            object_or_array: object_array_type,
            dimension,
        };

        frame.class.state.borrow_mut().cache.register(
            unvalidated_index,
            Rc::new(CacheEntry::Type(type_info.clone())),
        );

        if let ObjectArrayType::Class(jvm_class) = &type_info.object_or_array
            && !jvm_class.state.borrow().is_initialised
        {
            initialise_class_and_rewind!(frame, context, jvm_class, INSTRUCTION_SIZE);
        }

        type_info
    };

    let (array_type, array_dimension) = determine_object_array_type_and_dimension(type_info)?;

    let operand_value = pop_int(frame)?;
    if operand_value < 0 {
        throw_negative_array_size_exception!(frame, context, INSTRUCTION_SIZE);
    }
    let array_size = operand_value as usize;

    let object_array = ObjectArray {
        array: vec![None; array_size],
        dimension: array_dimension,
        object_array_type: array_type,
    };
    let object = HeapObject::ObjectArray(object_array);

    let array_ref = context.heap.allocate(object);
    frame
        .operand_stack
        .push(JvmValue::Reference(Some(array_ref)));

    Ok(())
}

pub fn new_instruction(context: JvmContext) -> JvmResult<()> {
    const INSTRUCTION_SIZE: usize = 3;
    let frame = context.current_thread.top_frame();
    let unvalidated_cp_index = read_u16_from_bytecode(frame);
    let new_object_class = if let Some(new_object_class) = frame
        .class
        .state
        .borrow()
        .cache
        .get_type(unvalidated_cp_index)
    {
        match &new_object_class.object_or_array {
            ObjectArrayType::Class(jvm_class) => jvm_class.clone(),
            _ => {
                return Err(JvmError::InvalidClassIndex(
                    NonZeroUsize::new(unvalidated_cp_index as usize).unwrap(),
                )
                .bx());
            }
        }
    } else {
        // find and load class
        let cp_index = validate_cp_index(unvalidated_cp_index)?;
        let class_name = read_class_type(frame, cp_index)?;

        let loaded_class = context.class_loader.get(class_name)?;
        frame.class.state.borrow_mut().cache.register(
            unvalidated_cp_index,
            Rc::new(CacheEntry::Type(TypeInfo {
                object_or_array: ObjectArrayType::Class(loaded_class.clone()),
                dimension: 0,
            })),
        );

        if !loaded_class.state.borrow().is_initialised {
            initialise_class_and_rewind!(frame, context, &loaded_class, INSTRUCTION_SIZE);
        }

        loaded_class
    };

    let object = if let Some(object) = new_object_class.state.borrow().default_object.clone() {
        object
    } else {
        let field_infos = determine_non_static_field_types(&new_object_class)?;
        let object = initialise_object_fields(new_object_class.clone(), &field_infos);
        let mut state_ref = new_object_class.state.borrow_mut();
        state_ref.default_object = Some(object.clone());
        state_ref.non_static_fields = Some(field_infos);

        object
    };

    let object_ref = context.heap.allocate(object);
    frame
        .operand_stack
        .push(JvmValue::Reference(Some(object_ref)));

    Ok(())
}

pub fn throw_exception_instruction(context: JvmContext) -> JvmResult<()> {
    const INSTRUCTION_SIZE: usize = 1;
    let frame = context.current_thread.top_frame();
    let exception_ref = if let Some(ex_ref) = pop_reference(frame)? {
        ex_ref
    } else {
        throw_null_pointer_exception!(frame, context, INSTRUCTION_SIZE);
    };
    let exception_obj = context.heap.get(exception_ref);
    let exception_class = match exception_obj {
        HeapObject::Object { class, fields: _ } => class,
        _ => todo!("Throw exception - not an exception"),
    };
    let throwable_interface = context.class_loader.get_throwable()?;
    if !JvmClass::is_sublcass_of(&throwable_interface, exception_class) {
        return Err(JvmError::ExpectedThrowable.bx());
    }

    frame.set_exception(exception_ref);

    Ok(())
}

pub fn instance_of_instruction(context: JvmContext) -> JvmResult<()> {
    const INSTRUCTION_SIZE: usize = 3;
    let frame = context.current_thread.top_frame();
    let unvalidated_cp_index = read_u16_from_bytecode(frame);
    let type_info = if let Some(type_info) = frame
        .class
        .state
        .borrow()
        .cache
        .get_type(unvalidated_cp_index)
    {
        type_info.clone()
    } else {
        let class_index = validate_cp_index(unvalidated_cp_index)?;
        let class_name = read_class_type(frame, class_index)?;

        let (expected_class, dimension_if_array) =
            determine_type_and_dimension_if_array(class_name, context.class_loader)?;

        let type_info = TypeInfo {
            object_or_array: expected_class,
            dimension: dimension_if_array,
        };

        frame.class.state.borrow_mut().cache.register(
            unvalidated_cp_index,
            Rc::new(CacheEntry::Type(type_info.clone())),
        );

        if let ObjectArrayType::Class(jvm_class) = &type_info.object_or_array
            && !jvm_class.state.borrow().is_initialised
        {
            initialise_class_and_rewind!(frame, context, jvm_class, INSTRUCTION_SIZE);
        }

        type_info
    };
    let object_ref = if let Some(reference) = pop_reference(frame)? {
        reference
    } else {
        frame.operand_stack.push(JvmValue::Int(0));
        return Ok(());
    };
    let object = context.heap.get(object_ref);
    let instance_of_result =
        check_instance_of(&type_info.object_or_array, type_info.dimension, object)?;
    frame.operand_stack.push(JvmValue::Int(instance_of_result));

    Ok(())
}

/// convert type reference to array type
fn determine_object_array_type_and_dimension(
    type_info: TypeInfo,
) -> JvmResult<(ObjectArrayType, NonZeroUsize)> {
    match type_info.object_or_array {
        ObjectArrayType::Primitive(_) => {
            if type_info.dimension == 0 {
                Err(JvmError::InvalidMultidimensionalPrimitiveArrayDimension.bx())
            } else {
                Ok((
                    type_info.object_or_array,
                    NonZeroUsize::new(type_info.dimension + 1).unwrap(),
                ))
            }
        }
        _ => Ok((
            type_info.object_or_array,
            NonZeroUsize::new(type_info.dimension + 1).unwrap(),
        )),
    }
}

/// determines the type based on a class/interface reference and it's dimension if it's an array (0 if not array)
fn determine_type_and_dimension_if_array(
    object_type: &str,
    class_loader: &mut ClassLoader,
) -> JvmResult<(ObjectArrayType, usize)> {
    if !object_type.starts_with('[') {
        let class_type = class_loader.get(object_type)?;
        return Ok((ObjectArrayType::Class(class_type), 0));
    }

    // for multidimensional arrays:
    let inner_type = object_type.trim_start_matches('[');
    let dimension = object_type.len() - inner_type.len();
    if inner_type.starts_with('L') {
        let inner_type_class = class_loader.get(&inner_type[1..(inner_type.len() - 1)])?;
        Ok((ObjectArrayType::Class(inner_type_class), dimension))
    } else {
        let primitive_type = inner_type
            .chars()
            .next()
            .expect("Excepted primitive type")
            .into();

        Ok((ObjectArrayType::Primitive(primitive_type), dimension))
    }
}

fn check_instance_of(
    expected_type: &ObjectArrayType,
    dimension_if_array: usize,
    object: &HeapObject,
) -> JvmResult<i32> {
    let matches = match object {
        HeapObject::Object { class, fields: _ } => {
            check_non_array_instance_of(class, expected_type, dimension_if_array)
        }
        HeapObject::IntArray(_) => {
            check_array_instance_of(DescriptorType::Integer, expected_type, dimension_if_array)
        }
        HeapObject::ByteArray(_) => {
            check_array_instance_of(DescriptorType::Byte, expected_type, dimension_if_array)
        }
        HeapObject::BooleanArray(_) => {
            check_array_instance_of(DescriptorType::Boolean, expected_type, dimension_if_array)
        }
        HeapObject::CharacterArray(_) => {
            check_array_instance_of(DescriptorType::Character, expected_type, dimension_if_array)
        }
        HeapObject::ShortArray(_) => {
            check_array_instance_of(DescriptorType::Short, expected_type, dimension_if_array)
        }
        HeapObject::FloatArray(_) => {
            check_array_instance_of(DescriptorType::Float, expected_type, dimension_if_array)
        }
        HeapObject::DoubleArray(_) => {
            check_array_instance_of(DescriptorType::Double, expected_type, dimension_if_array)
        }
        HeapObject::LongArray(_) => {
            check_array_instance_of(DescriptorType::Long, expected_type, dimension_if_array)
        }
        HeapObject::ObjectArray(object_array) => {
            check_object_array_instance_of(object_array, expected_type, dimension_if_array)
        }
    };

    let int = if matches { 1 } else { 0 };

    Ok(int)
}

fn check_object_array_instance_of(
    obj_array: &ObjectArray,
    expected_type: &ObjectArrayType,
    expected_dimension: usize,
) -> bool {
    let types_match = match expected_type {
        ObjectArrayType::Class(ex_jvm_class) => {
            if expected_dimension == 0 && ex_jvm_class.class_file.super_class_index.is_none() {
                return true;
            } else {
                match &obj_array.object_array_type {
                    ObjectArrayType::Class(jvm_class) => {
                        JvmClass::is_sublcass_of(ex_jvm_class, jvm_class)
                    }
                    _ => false,
                }
            }
        }
        ObjectArrayType::Primitive(ex_descriptor_type) => match &obj_array.object_array_type {
            ObjectArrayType::Primitive(descriptor_type) => descriptor_type == ex_descriptor_type,
            _ => false,
        },
    };
    let dimensions_match = expected_dimension == obj_array.dimension.get();
    types_match && dimensions_match
}

fn check_non_array_instance_of(
    obj_class: &Rc<JvmClass>,
    expected_type: &ObjectArrayType,
    expected_dimension: usize,
) -> bool {
    match expected_type {
        ObjectArrayType::Class(parent) => {
            expected_dimension == 0 && JvmClass::is_sublcass_of(parent, obj_class)
        }
        _ => false,
    }
}

fn check_array_instance_of(
    array_type: DescriptorType,
    expected_type: &ObjectArrayType,
    expected_dimension: usize,
) -> bool {
    let types_match = match expected_type {
        ObjectArrayType::Primitive(descriptor_type) => *descriptor_type == array_type,
        ObjectArrayType::Class(jvm_class) => {
            if jvm_class.class_file.super_class_index.is_none() && 0 == expected_dimension {
                return true; // early return
            } else {
                false
            }
        }
    };

    expected_dimension == 1 && types_match
}
