use std::{
    cell::RefCell,
    error::Error,
    fmt::{Debug, Display},
    num::NonZeroUsize,
    rc::Rc,
};

use crate::{
    class_cache::ClassCache, class_file::ClassFile, class_loader::ClassLoader,
    class_parser::ClassParserError, jvm_cache::JvmCache, jvm_heap::JvmHeap,
    native_method_resolver::NativeMethodResolver, v_table::VTable, verifier::VerifierError,
};

pub const STRING_CLASS_NAME: &str = "java/lang/String";
pub const CLASS_CLASS_NAME: &str = "java/lang/Class";
pub const OBJECT_CLASS_NAME: &str = "java/lang/Object";
pub const THROWABLE_CLASS_NAME: &str = "java/lang/Throwable";
pub const SYSTEM_CLASS_NAME: &str = "java/lang/System";
pub const FLOAT_CLASS_NAME: &str = "java/lang/Float";
pub const DOUBLE_CLASS_NAME: &str = "java/lang/Double";
pub const NULL_POINTER_EXCEPTION_NAME: &str = "java/lang/NullPointerException";
pub const NEGATIVE_ARRAY_SIZE_EXCEPTION_NAME: &str = "java/lang/NegativeArraySizeException";
pub const ARRAY_INDEX_OUT_OF_BOUNDS_EXCEPTION_NAME: &str =
    "java/lang/ArrayIndexOutOfBoundsException";
