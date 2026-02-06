use std::{
    collections::HashMap,
    error::Error,
    fmt::{Display, format},
    num::NonZeroUsize,
    rc::Rc,
};

use crate::{
    class_file::ClassFile, class_loader::ClassLoader, class_parser::ClassParserError,
    method_call_cache::MethodCallCache, verifier::VerifierError,
};

pub type JvmResult<T> = Result<T, Box<JvmError>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParameterCallType {
    Integer,
    Long,
    Reference,
    Short,
    Character,
    Byte,
    Float,
    Double,
}

#[derive(Debug, Clone, Copy)]
pub enum JvmType {
    Int,
    Long,
    Float,
    Double,
    Reference,
}
impl JvmType {
    pub fn description(self) -> &'static str {
        match self {
            Self::Int => "Int",
            Self::Long => "Long",
            Self::Float => "Float",
            Self::Double => "Double",
            Self::Reference => "Reference",
        }
    }
}

#[derive(Debug, Clone)]
pub enum JvmError {
    ClassParserError {
        parsed_class: String,
        error: ClassParserError,
    },
    ClassLoaderError(String),
    MethodNotFound {
        class_name: String,
        method_name: String,
    },
    UnimplementedInstruction(u8),
    NoOperandFound,
    NoLocalVariableFound,
    ProgramCounterOutOfBounds {
        current_index: usize,
        bytecode_len: usize,
    },
    TypeError {
        expected: JvmType,
        found: JvmType,
    },
    MissingReturnValue,
    InvalidReference,
    InvalidArrayType(u8),
    ClassVerificationError {
        verified_class: String,
        error: VerifierError,
    },
    IncompatibleArrayType,
    InvalidMethodDescriptor(String),
    InvalidConstantPoolIndex,
    InvalidMethodRefIndex(NonZeroUsize),
}
impl JvmError {
    pub fn bx(self) -> Box<Self> {
        Box::new(self)
    }
}
impl Display for JvmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let description = match self {
            JvmError::ClassParserError {
                parsed_class,
                error,
            } => {
                format!("Parsing error:{error} for '{parsed_class}'")
            }
            JvmError::ClassLoaderError(err) => err.to_owned(),
            JvmError::MethodNotFound {
                class_name,
                method_name,
            } => format!("Method '{method_name}' not found for '{class_name}'"),
            JvmError::UnimplementedInstruction(instruction) => format!(
                "Instruction with code '{}' not yet implemented",
                instruction
            ),
            JvmError::NoOperandFound => "No operand found".to_owned(),
            JvmError::NoLocalVariableFound => "No local variable found".to_owned(),
            JvmError::TypeError { expected, found } => format!(
                "Type error: expected {} found {}",
                expected.description(),
                found.description()
            ),
            JvmError::ProgramCounterOutOfBounds {
                current_index,
                bytecode_len,
            } => format!(
                "Program counter is out of bounds, index is {}, bytecode length is {}",
                current_index, bytecode_len
            ),
            JvmError::MissingReturnValue => "Missing method return value".to_owned(),
            JvmError::ClassVerificationError {
                verified_class,
                error,
            } => format!("Verification error:{error} for '{verified_class}'"),
            JvmError::InvalidArrayType(array_type) => format!("Invalid array type '{array_type}'"),
            JvmError::InvalidReference => "Reference points to invalid memory".to_owned(),
            JvmError::IncompatibleArrayType => "Incompatible array type".to_owned(),
            JvmError::InvalidMethodDescriptor(desc) => {
                format!("Invalid method descriptor: '{desc}'")
            }
            JvmError::InvalidMethodRefIndex(index) => {
                format!("Invalid method ref index: '{index}'")
            }
            JvmError::InvalidConstantPoolIndex => "Invalid constant pool index'".to_owned(),
        };

        f.write_str(&description)
    }
}
impl Error for JvmError {}

#[derive(Debug, Clone, Copy)]
pub enum JvmValue {
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Reference(Option<NonZeroUsize>),
    /// Unusable value following longs and doubles
    Unusable,
}
impl JvmValue {
    pub fn get_type(self) -> JvmType {
        match self {
            Self::Int(_) => JvmType::Int,
            Self::Long(_) => JvmType::Long,
            Self::Float(_) => JvmType::Float,
            Self::Double(_) => JvmType::Double,
            Self::Reference(_) => JvmType::Reference,
            Self::Unusable => panic!("Attempting to get the type of an unusable slot"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum HeapObject {
    Object {
        class: Rc<ClassFile>,
        fields: Vec<JvmValue>,
    },
    IntArray(Vec<i32>),
    ByteArray(Vec<i8>),
    BooleanArray(Vec<bool>),
    CharacterArray(Vec<u16>),
    ShortArray(Vec<i16>),
    FloatArray(Vec<f32>),
    DoubleArray(Vec<f64>),
    LongArray(Vec<i64>),
    ObjectArray(Vec<Option<NonZeroUsize>>),
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
        let operand_stack = Vec::with_capacity(bytecode.max_stack as usize);
        let mut local_variables = vec![JvmValue::Unusable; bytecode.max_locals as usize];
        debug_assert!(params.len() <= bytecode.max_locals as usize);
        local_variables[..params.len()].copy_from_slice(&params[..]);
        let descriptor = class
            .constant_pool
            .get_utf8(method.descriptor_index)
            .expect("Expected descriptor value");
        let is_void = descriptor.ends_with('V');

        Self {
            method_index,
            local_variables,
            operand_stack,
            program_counter: 0,
            bytecode_index,
            class,
            is_void,
            should_return: false,
            return_value: None,
        }
    }

    pub fn debug_print(&self) {
        let program_counter = self.program_counter;
        let bytecode = &self.class.methods[self.method_index]
            .get_bytecode(self.bytecode_index)
            .code;
        let stack = &self.operand_stack;
        let locals = &self.local_variables;

        println!(
            "code:\n{bytecode:?}\nprogram counter:\n{program_counter}\nstack:\n{stack:?}\nlocals:\n{locals:?}"
        );
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

#[derive(Debug)]
pub struct JvmContext<'a> {
    pub class_loader: &'a mut ClassLoader,
    pub current_thread: &'a mut JvmThread,
    pub heap: &'a mut JvmHeap,
    pub method_call_cache: &'a mut MethodCallCache,
}
