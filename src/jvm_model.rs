use std::{
    borrow::Cow, collections::HashMap, error::Error, fmt::Display, num::NonZeroUsize, rc::Rc,
};

use crate::{class_file::ClassFile, class_parser::ClassParserError};

#[derive(Debug, Clone)]
pub enum JvmError {
    ClassParserError(ClassParserError),
    ClassLoaderError(String),
    MethodNotFound {
        class_name: String,
        method_name: String,
    },
    UnimplementedInstruction(u8),
}
impl JvmError {
    pub fn bx(self) -> Box<Self> {
        Box::new(self)
    }
}
impl Display for JvmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let description = match self {
            JvmError::ClassParserError(class_parser_error) => {
                Cow::Owned(format!("{class_parser_error}"))
            }
            JvmError::ClassLoaderError(err) => Cow::Borrowed(err),
            JvmError::MethodNotFound {
                class_name,
                method_name,
            } => Cow::Owned(format!(
                "Method '{method_name}' not found for '{class_name}'"
            )),
            JvmError::UnimplementedInstruction(instruction) => Cow::Owned(format!(
                "Instruction with code '{}' not yet implemented",
                instruction
            )),
        };

        f.write_str(&description)
    }
}
impl Error for JvmError {}

#[derive(Debug, Clone)]
pub enum JvmValue {
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Reference(Option<NonZeroUsize>),
}

#[derive(Debug, Clone)]
pub struct HeapObject {
    pub class: Rc<ClassFile>,
    pub fields: Vec<JvmValue>,
}

#[derive(Debug, Clone)]
pub struct JvmHeap {
    heap: HashMap<NonZeroUsize, HeapObject>,
    reference_counter: usize,
}
impl JvmHeap {
    pub fn new() -> Self {
        Self {
            heap: HashMap::new(),
            reference_counter: 0,
        }
    }

    pub fn get(&mut self, reference: NonZeroUsize) -> Option<&mut HeapObject> {
        self.heap.get_mut(&reference)
    }

    /// returns the reference to the new object
    pub fn allocate(&mut self, object: HeapObject) -> NonZeroUsize {
        self.reference_counter += 1;
        let reference = NonZeroUsize::new(self.reference_counter).unwrap();
        self.heap.insert(reference, object);

        reference
    }
}

#[derive(Debug, Clone)]
pub struct JvmStackFrame {
    pub class: Rc<ClassFile>,
    pub method_index: usize,
    pub bytecode_index: usize,
    pub local_variables: Vec<JvmValue>,
    pub operand_stack: Vec<JvmValue>,
    pub program_counter: usize,
    pub is_void: bool,
    pub should_return: bool,
    pub return_value: Option<JvmValue>,
}
impl JvmStackFrame {
    pub fn new(
        class: Rc<ClassFile>,
        method_index: usize,
        bytecode_index: usize,
        params: Vec<JvmValue>,
    ) -> Self {
        let method = &class.methods[method_index];
        let bytecode = method.get_bytecode(bytecode_index);
        let mut operand_stack = Vec::with_capacity(bytecode.max_stack as usize);
        operand_stack.extend(params);
        let descriptor = class
            .constant_pool
            .get_utf8(method.descriptor_index)
            .expect("Expected descriptor value");
        let is_void = descriptor.ends_with('V');

        Self {
            method_index,
            local_variables: Vec::with_capacity(bytecode.max_locals as usize),
            operand_stack,
            program_counter: 0,
            bytecode_index,
            class,
            is_void,
            should_return: false,
            return_value: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct JvmThread {
    stack: Vec<JvmStackFrame>,
}
impl JvmThread {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn has_frames(&self) -> bool {
        !self.stack.is_empty()
    }

    pub fn push(&mut self, frame: JvmStackFrame) {
        self.stack.push(frame);
    }

    pub fn pop(&mut self) -> Option<JvmStackFrame> {
        self.stack.pop()
    }

    pub fn peek(&mut self) -> Option<&mut JvmStackFrame> {
        self.stack.last_mut()
    }
}
