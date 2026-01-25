use crate::{
    class_loader::ClassLoader,
    jvm_model::{JvmError, JvmHeap, JvmThread, JvmType, JvmValue},
};

type BytecodeInstruction =
    fn(&mut JvmThread, &mut JvmHeap, &mut ClassLoader) -> Result<(), Box<JvmError>>;

pub const BYTECODE_TABLE: BytecodeTable = BytecodeTable::new();

pub struct BytecodeTable {
    table: [BytecodeInstruction; 256],
}
impl BytecodeTable {
    const fn new() -> Self {
        let mut table: [BytecodeInstruction; 256] = [handle_unrecognised_instruction; 256];
        table[0x00] = nop_instruction;
        table[0x15] = integer_load_n;
        table[0x1a] = integer_load::<0>;
        table[0x1b] = integer_load::<1>;
        table[0x1c] = integer_load::<2>;
        table[0x1d] = integer_load::<3>;
        table[0x60] = integer_add;
        table[0xac] = integer_return_instruction;
        table[0xb1] = return_instruction;

        Self { table }
    }

    pub fn execute_instruction(
        &self,
        instruction: u8,
        thread: &mut JvmThread,
        heap: &mut JvmHeap,
        class_loader: &mut ClassLoader,
    ) -> Result<(), Box<JvmError>> {
        let table_index = instruction as usize;
        let handler = self.table[table_index];
        handler(thread, heap, class_loader)
    }
}

fn nop_instruction(
    _thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> Result<(), Box<JvmError>> {
    Ok(())
}

fn return_instruction(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> Result<(), Box<JvmError>> {
    let frame = thread.peek().unwrap();
    frame.should_return = true;

    Ok(())
}

fn integer_return_instruction(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> Result<(), Box<JvmError>> {
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
) -> Result<(), Box<JvmError>> {
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
) -> Result<(), Box<JvmError>> {
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
) -> Result<(), Box<JvmError>> {
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

    let result_value = JvmValue::Int(value_a + value_b);
    frame.operand_stack.push(result_value);

    Ok(())
}

fn handle_unrecognised_instruction(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> Result<(), Box<JvmError>> {
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
fn expect_int(value: JvmValue) -> Result<i32, Box<JvmError>> {
    match value {
        JvmValue::Int(v) => Ok(v),
        _ => Err(JvmError::TypeError {
            expected: JvmType::Int,
            found: value.get_type(),
        }
        .bx()),
    }
}
