use crate::{
    bytecode::{expect_int, expect_reference},
    jvm_heap::JvmHeap,
    jvm_model::{HeapObject, JvmResult, JvmThread, JvmValue},
};

pub fn register_natives(
    _thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _params: Vec<JvmValue>,
) -> JvmResult<()> {
    Ok(())
}

pub fn array_copy(
    _thread: &mut JvmThread,
    heap: &mut JvmHeap,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    let src_ref = expect_reference(params[0])?;
    let src_pos = expect_int(params[1])?;
    let dst_ref = expect_reference(params[2])?;
    let dst_pos = expect_int(params[3])?;
    let length = expect_int(params[4])?;

    let source = if let Some(r) = src_ref {
        heap.get(r).clone()
    } else {
        todo!("Throw NullPointerException")
    };
    let destination = if let Some(r) = dst_ref {
        heap.get(r)
    } else {
        todo!("Throw NullPointerException")
    };

    match (&source, destination) {
        (HeapObject::IntArray(items_s), HeapObject::IntArray(items_d)) => {
            copy_arr(items_s, src_pos, items_d, dst_pos, length)
        }
        (HeapObject::ByteArray(items_s), HeapObject::ByteArray(items_d)) => {
            copy_arr(items_s, src_pos, items_d, dst_pos, length)
        }
        (HeapObject::BooleanArray(items_s), HeapObject::BooleanArray(items_d)) => {
            copy_arr(items_s, src_pos, items_d, dst_pos, length)
        }
        (HeapObject::CharacterArray(items_s), HeapObject::CharacterArray(items_d)) => {
            copy_arr(items_s, src_pos, items_d, dst_pos, length)
        }
        (HeapObject::ShortArray(items_s), HeapObject::ShortArray(items_d)) => {
            copy_arr(items_s, src_pos, items_d, dst_pos, length)
        }
        (HeapObject::FloatArray(items_s), HeapObject::FloatArray(items_d)) => {
            copy_arr(items_s, src_pos, items_d, dst_pos, length)
        }
        (HeapObject::DoubleArray(items_s), HeapObject::DoubleArray(items_d)) => {
            copy_arr(items_s, src_pos, items_d, dst_pos, length)
        }
        (HeapObject::LongArray(items_s), HeapObject::LongArray(items_d)) => {
            copy_arr(items_s, src_pos, items_d, dst_pos, length)
        }
        (HeapObject::ObjectArray(items_s), HeapObject::ObjectArray(items_d)) => {
            copy_arr(items_s, src_pos, items_d, dst_pos, length)
        }
        _ => todo!("Throw exception"),
    }

    Ok(())
}

fn copy_arr<T: Copy>(src: &Vec<T>, s_start: i32, dst: &mut Vec<T>, d_start: i32, length: i32) {
    let src = &src[(s_start as usize)..((s_start + length) as usize)];
    let dst = &mut dst[(d_start as usize)..((d_start + length) as usize)];

    for (s, d) in src.iter().zip(dst) {
        *d = *s;
    }
}
