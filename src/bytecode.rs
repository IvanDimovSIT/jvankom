use std::num::NonZeroUsize;

use crate::{
    bytecode::{
        references_instructions::field_instructions::*,
        references_instructions::method_instructions::*,
        stack_instructions::{dup_instruction, dup_x1_instruction, pop_instruction},
    },
    jvm_model::{HeapObject, JvmContext, JvmError, JvmResult, JvmStackFrame, JvmType, JvmValue},
};

use comparisons_instructions::*;
use constants_instructions::*;
use control_instructions::*;
use conversions_instructions::*;
use extended_instructions::*;
use load_instructions::*;
use math_instructions::*;
use references_instructions::*;
use store_instructions::*;

mod access_check;
mod comparisons_instructions;
mod constants_instructions;
mod control_instructions;
mod conversions_instructions;
mod extended_instructions;
mod load_instructions;
mod math_instructions;
mod method_descriptor_parser;
mod references_instructions;
mod stack_instructions;
mod store_instructions;

// From https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-7.html
pub const NOP: u8 = 0x00;
pub const ACONST_NULL: u8 = 0x1;
pub const ICONST_M1: u8 = 0x2;
pub const ICONST_0: u8 = 0x3;
pub const ICONST_1: u8 = 0x4;
pub const ICONST_2: u8 = 0x5;
pub const ICONST_3: u8 = 0x6;
pub const ICONST_4: u8 = 0x7;
pub const ICONST_5: u8 = 0x8;
pub const BIPUSH: u8 = 0x10;
pub const SIPUSH: u8 = 0x11;
pub const LDC: u8 = 0x12;
pub const LDC2_W: u8 = 0x14;
pub const ILOAD: u8 = 0x15;
pub const ALOAD: u8 = 0x19;
pub const ILOAD_0: u8 = 0x1a;
pub const ILOAD_1: u8 = 0x1b;
pub const ILOAD_2: u8 = 0x1c;
pub const ILOAD_3: u8 = 0x1d;
pub const ALOAD_0: u8 = 0x2a;
pub const ALOAD_1: u8 = 0x2b;
pub const ALOAD_2: u8 = 0x2c;
pub const ALOAD_3: u8 = 0x2d;
pub const IALOAD: u8 = 0x2e;
pub const CALOAD: u8 = 0x34;
pub const ISTORE: u8 = 0x36;
pub const ASTORE: u8 = 0x3a;
pub const ISTORE_0: u8 = 0x3b;
pub const ISTORE_1: u8 = 0x3c;
pub const ISTORE_2: u8 = 0x3d;
pub const ISTORE_3: u8 = 0x3e;
pub const ASTORE_0: u8 = 0x4b;
pub const ASTORE_1: u8 = 0x4c;
pub const ASTORE_2: u8 = 0x4d;
pub const ASTORE_3: u8 = 0x4e;
pub const IASTORE: u8 = 0x4f;
pub const LASTORE: u8 = 0x50;
pub const AASTORE: u8 = 0x53;
pub const CASTORE: u8 = 0x55;
pub const POP: u8 = 0x57;
pub const DUP: u8 = 0x59;
pub const DUP_X1: u8 = 0x5a;
pub const IADD: u8 = 0x60;
pub const LADD: u8 = 0x61;
pub const ISUB: u8 = 0x64;
pub const IMUL: u8 = 0x68;
pub const IDIV: u8 = 0x6c;
pub const IREM: u8 = 0x70;
pub const INEG: u8 = 0x74;
pub const LSHL: u8 = 0x79;
pub const LAND: u8 = 0x7f;
pub const IINC: u8 = 0x84;
pub const I2L: u8 = 0x85;
pub const IFEQ: u8 = 0x99;
pub const IFNE: u8 = 0x9a;
pub const IFLT: u8 = 0x9b;
pub const IFGE: u8 = 0x9c;
pub const IFGT: u8 = 0x9d;
pub const IFLE: u8 = 0x9e;
pub const IF_ICMPEQ: u8 = 0x9f;
pub const IF_ICMPNE: u8 = 0xa0;
pub const IF_ICMPLT: u8 = 0xa1;
pub const IF_ICMPGE: u8 = 0xa2;
pub const IF_ICMPGT: u8 = 0xa3;
pub const IF_ICMPLE: u8 = 0xa4;
pub const GOTO: u8 = 0xa7;
pub const IRETURN: u8 = 0xac;
pub const DRETURN: u8 = 0xaf;
pub const ARETURN: u8 = 0xb0;
pub const RETURN: u8 = 0xb1;
pub const GETSTATIC: u8 = 0xb2;
pub const PUTSTATIC: u8 = 0xb3;
pub const GETFIELD: u8 = 0xb4;
pub const PUTFIELD: u8 = 0xb5;
pub const INVOKEVIRTUAL: u8 = 0xb6;
pub const INVOKESPECIAL: u8 = 0xb7;
pub const INVOKESTATIC: u8 = 0xb8;
pub const INVOKEINTERFACE: u8 = 0xb9;
pub const NEW: u8 = 0xbb;
pub const NEWARRAY: u8 = 0xbc;
pub const ANEWARRAY: u8 = 0xbd;
pub const ARRAYLENGTH: u8 = 0xbe;
pub const ATHROW: u8 = 0xbf;
pub const INSTANCEOF: u8 = 0xc1;
pub const IFNULL: u8 = 0xc6;
pub const IFNONNULL: u8 = 0xc7;

