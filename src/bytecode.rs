use crate::{
    class_loader::ClassLoader,
    jvm_model::{JvmError, JvmHeap, JvmResult, JvmThread, JvmType, JvmValue},
};

pub const NOP: u8 = 0x00;
pub const ILOAD: u8 = 0x15;
pub const ILOAD_0: u8 = 0x1a;
pub const ILOAD_1: u8 = 0x1b;
pub const ILOAD_2: u8 = 0x1c;
pub const ILOAD_3: u8 = 0x1d;
pub const IADD: u8 = 0x60;
pub const IRETURN: u8 = 0xac;
pub const RETURN: u8 = 0xb1;

type BytecodeInstruction = fn(&mut JvmThread, &mut JvmHeap, &mut ClassLoader) -> JvmResult<()>;

pub const BYTECODE_TABLE: BytecodeTable = BytecodeTable::new();

pub struct BytecodeTable {
    table: [BytecodeInstruction; 256],
}
impl BytecodeTable {
    const fn new() -> Self {
        let mut table: [BytecodeInstruction; 256] = [handle_unrecognised_instruction; 256];
        let instructions: [(u8, BytecodeInstruction); _] = [
            (NOP, nop_instruction),
            (ILOAD, integer_load_n),
            (ILOAD_0, integer_load::<0>),
            (ILOAD_1, integer_load::<1>),
            (ILOAD_2, integer_load::<2>),
            (ILOAD_3, integer_load::<3>),
            (IADD, integer_add),
            (IRETURN, integer_return_instruction),
            (RETURN, return_instruction),
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

    pub fn execute_instruction(
        &self,
        instruction: u8,
        thread: &mut JvmThread,
        heap: &mut JvmHeap,
        class_loader: &mut ClassLoader,
    ) -> JvmResult<()> {
        let table_index = instruction as usize;
        let handler = self.table[table_index];
        handler(thread, heap, class_loader)
    }
}

fn nop_instruction(
    _thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    Ok(())
}

fn return_instruction(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();
    frame.should_return = true;

    Ok(())
}

fn integer_return_instruction(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();
    if let Some(value_to_return) = frame.operand_stack.pop() {
        expect_int(value_to_return)?;
        frame.should_return = true;
        frame.return_value = Some(value_to_return);
    } else {
        return Err(JvmError::NoOperandFound.bx());
    }

    Ok(())
}

fn integer_load_n(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();
    let bytecode = frame.class.methods[frame.method_index].get_bytecode(frame.bytecode_index);
    if frame.program_counter >= bytecode.code.len() {
        return Err(JvmError::ProgramCounterOutOfBounds {
            current_index: frame.program_counter,
            bytecode_len: bytecode.code.len(),
        }
        .bx());
    }
    let index_value = bytecode.code[frame.program_counter] as usize;
    frame.program_counter += 1;

    if let Some(value) = frame.local_variables.get(index_value) {
        expect_int(*value)?;
        frame.operand_stack.push(*value);
    } else {
        return Err(JvmError::NoLocalVariableFound.bx());
    }

    Ok(())
}

fn integer_load<const INDEX: usize>(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();

    if let Some(value) = frame.local_variables.get(INDEX) {
        expect_int(*value)?;
        frame.operand_stack.push(*value);
    } else {
        return Err(JvmError::NoLocalVariableFound.bx());
    }

    Ok(())
}

fn integer_add(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();

    let value_a = if let Some(a) = frame.operand_stack.pop() {
        expect_int(a)?
    } else {
        return Err(JvmError::NoOperandFound.bx());
    };

    let value_b = if let Some(b) = frame.operand_stack.pop() {
        expect_int(b)?
    } else {
        return Err(JvmError::NoOperandFound.bx());
    };

    let result_value = JvmValue::Int(value_a.wrapping_add(value_b));
    frame.operand_stack.push(result_value);

    Ok(())
}

fn handle_unrecognised_instruction(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();
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