pub const ARRAY_STORE_EXCEPTION_NAME: &str = "java/lang/ArrayStoreException";
pub const ARITHMETIC_EXCEPTION_NAME: &str = "java/lang/ArithmeticException";
pub const ILLEGAL_ACCESS_ERROR_NAME: &str = "java/lang/IllegalAccessError";
pub const CLASS_CAST_EXCEPTION_NAME: &str = "java/lang/ClassCastException";
pub const THROWABLE_INTERFACE_NAME: &str = "java/lang/Throwable";
pub const ARRAY_CLASS_NAME: &str = "java/lang/reflect/Array";
pub const JVANKOM_PRINT_STEAM_CLASS_NAME: &str = "jvankomrt/JVankoMPrintStream";

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
impl From<char> for DescriptorType {
    fn from(value: char) -> Self {
        match value {
            'I' => DescriptorType::Integer,
            'J' => DescriptorType::Long,
            'F' => DescriptorType::Float,
            'D' => DescriptorType::Double,
            'B' => DescriptorType::Byte,
            'C' => DescriptorType::Character,
            'S' => DescriptorType::Short,
            'Z' => DescriptorType::Boolean,
            _ => DescriptorType::Reference,
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
    ExpectedThrowable,
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
    InvalidInterfaceMethodRefIndex(NonZeroUsize),
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
    ExpectedNativeMethod {
        method_name: String,
        method_descriptor: String,
    },
    ExpectedStaticMethod {
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
    ExpectedArray,
    ExpectedNonArrayObject,
    UnhandledException {
        reference: NonZeroUsize,
        class_name: String,
        fields: Vec<JvmValue>,
    },
    InvalidMultidimensionalPrimitiveArrayDimension,
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
            JvmError::InvalidInterfaceMethodRefIndex(index) => {
                format!("Invalid interface method ref index: '{index}'")
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
            JvmError::ExpectedNativeMethod {
                method_name,
                method_descriptor,
            } => {
                format!("Expected method to be native: {method_name}{method_descriptor}")
            }
            JvmError::ExpectedStaticMethod {
                method_name,
                method_descriptor,
            } => {
                format!("Expected method to be static: {method_name}{method_descriptor}")
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
            JvmError::ExpectedArray => "Expected array object".to_owned(),
            JvmError::ExpectedNonArrayObject => "Expected non-array object".to_owned(),
            JvmError::UnhandledException {
                reference,
                class_name,
                fields,
            } => {
                format!("Unhandled exception: {class_name} (adr. {reference}) fields:{fields:?}")
            }
            JvmError::InvalidMultidimensionalPrimitiveArrayDimension => {
                "Invalid multi-dimensional primitive array dimension".to_owned()
            }
            JvmError::ExpectedThrowable => "Expected class to be instance of Throwable".to_owned(),
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
            JvmValue::Double(_) => matches!(desciptor, DescriptorType::Double),
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
    ObjectArray(ObjectArray),
}

#[derive(Debug, Clone)]
pub struct ObjectArray {
    pub array: Vec<Option<NonZeroUsize>>,
    pub dimension: NonZeroUsize,
    pub object_array_type: ObjectArrayType,
}

#[derive(Debug, Clone)]
pub enum ObjectArrayType {
    Class(Rc<JvmClass>),
    Primitive(DescriptorType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameReturn {
    NotReturning,
    Returning,
    Exception,
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
    pub should_return: FrameReturn,
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
            .expect_utf8(method.descriptor_index);
        let is_void = descriptor.ends_with('V');

        Self {
            method_index,
            local_variables,
            operand_stack,
            program_counter: 0,
            bytecode_index,
            class,
            is_void,
            should_return: FrameReturn::NotReturning,
            return_value: None,
        }
    }

    pub fn set_exception(&mut self, exception_ref: NonZeroUsize) {
        self.return_value = Some(JvmValue::Reference(Some(exception_ref)));
        self.should_return = FrameReturn::Exception;
    }

    pub fn unset_exception(&mut self) {
        self.return_value = None;
        self.should_return = FrameReturn::NotReturning;
    }

    pub fn debug_print(&self) {
        let program_counter = self.program_counter;
        let bytecode = &self.class.class_file.methods[self.method_index]
            .get_bytecode(self.bytecode_index)
            .code;
        let bytecode_display = &bytecode[0..50.min(bytecode.len())];
        let stack = &self.operand_stack[0..20.min(self.operand_stack.len())];
        let locals = &self.local_variables[0..20.min(self.local_variables.len())];
        let method = self
            .class
            .class_file
            .constant_pool
            .expect_utf8(self.class.class_file.methods[self.method_index].name_index);
        let descriptor = self
            .class
            .class_file
            .constant_pool
            .expect_utf8(self.class.class_file.methods[self.method_index].descriptor_index);
        let class_name = self.class.class_file.get_class_name();

        println!(
            "method:{class_name}.{method}{descriptor}\ncode:\n{bytecode_display:?}\nprogram counter:\n{program_counter}\nstack:\n{stack:?}\nlocals:\n{locals:?}\n"
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

    pub fn top_frame(&mut self) -> &mut JvmStackFrame {
        self.stack.last_mut().expect("No frames found")
    }

    pub fn insert(&mut self, index: usize, frame: JvmStackFrame) {
        self.stack.insert(index, frame);
    }

    pub fn peek_at(&mut self, index: usize) -> &mut JvmStackFrame {
        &mut self.stack[index]
    }

    pub fn get_stack_frames(&self) -> &[JvmStackFrame] {
        &self.stack
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

    /// returns the class, method name and method descriptor based on method ref index,
    /// 'class' is the class holding the CP value
    pub fn read_method_ref(&self, method_ref: NonZeroUsize) -> JvmResult<(&str, &str, &str)> {
        if let Some(called_method) = self
            .class_file
            .constant_pool
            .get_class_methodname_descriptor(method_ref)
        {
            Ok(called_method)
        } else {
            Err(JvmError::InvalidMethodRefIndex(method_ref).bx())
        }
    }

    /// returns the class, method name and method descriptor based on interface method ref index,
    /// 'class' is the class holding the CP value
    pub fn read_interface_method_ref(
        &self,
        interface_method_ref: NonZeroUsize,
    ) -> JvmResult<(&str, &str, &str)> {
        if let Some(interface_info) = self
            .class_file
            .constant_pool
            .get_interface_method(interface_method_ref)
        {
            Ok(interface_info)
        } else {
            Err(JvmError::InvalidInterfaceMethodRefIndex(interface_method_ref).bx())
        }
    }

    pub fn is_sublcass_of(parent: &Rc<JvmClass>, child: &Rc<JvmClass>) -> bool {
        if Rc::as_ptr(parent) == Rc::as_ptr(child) {
            return true;
        }

        for interface in &child.state.borrow().interfaces {
            if Self::is_sublcass_of(parent, interface) {
                return true;
            }
        }

        if let Some(super_class) = &child.state.borrow().super_class {
            Self::is_sublcass_of(parent, super_class)
        } else {
            false
        }
    }
}

pub struct ClassState {
    pub is_initialised: bool,
    pub non_static_fields: Option<Vec<FieldInfo>>,
    pub static_fields: Option<Vec<StaticFieldInfo>>,
    pub default_object: Option<HeapObject>,
    pub super_class: Option<Rc<JvmClass>>,
    pub v_table: VTable,
    pub cache: ClassCache,
    pub interfaces: Vec<Rc<JvmClass>>,
}
impl Default for ClassState {
    fn default() -> Self {
        Self {
            is_initialised: false,
            non_static_fields: None,
            default_object: None,
            super_class: None,
            v_table: VTable::new(),
            static_fields: None,
            cache: ClassCache::new(),
            interfaces: vec![],
        }
    }
}
impl Debug for ClassState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClassState")
            .field("is_initialised", &self.is_initialised)
            .finish()
    }
}