type BytecodeInstruction = fn(JvmContext) -> JvmResult<()>;

pub const BYTECODE_TABLE: BytecodeTable = BytecodeTable::new();

pub struct BytecodeTable {
    table: [BytecodeInstruction; 256],
}
impl BytecodeTable {
    const fn new() -> Self {
        let mut table: [BytecodeInstruction; 256] = [handle_unrecognised_instruction; 256];
        let instructions: [(u8, BytecodeInstruction); _] = [
            (NOP, nop_instruction),
            (ACONST_NULL, null_const_instruction),
            (ICONST_M1, integer_const_instruction::<-1>),
            (ICONST_0, integer_const_instruction::<0>),
            (ICONST_1, integer_const_instruction::<1>),
            (ICONST_2, integer_const_instruction::<2>),
            (ICONST_3, integer_const_instruction::<3>),
            (ICONST_4, integer_const_instruction::<4>),
            (ICONST_5, integer_const_instruction::<5>),
            (BIPUSH, bipush_instruction),
            (SIPUSH, sipush_instruction),
            (LDC, ldc_instruction),
            (LDC2_W, ldc2w_instruction),
            (ILOAD, integer_load_n),
            (ALOAD, reference_load_n),
            (ILOAD_0, integer_load::<0>),
            (ILOAD_1, integer_load::<1>),
            (ILOAD_2, integer_load::<2>),
            (ILOAD_3, integer_load::<3>),
            (ALOAD_0, reference_load_instruction::<0>),
            (ALOAD_1, reference_load_instruction::<1>),
            (ALOAD_2, reference_load_instruction::<2>),
            (ALOAD_3, reference_load_instruction::<3>),
            (IALOAD, load_integer_array_instruction),
            (CALOAD, load_character_array_instruction),
            (ISTORE, store_integer_n_instruction),
            (ASTORE, store_reference_n_instruction),
            (IADD, integer_add_instruction),
            (LADD, long_add_instruction),
            (ISUB, integer_subtract_instruction),
            (IMUL, integer_muliply_instruction),
            (INEG, integer_negate_instruction),
            (LSHL, shift_left_long_instruction),
            (LAND, long_and_instruction),
            (IDIV, integer_divide_instruction),
            (IREM, integer_remainder_instruction),
            (ISTORE_0, store_integer_instruction::<0>),
            (ISTORE_1, store_integer_instruction::<1>),
            (ISTORE_2, store_integer_instruction::<2>),
            (ISTORE_3, store_integer_instruction::<3>),
            (ASTORE_0, store_reference_instruction::<0>),
            (ASTORE_1, store_reference_instruction::<1>),
            (ASTORE_2, store_reference_instruction::<2>),
            (ASTORE_3, store_reference_instruction::<3>),
            (IASTORE, store_integer_array_instruction),
            (LASTORE, store_long_array_instruction),
            (AASTORE, store_object_array_instruction),
            (CASTORE, store_character_array_instruction),
            (POP, pop_instruction),
            (DUP, dup_instruction),
            (DUP_X1, dup_x1_instruction),
            (IINC, increment_instruction),
            (I2L, int_to_long_instruction),
            (IFEQ, if_equals_instruction),
            (IFNE, if_not_equals_instruction),
            (IFLT, if_less_than_instruction),
            (IFGE, if_greater_than_or_equals_instruction),
            (IFGT, if_greater_than_instruction),
            (IFLE, if_less_than_or_equals_instruction),
            (IF_ICMPEQ, if_compare_equals_instruction),
            (IF_ICMPNE, if_compare_not_equals_instruction),
            (IF_ICMPLT, if_compare_less_than_instruction),
            (IF_ICMPGE, if_compare_greater_than_or_equals_instruction),
            (IF_ICMPGT, if_compare_greater_than_instruction),
            (IF_ICMPLE, if_compare_less_than_or_equals_instruction),
            (GOTO, goto_instruction),
            (IRETURN, integer_return_instruction),
            (DRETURN, double_return_instruction),
            (ARETURN, object_return_instruction),
            (RETURN, return_instruction),
            (GETSTATIC, get_static_instruction),
            (PUTSTATIC, put_static_instruction),
            (GETFIELD, get_field_instruction),
            (PUTFIELD, put_field_instruction),
            (INVOKEVIRTUAL, invoke_virtual_instruction),
            (INVOKESPECIAL, invoke_static_or_special_instruction::<true>),
            (INVOKESTATIC, invoke_static_or_special_instruction::<false>),
            (INVOKEINTERFACE, invoke_interface),
            (NEW, new_instruction),
            (NEWARRAY, new_array_instruction),
            (ANEWARRAY, new_object_array_instruction),
            (ARRAYLENGTH, array_length_instruction),
            (ATHROW, throw_exception_instruction),
            (INSTANCEOF, instance_of_instruction),
            (IFNULL, if_null_instruction),
            (IFNONNULL, if_not_null_instruction),
        ];

        let mut i = 0;
        while i < instructions.len() {
            let instruction = instructions[i].0;
            let function = instructions[i].1;
            table[instruction as usize] = function;
            i += 1;
        }

        Self { table }
    }

