use std::{cell::RefCell, error::Error, fmt::Display, num::NonZeroUsize, rc::Rc};

use crate::{
    class_file::ClassFile, class_loader::ClassLoader, class_parser::ClassParserError,
    field_access_cache::FieldAccessCache, method_call_cache::MethodCallCache,
    native_method_resolver::NativeMethodResolver, object_creation_cache::ObjectCreationCache,
    v_table::VTable, verifier::VerifierError,
};

pub type JvmResult<T> = Result<T, Box<JvmError>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DescriptorType {
    Integer,
    Long,
    Reference,
    Short,
    Character,
    Byte,
    Float,
    Double,
    Boolean,
}
impl DescriptorType {
    pub fn create_default_value(self) -> JvmValue {
        match self {
            DescriptorType::Integer
            | DescriptorType::Character
            | DescriptorType::Byte
            | DescriptorType::Boolean
            | DescriptorType::Short => JvmValue::Int(0),
            DescriptorType::Long => JvmValue::Long(0),
            DescriptorType::Reference => JvmValue::Reference(None),
            DescriptorType::Float => JvmValue::Float(0.0),
            DescriptorType::Double => JvmValue::Double(0.0),
        }
    }
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
    InvalidFieldRefIndex(NonZeroUsize),
    InvalidClassIndex(NonZeroUsize),
    VirtualMethodError {
        method_name: String,
        method_descriptor: String,
    },
    ExpectedNonNativeMethod {
        method_name: String,
        method_descriptor: String,
    },
    NativeMethodImplementationNotFound {
        class_name: String,
        method_name: String,
        method_descriptor: String,
    },
    FieldNotFound {
        class_name: String,
        field_name: String,
    },
    StaticFieldNotFound {
        class_name: String,
        field_name: String,
    },
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
            JvmError::InvalidFieldRefIndex(index) => {
                format!("Invalid field ref index: '{index}'")
            }
            JvmError::InvalidConstantPoolIndex => "Invalid constant pool index'".to_owned(),
            JvmError::InvalidClassIndex(index) => {
                format!("Invalid class index: '{index}'")
            }
            JvmError::VirtualMethodError {
                method_name,
                method_descriptor,
            } => {
                format!("Error calling virtual method: {method_name}{method_descriptor}")
            }
            JvmError::ExpectedNonNativeMethod {
                method_name,
                method_descriptor,
            } => {
                format!("Expected method to not be native: {method_name}{method_descriptor}")
            }
            JvmError::NativeMethodImplementationNotFound {
                class_name,
                method_name,
                method_descriptor,
            } => {
                format!(
                    "Native method implementation not found for: {class_name}.{method_name}{method_descriptor}"
                )
            }
            JvmError::FieldNotFound {
                class_name,
                field_name,
            } => {
                format!("Field not found: {class_name}.{field_name}")
            }
            JvmError::StaticFieldNotFound {
                class_name,
                field_name,
            } => {
                format!("Static field not found: {class_name}.{field_name}")
            }
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

