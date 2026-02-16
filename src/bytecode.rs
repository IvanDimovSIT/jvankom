use std::num::NonZeroUsize;

use crate::{
    bytecode::stack_instructions::pop_instruction,
    class_file::Bytecode,
    jvm_model::{HeapObject, JvmContext, JvmError, JvmResult, JvmStackFrame, JvmType, JvmValue},
};

use constants_instructions::*;
use control_instructions::*;
use load_instructions::*;
use math_instructions::*;
use references_instructions::*;
use store_instructions::*;

mod constants_instructions;
mod control_instructions;
mod load_instructions;
mod math_instructions;
mod method_descriptor_parser;
mod object_field_initialisation;
mod references_instructions;
mod stack_instructions;
mod store_instructions;

// From https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-7.html
pub const NOP: u8 = 0x00;
pub const ICONST_M1: u8 = 0x2;
pub const ICONST_0: u8 = 0x3;
pub const ICONST_1: u8 = 0x4;
pub const ICONST_2: u8 = 0x5;
pub const ICONST_3: u8 = 0x6;
pub const ICONST_4: u8 = 0x7;
pub const ICONST_5: u8 = 0x8;
pub const BIPUSH: u8 = 0x10;
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
pub const POP: u8 = 0x57;
pub const IADD: u8 = 0x60;
pub const ISUB: u8 = 0x64;
pub const IMUL: u8 = 0x68;
pub const IDIV: u8 = 0x6c;
pub const IREM: u8 = 0x70;
pub const INEG: u8 = 0x74;
pub const IRETURN: u8 = 0xac;
pub const ARETURN: u8 = 0xb0;
pub const RETURN: u8 = 0xb1;
pub const INVOKEVIRTUAL: u8 = 0xb6;
pub const INVOKESTATIC: u8 = 0xb8;
pub const NEW: u8 = 0xbb;
pub const NEWARRAY: u8 = 0xbc;

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
            (ICONST_M1, integer_const_instruction::<-1>),
            (ICONST_0, integer_const_instruction::<0>),
            (ICONST_1, integer_const_instruction::<1>),
            (ICONST_2, integer_const_instruction::<2>),
            (ICONST_3, integer_const_instruction::<3>),
            (ICONST_4, integer_const_instruction::<4>),
            (ICONST_5, integer_const_instruction::<5>),
            (BIPUSH, bipush_instruction),
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
            (ISTORE, store_integer_n_instruction),
            (ASTORE, store_reference_n_instruction),
            (IADD, integer_add),
            (ISUB, integer_subtract),
            (IMUL, integer_muliply),
            (INEG, integer_negate),
            (IDIV, integer_divide),
            (IREM, integer_remainder),
            (ISTORE_0, store_integer_instruction::<0>),
            (ISTORE_1, store_integer_instruction::<1>),
            (ISTORE_2, store_integer_instruction::<2>),
            (ISTORE_3, store_integer_instruction::<3>),
            (ASTORE_0, store_reference_instruction::<0>),
            (ASTORE_1, store_reference_instruction::<1>),
            (ASTORE_2, store_reference_instruction::<2>),
            (ASTORE_3, store_reference_instruction::<3>),
            (IASTORE, store_integer_array_instruction),
            (POP, pop_instruction),
            (IRETURN, integer_return_instruction),
            (ARETURN, object_return_instruction),
            (RETURN, return_instruction),
            (INVOKEVIRTUAL, invoke_virtual_instruction),
            (INVOKESTATIC, invoke_static_instruction),
            (NEW, new_instruction),
            (NEWARRAY, new_array_instruction),
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
    let frame = context.current_thread.peek().unwrap();
    let bytecode = frame.class.methods[frame.method_index].get_bytecode(frame.bytecode_index);
    assert!(frame.program_counter > 0);
    let previous_index = frame.program_counter.saturating_sub(1);
    let unrecognised_bytecode = bytecode.code[previous_index];

    let instruction_fn_ptr =
        BYTECODE_TABLE.table[unrecognised_bytecode as usize] as *const BytecodeInstruction;
    let expected_fn_ptr = handle_unrecognised_instruction as *const BytecodeInstruction;
    assert_eq!(expected_fn_ptr, instruction_fn_ptr);

    Err(JvmError::UnimplementedInstruction(unrecognised_bytecode).bx())
}

/// read u16 from 2 bytecode values (moves PC forward by 2)
#[inline]
fn read_u16_from_bytecode(frame: &mut JvmStackFrame) -> u16 {
    let bytecode = frame.class.methods[frame.method_index].get_bytecode(frame.bytecode_index);

    let index_byte1 = bytecode.code[frame.program_counter] as u16;
    frame.program_counter += 1;
    debug_assert!(frame.program_counter < bytecode.code.len());

    let index_byte2 = bytecode.code[frame.program_counter] as u16;
    frame.program_counter += 1;
    debug_assert!(frame.program_counter < bytecode.code.len());

    (index_byte1 << 8) | index_byte2
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
fn pop_int(frame: &mut JvmStackFrame) -> JvmResult<i32> {
    if let Some(a) = frame.operand_stack.pop() {
        expect_int(a)
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
fn expect_reference(value: JvmValue) -> JvmResult<Option<NonZeroUsize>> {
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
fn expect_int(value: JvmValue) -> JvmResult<i32> {
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
fn expect_long(value: JvmValue) -> JvmResult<i64> {
    match value {
        JvmValue::Long(v) => Ok(v),
        _ => Err(JvmError::TypeError {
            expected: JvmType::Long,
            found: value.get_type(),
        }
        .bx()),
    }
}
