use crate::{
    jvm_model::{DescriptorType, JvmClass, ObjectArray, ObjectArrayType},
    throw_array_index_out_of_bounds_exception, throw_array_store_exception,
    throw_null_pointer_exception,
};

use super::*;

#[inline]
fn store_generic_array_instruction<P, F, T>(
    context: JvmContext,
    pop_generic: P,
    get_array_fn: F,
) -> JvmResult<()>
where
    P: FnOnce(&mut JvmStackFrame) -> JvmResult<T>,
    F: FnOnce(&mut HeapObject) -> Option<&mut Vec<T>>,
{
    let frame = context.current_thread.peek().unwrap();

    let value = pop_generic(frame)?;
    let index = pop_int(frame)?;
    let array_ref = if let Some(array_ref) = pop_reference(frame)? {
        array_ref
    } else {
        throw_null_pointer_exception!(frame, context, 1);
    };

    let array = if let Some(arr) = get_array_fn(context.heap.get(array_ref)) {
        arr
    } else {
        throw_array_store_exception!(frame, context, 1);
    };

    if index < 0 || index as usize >= array.len() {
        throw_array_index_out_of_bounds_exception!(frame, context, 1);
    }
    array[index as usize] = value;

    Ok(())
}

pub fn store_long_array_instruction(context: JvmContext) -> JvmResult<()> {
    store_generic_array_instruction(context, pop_long, |obj| match obj {
        HeapObject::LongArray(items) => Some(items),
        _ => None,
    })
}

pub fn store_integer_array_instruction(context: JvmContext) -> JvmResult<()> {
    store_generic_array_instruction(context, pop_int, |obj| match obj {
        HeapObject::IntArray(items) => Some(items),
        _ => None,
    })
}

pub fn store_character_array_instruction(context: JvmContext) -> JvmResult<()> {
    store_generic_array_instruction(
        context,
        |frame| Ok(pop_int(frame)? as u16),
        |obj| match obj {
            HeapObject::CharacterArray(items) => Some(items),
            _ => None,
        },
    )
}

pub fn store_object_array_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();

    let value = pop_reference(frame)?;

    let expected_storage_type = if let Some(non_null) = value {
        Some(get_valid_object_array_store_for_element(
            context.heap.get(non_null),
        ))
    } else {
        None
    };

    let index = pop_int(frame)?;
    let array_ref = if let Some(array_ref) = pop_reference(frame)? {
        array_ref
    } else {
        throw_null_pointer_exception!(frame, context, 1);
    };

    let obj_array = match context.heap.get(array_ref) {
        HeapObject::ObjectArray(object_array) => object_array,
        _ => throw_array_store_exception!(frame, context, 1),
    };

    if index < 0 || index as usize >= obj_array.array.len() {
        throw_array_index_out_of_bounds_exception!(frame, context, 1);
    }
    let is_storage_valid = if let Some((expected_type, expected_dimension)) = expected_storage_type
    {
        check_object_array_type_matches_expected_element_type(
            obj_array,
            &expected_type,
            expected_dimension,
        )
    } else {
        true //array type always valid for null
    };

    if !is_storage_valid {
        throw_array_store_exception!(frame, context, 1);
    }

    obj_array.array[index as usize] = value;

    Ok(())
}

fn check_object_array_type_matches_expected_element_type(
    obj_array: &ObjectArray,
    expected_type: &ObjectArrayType,
    expected_dimension: usize,
) -> bool {
    obj_array.dimension.get() == expected_dimension
        && match &obj_array.object_array_type {
            ObjectArrayType::Class(jvm_class) => match expected_type {
                ObjectArrayType::Class(ex_jvm_class) => {
                    JvmClass::is_sublcass_of(jvm_class, ex_jvm_class)
                }
                _ => false,
            },
            ObjectArrayType::Primitive(descriptor_type) => match expected_type {
                ObjectArrayType::Primitive(ex_descriptor_type) => {
                    *ex_descriptor_type == *descriptor_type
                }
                _ => false,
            },
        }
}

/// returns the expected array type with it's dimension needed to store the element
fn get_valid_object_array_store_for_element(element: &HeapObject) -> (ObjectArrayType, usize) {
    match element {
        HeapObject::Object { class, fields: _ } => (ObjectArrayType::Class(class.clone()), 1),
        HeapObject::IntArray(_) => (ObjectArrayType::Primitive(DescriptorType::Integer), 2),
        HeapObject::ByteArray(_) => (ObjectArrayType::Primitive(DescriptorType::Byte), 2),
        HeapObject::BooleanArray(_) => (ObjectArrayType::Primitive(DescriptorType::Boolean), 2),
        HeapObject::CharacterArray(_) => (ObjectArrayType::Primitive(DescriptorType::Character), 2),
        HeapObject::ShortArray(_) => (ObjectArrayType::Primitive(DescriptorType::Short), 2),
        HeapObject::FloatArray(_) => (ObjectArrayType::Primitive(DescriptorType::Float), 2),
        HeapObject::DoubleArray(_) => (ObjectArrayType::Primitive(DescriptorType::Double), 2),
        HeapObject::LongArray(_) => (ObjectArrayType::Primitive(DescriptorType::Long), 2),
        HeapObject::ObjectArray(object_array) => (
            object_array.object_array_type.clone(),
            object_array.dimension.get() + 1,
        ),
    }
}

#[inline]
fn store_generic_n_instruction<P, W, T>(
    context: JvmContext,
    pop_generic: P,
    wrap_value: W,
) -> JvmResult<()>
where
    P: FnOnce(&mut JvmStackFrame) -> JvmResult<T>,
    W: FnOnce(T) -> JvmValue,
{
    let frame = context.current_thread.peek().unwrap();
    let index_value = read_u8_from_bytecode(frame) as usize;
    let generic_value = pop_generic(frame)?;

    frame.local_variables[index_value] = wrap_value(generic_value);

    Ok(())
}

#[inline]
fn store_generic_instruction<const INDEX: usize, P, W, T>(
    context: JvmContext,
    pop_generic: P,
    wrap_value: W,
) -> JvmResult<()>
where
    P: FnOnce(&mut JvmStackFrame) -> JvmResult<T>,
    W: FnOnce(T) -> JvmValue,
{
    let frame = context.current_thread.peek().unwrap();

    let value = pop_generic(frame)?;
    debug_assert!(INDEX < frame.local_variables.len());
    frame.local_variables[INDEX] = wrap_value(value);

    Ok(())
}

pub fn store_reference_n_instruction(context: JvmContext) -> JvmResult<()> {
    store_generic_n_instruction(context, pop_reference, JvmValue::Reference)
}

pub fn store_reference_instruction<const INDEX: usize>(context: JvmContext) -> JvmResult<()> {
    store_generic_instruction::<INDEX, _, _, _>(context, pop_reference, JvmValue::Reference)
}

pub fn store_integer_n_instruction(context: JvmContext) -> JvmResult<()> {
    store_generic_n_instruction(context, pop_int, JvmValue::Int)
}

pub fn store_integer_instruction<const INDEX: usize>(context: JvmContext) -> JvmResult<()> {
    store_generic_instruction::<INDEX, _, _, _>(context, pop_int, JvmValue::Int)
}
