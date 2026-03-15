use std::num::NonZeroUsize;

use crate::{
    bytecode::{expect_int, expect_reference},
    class_loader::ClassLoader,
    exceptions::throw_jvm_exception,
    jvm_heap::JvmHeap,
    jvm_model::{
        DescriptorType, HeapObject, JvmResult, JvmThread, JvmValue, NULL_POINTER_EXCEPTION_NAME,
        SYSTEM_CLASS_NAME,
    },
};

pub fn register_natives(
    _thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
    _params: Vec<JvmValue>,
) -> JvmResult<()> {
    Ok(())
}

pub fn set_out0(
    _thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    class_loader: &mut ClassLoader,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    set_system_field(class_loader, params, "out")
}

pub fn set_err0(
    _thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    class_loader: &mut ClassLoader,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    set_system_field(class_loader, params, "err")
}

fn set_system_field(
    class_loader: &mut ClassLoader,
    params: Vec<JvmValue>,
    field_name: &str,
) -> JvmResult<()> {
    let print_stream = expect_reference(params[0])?;
    let system_class = class_loader.get(SYSTEM_CLASS_NAME)?;
    let mut system_class_state = system_class.state.borrow_mut();
    let field = system_class_state
        .static_fields
        .as_mut()
        .expect("fields not initialised")
        .iter_mut()
        .find(|f| f.name == field_name && f.descriptor_type == DescriptorType::Reference)
        .expect("fields not found for java/lang/System");
    field.value = JvmValue::Reference(print_stream);

    Ok(())
}

enum TempArrayCopy {
    Int(Vec<i32>),
    Byte(Vec<i8>),
    Boolean(Vec<bool>),
    Character(Vec<u16>),
    Short(Vec<i16>),
    Float(Vec<f32>),
    Double(Vec<f64>),
    Long(Vec<i64>),
    Object(Vec<Option<NonZeroUsize>>),
}

pub fn array_copy(
    thread: &mut JvmThread,
    heap: &mut JvmHeap,
    class_loader: &mut ClassLoader,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    let src_ref = expect_reference(params[0])?;
    let src_pos = expect_int(params[1])?;
    let dst_ref = expect_reference(params[2])?;
    let dst_pos = expect_int(params[3])?;
    let length = expect_int(params[4])?;

    let src_r = if let Some(r) = src_ref {
        r
    } else {
        return throw_jvm_exception(thread, heap, class_loader, NULL_POINTER_EXCEPTION_NAME);
    };
    let dst_r = if let Some(d) = dst_ref {
        d
    } else {
        return throw_jvm_exception(thread, heap, class_loader, NULL_POINTER_EXCEPTION_NAME);
    };

    let src_pos = src_pos as usize;
    let end = src_pos + length as usize;
    let slice = match heap.get(src_r) {
        HeapObject::IntArray(a) => TempArrayCopy::Int(a[src_pos..end].to_vec()),
        HeapObject::ByteArray(a) => TempArrayCopy::Byte(a[src_pos..end].to_vec()),
        HeapObject::BooleanArray(a) => TempArrayCopy::Boolean(a[src_pos..end].to_vec()),
        HeapObject::CharacterArray(a) => TempArrayCopy::Character(a[src_pos..end].to_vec()),
        HeapObject::ShortArray(a) => TempArrayCopy::Short(a[src_pos..end].to_vec()),
        HeapObject::FloatArray(a) => TempArrayCopy::Float(a[src_pos..end].to_vec()),
        HeapObject::DoubleArray(a) => TempArrayCopy::Double(a[src_pos..end].to_vec()),
        HeapObject::LongArray(a) => TempArrayCopy::Long(a[src_pos..end].to_vec()),
        HeapObject::ObjectArray(a) => TempArrayCopy::Object(a.array[src_pos..end].to_vec()),
        _ => todo!("Throw exception"),
    };

    let dst_pos = dst_pos as usize;
    match (slice, heap.get(dst_r)) {
        (TempArrayCopy::Int(src), HeapObject::IntArray(dst)) => {
            dst[dst_pos..dst_pos + src.len()].copy_from_slice(&src)
        }
        (TempArrayCopy::Byte(src), HeapObject::ByteArray(dst)) => {
            dst[dst_pos..dst_pos + src.len()].copy_from_slice(&src)
        }
        (TempArrayCopy::Boolean(src), HeapObject::BooleanArray(dst)) => {
            dst[dst_pos..dst_pos + src.len()].copy_from_slice(&src)
        }
        (TempArrayCopy::Character(src), HeapObject::CharacterArray(dst)) => {
            dst[dst_pos..dst_pos + src.len()].copy_from_slice(&src)
        }
        (TempArrayCopy::Short(src), HeapObject::ShortArray(dst)) => {
            dst[dst_pos..dst_pos + src.len()].copy_from_slice(&src)
        }
        (TempArrayCopy::Float(src), HeapObject::FloatArray(dst)) => {
            dst[dst_pos..dst_pos + src.len()].copy_from_slice(&src)
        }
        (TempArrayCopy::Double(src), HeapObject::DoubleArray(dst)) => {
            dst[dst_pos..dst_pos + src.len()].copy_from_slice(&src)
        }
        (TempArrayCopy::Long(src), HeapObject::LongArray(dst)) => {
            dst[dst_pos..dst_pos + src.len()].copy_from_slice(&src)
        }
        (TempArrayCopy::Object(src), HeapObject::ObjectArray(dst)) => {
            dst.array[dst_pos..dst_pos + src.len()].clone_from_slice(&src)
        }
        _ => todo!("Throw exception"),
    }

    Ok(())
}