    pub fn execute_instruction(&self, instruction: u8, jvm_context: JvmContext) -> JvmResult<()> {
        let table_index = instruction as usize;
        let handler = self.table[table_index];
        handler(jvm_context)
    }
}

fn handle_unrecognised_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    let bytecode =
        frame.class.class_file.methods[frame.method_index].get_bytecode(frame.bytecode_index);
    assert!(frame.program_counter > 0);
    let previous_index = frame.program_counter.saturating_sub(1);
    let unrecognised_bytecode = bytecode.code[previous_index];

    let instruction_fn_ptr =
        BYTECODE_TABLE.table[unrecognised_bytecode as usize] as *const BytecodeInstruction;
    let expected_fn_ptr = handle_unrecognised_instruction as *const BytecodeInstruction;
    assert_eq!(expected_fn_ptr, instruction_fn_ptr);

    Err(JvmError::UnimplementedInstruction(unrecognised_bytecode).bx())
}

/// initialises the class and rewinds the instruction, where $size is the size of the instruction
#[macro_export]
macro_rules! initialise_class_and_rewind {
    ($frame:expr, $context:expr, $jvm_class:expr, $size:expr) => {{
        const _CHECK_SIZE: () = assert!($size > 0);

        $frame.program_counter -= $size; // rewind
        return $crate::jvm::Jvm::initialise_class(
            $context.current_thread,
            $jvm_class,
            $context.class_loader,
            $jvm_class.class_file.get_class_name(),
        );
    }};
}

fn validate_cp_index(unvalidated_cp_index: u16) -> JvmResult<NonZeroUsize> {
    if let Some(index) = NonZeroUsize::new(unvalidated_cp_index as usize) {
        Ok(index)
    } else {
        Err(JvmError::InvalidConstantPoolIndex.bx())
    }
}

fn read_class_type(frame: &mut JvmStackFrame, type_index: NonZeroUsize) -> JvmResult<&str> {
    if let Some(arr_type) = frame
        .class
        .class_file
        .constant_pool
        .get_class_name(type_index)
    {
        Ok(arr_type)
    } else {
        Err(JvmError::InvalidClassIndex(type_index).bx())
    }
}