    pub fn matches_type(self, desciptor: DescriptorType) -> bool {
        match self {
            JvmValue::Int(_) => matches!(
                desciptor,
                DescriptorType::Short
                    | DescriptorType::Boolean
                    | DescriptorType::Character
                    | DescriptorType::Byte
                    | DescriptorType::Integer
            ),
            JvmValue::Long(_) => matches!(desciptor, DescriptorType::Long),
            JvmValue::Float(_) => matches!(desciptor, DescriptorType::Float),
            JvmValue::Double(_) => matches!(desciptor, DescriptorType::Long),
            JvmValue::Reference(_) => matches!(desciptor, DescriptorType::Reference),
            JvmValue::Unusable => panic!("Attempting to get the type of an unusable slot"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum HeapObject {
    Object {
        class: Rc<JvmClass>,
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
    heap: Vec<Option<HeapObject>>,
    free_slots: Vec<usize>,
}
impl JvmHeap {
    pub fn new() -> Self {
        const INITIAL_ALLOCATION: usize = 2;
        Self {
            heap: vec![None; INITIAL_ALLOCATION],
            free_slots: (1..INITIAL_ALLOCATION).collect(),
        }
    }

    pub fn get(&mut self, reference: NonZeroUsize) -> &mut HeapObject {
        if let Some(obj) = &mut self.heap[reference.get()] {
            obj
        } else {
            panic!("Reference {} is invalid", reference);
        }
    }

    /// returns the reference to the new object
    pub fn allocate(&mut self, object: HeapObject) -> NonZeroUsize {
        if let Some(free_index) = self.free_slots.pop() {
            debug_assert!(self.heap[free_index].is_none());
            self.heap[free_index] = Some(object);
            NonZeroUsize::new(free_index).expect("Index should not be zero")
        } else {
            let new_index = self.heap.len();
            self.heap.push(Some(object));
            NonZeroUsize::new(new_index).expect("Index should not be zero")
        }
    }
}

#[derive(Debug, Clone)]
pub struct JvmStackFrame {
    pub class: Rc<JvmClass>,
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
        class: Rc<JvmClass>,
        method_index: usize,
        bytecode_index: usize,
        params: Vec<JvmValue>,
    ) -> Self {
        let method = &class.class_file.methods[method_index];
        let bytecode = method.get_bytecode(bytecode_index);
        let operand_stack = Vec::with_capacity(bytecode.max_stack as usize);
        let mut local_variables = vec![JvmValue::Unusable; bytecode.max_locals as usize];
        debug_assert!(params.len() <= bytecode.max_locals as usize);
        local_variables[..params.len()].copy_from_slice(&params[..]);
        let descriptor = class
            .class_file
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
        let bytecode = &self.class.class_file.methods[self.method_index]
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
pub struct JvmCache {
    pub method_call_cache: MethodCallCache,
}
impl JvmCache {
    pub fn new() -> Self {
        Self {
            method_call_cache: MethodCallCache::new(),
        }
    }
}

#[derive(Debug)]
pub struct JvmContext<'a> {
    pub class_loader: &'a mut ClassLoader,
    pub current_thread: &'a mut JvmThread,
    pub heap: &'a mut JvmHeap,
    pub cache: &'a mut JvmCache,
    pub native_method_resolver: &'a mut NativeMethodResolver,
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    /// class that originally declared the field, class file holds the Field
    pub class: Rc<JvmClass>,
    /// index into the original class file that has the Field object
    pub field_class_file_index: usize,
    pub descriptor_type: DescriptorType,
}

#[derive(Debug, Clone)]
pub struct StaticFieldInfo {
    pub name: String,
    pub descriptor_type: DescriptorType,
    /// index into the class file
    pub field_class_file_index: usize,
    pub value: JvmValue,
}

#[derive(Debug)]
pub struct JvmClass {
    pub class_file: ClassFile,
    pub state: RefCell<ClassState>,
}
impl JvmClass {
    pub fn new(class_file: ClassFile) -> Rc<Self> {
        Rc::new(Self {
            class_file,
            state: RefCell::new(ClassState::default()),
        })
    }
}

#[derive(Debug)]
pub struct ClassState {
    pub is_initialised: bool,
    pub non_static_fields: Option<Vec<FieldInfo>>,
    pub static_fields: Option<Vec<StaticFieldInfo>>,
    pub default_object: Option<HeapObject>,
    pub super_class: Option<Rc<JvmClass>>,
    /// cache that maps indexes from new to classes
    pub object_creation_cache: ObjectCreationCache,
    pub v_table: VTable,
    pub field_access_cache: FieldAccessCache,
}
impl Default for ClassState {
    fn default() -> Self {
        Self {
            is_initialised: false,
            non_static_fields: None,
            default_object: None,
            super_class: None,
            object_creation_cache: ObjectCreationCache::new(),
            v_table: VTable::new(),
            field_access_cache: FieldAccessCache::new(),
            static_fields: None,
        }
    }
}
