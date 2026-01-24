use crate::{
    class_loader::ClassLoader,
    jvm_model::{JvmError, JvmHeap, JvmThread},
};

type BytecodeInstruction =
    fn(&mut JvmThread, &mut JvmHeap, &mut ClassLoader) -> Result<(), Box<JvmError>>;

pub const BYTECODE_TABLE: BytecodeTable = BytecodeTable::new();

pub struct BytecodeTable {
    table: [Option<BytecodeInstruction>; 256],
}
impl BytecodeTable {
    const fn new() -> Self {
        let mut table = [None; 256];
        table[0x00] = Some(nop_instruction as BytecodeInstruction);

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
        if let Some(handler) = self.table[table_index] {
            handler(thread, heap, class_loader)
        } else {
            Err(JvmError::UnimplementedInstruction(instruction).bx())
        }
    }
}

fn nop_instruction(
    _thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> Result<(), Box<JvmError>> {
    Ok(())
}