/// read u8 from a bytecode value (moves PC forward by 1)
#[inline]
fn read_u8_from_bytecode(frame: &mut JvmStackFrame) -> u8 {
    let bytecode =
        frame.class.class_file.methods[frame.method_index].get_bytecode(frame.bytecode_index);

    let value = bytecode.code[frame.program_counter];
    frame.program_counter += 1;
    debug_assert!(frame.program_counter < bytecode.code.len());

    value
}

/// read u16 from 2 bytecode values (moves PC forward by 2)
#[inline]
fn read_u16_from_bytecode(frame: &mut JvmStackFrame) -> u16 {
    let bytecode =
        frame.class.class_file.methods[frame.method_index].get_bytecode(frame.bytecode_index);

    let index_byte1 = bytecode.code[frame.program_counter] as u16;
    frame.program_counter += 1;
    debug_assert!(frame.program_counter < bytecode.code.len());

    let index_byte2 = bytecode.code[frame.program_counter] as u16;
    frame.program_counter += 1;
    debug_assert!(frame.program_counter < bytecode.code.len());

    (index_byte1 << 8) | index_byte2
}

#[inline]
fn read_i16_from_bytecode(frame: &mut JvmStackFrame) -> i16 {
    read_u16_from_bytecode(frame) as i16
}

#[inline]
fn pop_long(frame: &mut JvmStackFrame) -> JvmResult<i64> {
    if let Some(a) = frame.operand_stack.pop() {
        expect_long(a)
    } else {
        Err(JvmError::NoOperandFound.bx())
    }
}

#[inline]
fn pop_any(frame: &mut JvmStackFrame) -> JvmResult<JvmValue> {
    if let Some(value) = frame.operand_stack.pop() {
        Ok(value)
    } else {
        Err(JvmError::NoOperandFound.bx())
    }
}

#[inline]
fn pop_int(frame: &mut JvmStackFrame) -> JvmResult<i32> {
    if let Some(a) = frame.operand_stack.pop() {
        expect_int(a)
    } else {
        Err(JvmError::NoOperandFound.bx())
    }
}

#[inline]
fn pop_double(frame: &mut JvmStackFrame) -> JvmResult<f64> {
    if let Some(a) = frame.operand_stack.pop() {
        expect_double(a)
    } else {
        Err(JvmError::NoOperandFound.bx())
    }
}

#[inline]
fn pop_float(frame: &mut JvmStackFrame) -> JvmResult<f32> {
    if let Some(a) = frame.operand_stack.pop() {
        expect_float(a)
    } else {
        Err(JvmError::NoOperandFound.bx())
    }
}

#[inline]
fn pop_reference(frame: &mut JvmStackFrame) -> JvmResult<Option<NonZeroUsize>> {
    if let Some(a) = frame.operand_stack.pop() {
        expect_reference(a)
    } else {
        Err(JvmError::NoOperandFound.bx())
    }
}

#[inline]
pub fn expect_float(value: JvmValue) -> JvmResult<f32> {
    match value {
        JvmValue::Float(float) => Ok(float),
        _ => Err(JvmError::TypeError {
            expected: JvmType::Float,
            found: value.get_type(),
        }
        .bx()),
    }
}

#[inline]
pub fn expect_double(value: JvmValue) -> JvmResult<f64> {
    match value {
        JvmValue::Double(double) => Ok(double),
        _ => Err(JvmError::TypeError {
            expected: JvmType::Double,
            found: value.get_type(),
        }
        .bx()),
    }
}

#[inline]
pub fn expect_reference(value: JvmValue) -> JvmResult<Option<NonZeroUsize>> {
    match value {
        JvmValue::Reference(reference) => Ok(reference),
        _ => Err(JvmError::TypeError {
            expected: JvmType::Reference,
            found: value.get_type(),
        }
        .bx()),
    }
}

#[inline]
pub fn expect_int(value: JvmValue) -> JvmResult<i32> {
    match value {
        JvmValue::Int(v) => Ok(v),
        _ => Err(JvmError::TypeError {
            expected: JvmType::Int,
            found: value.get_type(),
        }
        .bx()),
    }
}

#[inline]
pub fn expect_long(value: JvmValue) -> JvmResult<i64> {
    match value {
        JvmValue::Long(v) => Ok(v),
        _ => Err(JvmError::TypeError {
            expected: JvmType::Long,
            found: value.get_type(),
        }
        .bx()),
    }
}
