use std::{num::NonZeroUsize, rc::Rc};

use crate::{
    bytecode::BYTECODE_TABLE,
    class_file::MethodAccessFlags,
    class_loader::ClassLoader,
    exceptions::handle_exception,
    field_initialisation::determine_static_fields,
    jvm_cache::JvmCache,
    jvm_heap::JvmHeap,
    jvm_model::{
        FrameReturn, HeapObject, JvmClass, JvmContext, JvmError, JvmResult, JvmStackFrame,
        JvmThread, JvmValue, ObjectArray, ObjectArrayType, STRING_CLASS_NAME,
    },
    native_method_resolver::NativeMethodResolver,
};

pub struct Jvm {
    class_loader: ClassLoader,
    threads: Vec<JvmThread>,
    heap: JvmHeap,
    cache: JvmCache,
    native_method_resolver: NativeMethodResolver,
}
impl Jvm {
    pub fn new(class_loader: ClassLoader, heap: JvmHeap) -> Self {
        Self {
            class_loader,
            threads: vec![],
            heap,
            cache: JvmCache::new(),
            native_method_resolver: NativeMethodResolver::new(),
        }
    }

    /// calls <clinit> method on class and super classes if it hasn't been called
    pub fn initialise_class(
        thread: &mut JvmThread,
        loaded_class: &Rc<JvmClass>,
        class_loader: &mut ClassLoader,
        class_name: &str,
    ) -> JvmResult<()> {
        const INITIALISE_CLASS_METHOD: &str = "<clinit>";
        const INITIALISE_CLASS_DESCIPTOR: &str = "()V";
        if loaded_class.state.borrow().is_initialised {
            return Ok(());
        }
        loaded_class.state.borrow_mut().is_initialised = true;

        let (method_index, bytecode_index) = if let Some((m_index, b_index)) = loaded_class
            .class_file
            .get_method_and_bytecode_index(INITIALISE_CLASS_METHOD, INITIALISE_CLASS_DESCIPTOR)
        {
            (m_index, b_index)
        } else {
            return Self::initialise_class_state_and_recurse_load(
                thread,
                loaded_class,
                class_loader,
                class_name,
            );
        };

        if let Some(bytecode_index) = bytecode_index {
            thread.push(JvmStackFrame::new(
                loaded_class.clone(),
                method_index,
                bytecode_index,
                vec![],
            ));
        } else {
            return Err(JvmError::ExpectedNonNativeMethod {
                method_name: INITIALISE_CLASS_METHOD.to_owned(),
                method_descriptor: INITIALISE_CLASS_DESCIPTOR.to_owned(),
            }
            .bx());
        }

        Self::initialise_class_state_and_recurse_load(
            thread,
            loaded_class,
            class_loader,
            class_name,
        )
    }

    fn initialise_class_state_and_recurse_load(
        thread: &mut JvmThread,
        loaded_class: &Rc<JvmClass>,
        class_loader: &mut ClassLoader,
        class_name: &str,
    ) -> JvmResult<()> {
        let mut state = loaded_class.state.borrow_mut();
        if state.static_fields.is_none() {
            state.static_fields = Some(determine_static_fields(&loaded_class.class_file));
        }
        drop(state);
        Self::initialise_class_interfaces(loaded_class, class_loader)?;

        // recusrively load super classes
        if let Some(super_class_name) = loaded_class.class_file.get_super_class_name() {
            let super_class = class_loader.get(super_class_name)?;
            loaded_class.state.borrow_mut().super_class = Some(super_class.clone());
            return Self::initialise_class(thread, &super_class, class_loader, class_name);
        }

        Ok(())
    }

    fn initialise_class_interfaces(
        loaded_class: &Rc<JvmClass>,
        class_loader: &mut ClassLoader,
    ) -> JvmResult<()> {
        if !loaded_class.state.borrow().interfaces.is_empty() {
            return Ok(());
        }

        for interface_index in &loaded_class.class_file.interfaces {
            let interface_name = loaded_class
                .class_file
                .constant_pool
                .get_class_name(*interface_index)
                .expect("Interface index is invalid");
            let interface = class_loader.get(interface_name)?;
            Self::initialise_class_interfaces(&interface, class_loader)?;
            loaded_class.state.borrow_mut().interfaces.push(interface);
        }

        Ok(())
    }

    pub fn get_threads(self) -> Vec<JvmThread> {
        self.threads
    }

    pub fn run_main(&mut self, class_name: String, args: Vec<String>) -> JvmResult<()> {
        const LIBRARY_CLASS_NAME: &str = "jvankomrt/JVMInit";
        const LIBRARY_METHOD_NAME: &str = "init";
        const LIBRARY_METHOD_DESC: &str = "()V";
        const MAIN_METHOD_NAME: &str = "main";
        const MAIN_METHOD_DESC: &str = "([Ljava/lang/String;)V";
        self.run_method(
            LIBRARY_CLASS_NAME.to_owned(),
            LIBRARY_METHOD_NAME.to_owned(),
            LIBRARY_METHOD_DESC.to_owned(),
            vec![],
        )?;
        self.threads.clear();
        let params = self.prepare_string_args(args)?;
        self.run_method(
            class_name,
            MAIN_METHOD_NAME.to_owned(),
            MAIN_METHOD_DESC.to_owned(),
            params,
        )?;

        Ok(())
    }

    pub fn run_with_init(
        &mut self,
        class_name: String,
        method_name: String,
        method_descriptor: String,
        params: Vec<JvmValue>,
    ) -> JvmResult<Option<JvmValue>> {
        const LIBRARY_CLASS_NAME: &str = "jvankomrt/JVMInit";
        const LIBRARY_METHOD_NAME: &str = "init";
        const LIBRARY_METHOD_DESC: &str = "()V";
        self.run_method(
            LIBRARY_CLASS_NAME.to_owned(),
            LIBRARY_METHOD_NAME.to_owned(),
            LIBRARY_METHOD_DESC.to_owned(),
            vec![],
        )?;
        self.threads.clear();
        self.run_method(class_name, method_name, method_descriptor, params)
    }

    pub fn run_method(
        &mut self,
        class_name: String,
        method_name: String,
        method_descriptor: String,
        params: Vec<JvmValue>,
    ) -> JvmResult<Option<JvmValue>> {
        let loaded_class = self.class_loader.get(&class_name)?;

        let (method_index, bytecode_index) = if let Some(index) = loaded_class
            .class_file
            .get_method_and_bytecode_index(&method_name, &method_descriptor)
        {
            index
        } else {
            return Err(JvmError::MethodNotFound {
                class_name,
                method_name,
            }
            .bx());
        };

        if !loaded_class.class_file.methods[method_index]
            .access_flags
            .check_flag(MethodAccessFlags::STATIC_FLAG)
        {
            return Err(JvmError::ExpectedStaticMethod {
                method_name,
                method_descriptor,
            }
            .bx());
        }

        if bytecode_index.is_none() {
            return Err(JvmError::ExpectedNonNativeMethod {
                method_name,
                method_descriptor,
            }
            .bx());
        }

        let mut thread = JvmThread::new();
        let stack_frame = JvmStackFrame::new(
            loaded_class.clone(),
            method_index,
            bytecode_index.unwrap(),
            params,
        );
        thread.push(stack_frame);
        Self::initialise_class(
            &mut thread,
            &loaded_class,
            &mut self.class_loader,
            &class_name,
        )?;
        self.threads.push(thread);

        self.run_thread()
    }

    fn run_thread(&mut self) -> JvmResult<Option<JvmValue>> {
        assert!(!self.threads.is_empty());

        let current_thread = &mut self.threads[0];
        while let Some(frame) = current_thread.peek() {
            let instruction = {
                let method = &frame.class.class_file.methods[frame.method_index];
                let bytecode = method.get_bytecode(frame.bytecode_index);
                if frame.should_return == FrameReturn::NotReturning {
                } else if frame.should_return == FrameReturn::Returning {
                    if frame.is_void {
                        debug_assert!(frame.return_value.is_none());
                        current_thread.pop();
                    } else {
                        let old_frame = current_thread.pop().unwrap();
                        let return_value = if let Some(value) = old_frame.return_value {
                            value
                        } else {
                            return Err(JvmError::MissingReturnValue.bx());
                        };

                        if let Some(top_frame) = current_thread.peek() {
                            top_frame.operand_stack.push(return_value);
                        } else {
                            return Ok(Some(return_value));
                        }
                    };

                    continue;
                } else {
                    handle_exception(current_thread, &mut self.heap, &mut self.class_loader)?;
                    continue;
                }
                debug_assert!(frame.program_counter < bytecode.code.len());
                // DEBUG
                #[cfg(debug_assertions)]
                frame.debug_print();

                let instruction = bytecode.code[frame.program_counter];
                frame.program_counter += 1;
                instruction
            };

            BYTECODE_TABLE.execute_instruction(
                instruction,
                JvmContext {
                    current_thread,
                    heap: &mut self.heap,
                    class_loader: &mut self.class_loader,
                    cache: &mut self.cache,
                    native_method_resolver: &mut self.native_method_resolver,
                },
            )?;

            // single threaded gc
            if self.heap.should_gc {
                self.heap.perform_gc(
                    &[current_thread],
                    self.cache.string_pool.get_string_references(),
                    self.class_loader.get_all_loaded_classes(),
                );
            }
        }

        Ok(None)
    }

    /// returns (used slotes, total allocated slots)
    pub fn get_cache_storage_efficieny(&self) -> (usize, usize) {
        self.class_loader
            .get_all_loaded_classes()
            .fold((0, 0), |(used, total), c| {
                let (u, t) = c.state.borrow().cache.get_storage_efficiency();
                (used + u, total + t)
            })
    }

    fn prepare_string_args(&mut self, args: Vec<String>) -> JvmResult<Vec<JvmValue>> {
        let mut strings = vec![];
        let string_class = self.class_loader.get(STRING_CLASS_NAME)?;
        for arg in args {
            let mut obj = string_class
                .state
                .borrow()
                .default_object
                .clone()
                .expect("String needs to be initialised");
            self.cache
                .string_pool
                .initialise_string_fields(&arg, &mut obj, &mut self.heap);
            let string_ref = self.heap.allocate(obj);
            strings.push(Some(string_ref));
        }

        let array = HeapObject::ObjectArray(ObjectArray {
            array: strings,
            dimension: NonZeroUsize::new(1).unwrap(),
            object_array_type: ObjectArrayType::Class(string_class),
        });
        let array_ref = self.heap.allocate(array);

        Ok(vec![JvmValue::Reference(Some(array_ref))])
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        class_loader::ClassSource,
        jvm_model::{
            ARITHMETIC_EXCEPTION_NAME, ARRAY_INDEX_OUT_OF_BOUNDS_EXCEPTION_NAME, HeapObject,
            NEGATIVE_ARRAY_SIZE_EXCEPTION_NAME, NULL_POINTER_EXCEPTION_NAME, OBJECT_CLASS_NAME,
            STRING_CLASS_NAME,
        },
        native_method_resolver::PRINT_LOG,
    };
    use sequential_test::sequential;

    use super::*;

    #[test]
    fn test_sum() {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run_method(
                "TestSimple".to_owned(),
                "sum".to_owned(),
                "(II)I".to_owned(),
                vec![JvmValue::Int(9), JvmValue::Int(10)],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(value) => assert_eq!(19, value),
            _ => panic!("expected int"),
        }
    }

    #[test]
    fn test_int_array_creation_and_indexing() {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run_method(
                "TestSimple".to_owned(),
                "arrayTest".to_owned(),
                "(III)I".to_owned(),
                vec![JvmValue::Int(100), JvmValue::Int(0), JvmValue::Int(3)],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(value) => assert_eq!(100, value),
            _ => panic!("expected int"),
        }
    }

    #[test]
    #[should_panic]
    fn test_int_array_invalid_index() {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let _result = jvm
            .run_method(
                "TestSimple".to_owned(),
                "arrayTest".to_owned(),
                "(III)I".to_owned(),
                vec![JvmValue::Int(100), JvmValue::Int(4), JvmValue::Int(3)],
            )
            .unwrap()
            .unwrap();
    }

    #[test]
    fn test_constants() {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run_method(
                "TestSimple".to_owned(),
                "constants".to_owned(),
                "(I)I".to_owned(),
                vec![JvmValue::Int(100)],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(value) => assert_eq!(102, value),
            _ => panic!("expected int"),
        }
    }

    #[test]
    fn test_single_jar() {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/simpleJar.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "TestSimple".to_owned(),
                "constants".to_owned(),
                "(I)I".to_owned(),
                vec![JvmValue::Int(100)],
            )
            .unwrap()
            .unwrap();
        match result {
            JvmValue::Int(x) => assert_eq!(102, x),
            _ => panic!("Expected int result"),
        }
    }

    #[test]
    fn test_single_class_static_method_calls() {
        test_single_class_static_method_calls_helper(100, 1000, 104);
        test_single_class_static_method_calls_helper(1000, 100, 1004);
    }

    #[test]
    fn test_method_caching() {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run_method(
                "TestStaticMethodCallCache".to_owned(),
                "mainCall".to_owned(),
                "(II)I".to_owned(),
                vec![JvmValue::Int(1000), JvmValue::Int(100)],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(value) => assert_eq!(2100, value),
            _ => panic!("expected int"),
        }

        assert_eq!(2, jvm.class_loader.get_total_cache_hits());
    }

    #[test]
    fn test_parameter_overload() {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run_method(
                "ParameterOverloadTest".to_owned(),
                "mainCall".to_owned(),
                "(II)I".to_owned(),
                vec![JvmValue::Int(3), JvmValue::Int(100)],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(value) => assert_eq!(106, value),
            _ => panic!("expected int"),
        }
    }

    #[test]
    fn test_integer_math() {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run_method(
                "IntegerMathTest".to_owned(),
                "mainCall".to_owned(),
                "(II)[I".to_owned(),
                vec![JvmValue::Int(8), JvmValue::Int(3)],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Reference(Some(value)) => match jvm.heap.get(value) {
                crate::jvm_model::HeapObject::IntArray(items) => {
                    assert_eq!(6, items.len());
                    assert_eq!(11, items[0]);
                    assert_eq!(-8, items[1]);
                    assert_eq!(24, items[2]);
                    assert_eq!(2, items[3]);
                    assert_eq!(5, items[4]);
                    assert_eq!(2, items[5]);
                }
                _ => panic!("expected int array"),
            },
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn test_integer_math_divide_by_zero() {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm.run_method(
            "IntegerMathTest".to_owned(),
            "mainCall".to_owned(),
            "(II)[I".to_owned(),
            vec![JvmValue::Int(8), JvmValue::Int(0)],
        );

        match result {
            Err(err) => match *err {
                JvmError::UnhandledException {
                    reference: _,
                    class_name,
                    fields: _,
                } => assert_eq!(ARITHMETIC_EXCEPTION_NAME, class_name),
                _ => panic!("expected UnhandledException"),
            },
            _ => panic!("expected exception"),
        }
    }

    #[test]
    fn test_cross_class_call() {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/CrossCallTest.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "CrossCall1Test".to_owned(),
                "callOtherClass".to_owned(),
                "(I)I".to_owned(),
                vec![JvmValue::Int(11)],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(int) => {
                assert_eq!(44, int);
            }
            _ => panic!("expected int"),
        }

        assert_eq!(2, jvm.class_loader.get_total_cache_hits());
        assert_eq!(3, jvm.class_loader.get_loaded_count());
    }

    #[test]
    fn test_virtual_call_self() {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/VirtualCallTest.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "VirtualCall1Test".to_owned(),
                "mainCallSelf".to_owned(),
                "(I)[I".to_owned(),
                vec![JvmValue::Int(5)],
            )
            .unwrap()
            .unwrap();

        let array_ref = match result {
            JvmValue::Reference(Some(r)) => r,
            _ => panic!("expected valid reference"),
        };
        let array = match jvm.heap.get(array_ref) {
            HeapObject::IntArray(items) => items,
            _ => panic!("Expected array"),
        };

        assert_eq!(2, array.len());
        assert_eq!(6, array[0]);
        assert_eq!(7, array[1]);
        assert_eq!(3, jvm.class_loader.get_loaded_count());
        assert_eq!(1, jvm.class_loader.get_total_cache_hits());
    }

    #[test]
    fn test_virtual_call_other() {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/VirtualCallTest.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "VirtualCall1Test".to_owned(),
                "mainCallOther".to_owned(),
                "(I)[I".to_owned(),
                vec![JvmValue::Int(5)],
            )
            .unwrap()
            .unwrap();

        let array_ref = match result {
            JvmValue::Reference(Some(r)) => r,
            _ => panic!("expected valid reference"),
        };
        let array = match jvm.heap.get(array_ref) {
            HeapObject::IntArray(items) => items,
            _ => panic!("Expected array"),
        };

        assert_eq!(2, array.len());
        assert_eq!(105, array[0]);
        assert_eq!(205, array[1]);
        assert_eq!(4, jvm.class_loader.get_loaded_count());
        assert_eq!(2, jvm.class_loader.get_total_cache_hits());
    }

    #[test]
    fn test_virtual_call_abstract() {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/VirtualCallTest.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "VirtualCall1Test".to_owned(),
                "mainCallAbstract".to_owned(),
                "(I)[I".to_owned(),
                vec![JvmValue::Int(5)],
            )
            .unwrap()
            .unwrap();

        let array_ref = match result {
            JvmValue::Reference(Some(r)) => r,
            _ => panic!("expected valid reference"),
        };
        let array = match jvm.heap.get(array_ref) {
            HeapObject::IntArray(items) => items,
            _ => panic!("Expected array"),
        };

        assert_eq!(2, array.len());
        assert_eq!(5000, array[0]);
        assert_eq!(5000000, array[1]);
        assert_eq!(4, jvm.class_loader.get_loaded_count());
        assert_eq!(2, jvm.class_loader.get_total_cache_hits());
    }

    #[test]
    fn test_virtual_call_other_with_private() {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/VirtualCallTest.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "VirtualCall1Test".to_owned(),
                "mainCallOtherWithPrivate".to_owned(),
                "(I)[I".to_owned(),
                vec![JvmValue::Int(5)],
            )
            .unwrap()
            .unwrap();

        let array_ref = match result {
            JvmValue::Reference(Some(r)) => r,
            _ => panic!("expected valid reference"),
        };
        let array = match jvm.heap.get(array_ref) {
            HeapObject::IntArray(items) => items,
            _ => panic!("Expected array"),
        };

        assert_eq!(2, array.len());
        assert_eq!(12, array[0]);
        assert_eq!(33, array[1]);
        assert_eq!(4, jvm.class_loader.get_loaded_count());
        assert_eq!(4, jvm.class_loader.get_total_cache_hits());
    }

    #[test]
    fn test_non_static_fields() {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/NonStaticFieldTest.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "NonStaticFieldTest1".to_owned(),
                "mainCall".to_owned(),
                "(I)[I".to_owned(),
                vec![JvmValue::Int(100)],
            )
            .unwrap()
            .unwrap();

        let array_ref = match result {
            JvmValue::Reference(Some(r)) => r,
            _ => panic!("expected valid reference"),
        };
        let array = match jvm.heap.get(array_ref) {
            HeapObject::IntArray(items) => items,
            _ => panic!("Expected array"),
        };

        assert_eq!(5, array.len());
        assert_eq!(2, array[0]);
        assert_eq!(100, array[1]);
        assert_eq!(5, array[2]);
        assert_eq!(6, array[3]);
        assert_eq!(7, array[4]);
        assert_eq!(3, jvm.class_loader.get_loaded_count());
        assert_eq!(10, jvm.class_loader.get_total_cache_hits());
    }

    #[test]
    fn test_chain_field_inheritance() {
        test_chain_field_inheritance_helper("mainCall");
        test_chain_field_inheritance_helper("testCache");
    }

    fn test_chain_field_inheritance_helper(method: impl Into<String>) {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/ChainFieldInheritanceTest.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "ChainFieldInheritanceTest".to_owned(),
                method.into(),
                "()[I".to_owned(),
                vec![],
            )
            .unwrap()
            .unwrap();

        let array_ref = match result {
            JvmValue::Reference(Some(r)) => r,
            _ => panic!("expected valid reference"),
        };
        let array = match jvm.heap.get(array_ref) {
            HeapObject::IntArray(items) => items,
            _ => panic!("Expected array"),
        };

        assert_eq!(4, array.len());
        assert_eq!(1, array[0]);
        assert_eq!(2, array[1]);
        assert_eq!(3, array[2]);
        assert_eq!(4, array[3]);
        assert_eq!(6, jvm.class_loader.get_loaded_count());
    }

    #[test]
    fn test_mixed_field_access() {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/MixedFieldAccessTest.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "MixedFieldAccessTest".to_owned(),
                "runTest".to_owned(),
                "()[J".to_owned(),
                vec![],
            )
            .unwrap()
            .unwrap();

        let array_ref = match result {
            JvmValue::Reference(Some(r)) => r,
            _ => panic!("expected valid reference"),
        };
        let array = match jvm.heap.get(array_ref) {
            HeapObject::LongArray(items) => items,
            _ => panic!("Expected array"),
        };
        let expected_values = vec![
            100, 110, 80, 90, 70, 10, 10, 10, 20, 40, 50, 30, 60, 1234, 5678, 999, 8888,
        ];
        assert_eq!(expected_values, *array);
        assert_eq!(15, jvm.class_loader.get_total_cache_hits());
    }

    #[test]
    fn test_field_shadowing() {
        test_field_shadowing_helper("mainCall");
        test_field_shadowing_helper("testCache");
    }

    #[test]
    fn test_static_field_self() {
        test_static_field_helper(
            "testSelf",
            vec![JvmValue::Int(10), JvmValue::Int(100)],
            vec![20, 100],
            3,
        );
    }

    #[test]
    fn test_static_field_self_cache() {
        test_static_field_helper(
            "testSelfCache",
            vec![JvmValue::Int(10), JvmValue::Int(100)],
            vec![30, 200],
            3,
        );
    }

    #[test]
    fn test_static_field_parent() {
        test_static_field_helper(
            "testParent",
            vec![JvmValue::Int(1000), JvmValue::Int(2000)],
            vec![9000, 10, 3000, 3000],
            3,
        );
    }

    #[test]
    fn test_static_field_parent_cache() {
        test_static_field_helper(
            "testParentCache",
            vec![JvmValue::Int(1000), JvmValue::Int(2000)],
            vec![9000, 10, 6000, 6000],
            3,
        );
    }

    #[test]
    fn test_static_field_other() {
        test_static_field_helper(
            "testOther",
            vec![JvmValue::Int(1000), JvmValue::Int(2000)],
            vec![2000, 4000, 6000],
            6,
        );
    }

    #[test]
    fn test_static_field_other_cache() {
        test_static_field_helper(
            "testOtherCache",
            vec![JvmValue::Int(10), JvmValue::Int(20)],
            vec![1020, 2040, 3060],
            6,
        );
    }

    #[test]
    fn test_gc_max() {
        test_gc_helper(1, 12, false);
        test_gc_helper(1, 6, true);
    }

    #[test]
    fn test_gc_once() {
        test_gc_helper(10, 15, true);
    }

    #[test]
    fn test_gc_no_gc() {
        test_gc_helper(1000, 13, false);
        test_gc_helper(1000, 16, true);
    }

    #[test]
    fn test_string_char_at() {
        for index in 0..("Hello".len()) {
            test_string_char_at_helper(index);
        }
    }

    #[test]
    fn test_string_concat() {
        for index in 0..("_Hello_".len()) {
            test_string_concat_helper('a' as i32, 'b' as i32, index);
        }
    }

    #[test]
    fn test_string_string_builder() {
        test_string_string_builder_helper(0, 'a' as i32);
        test_string_string_builder_helper(1, 'b' as i32);
        test_string_string_builder_helper(2, 'c' as i32);
    }

    #[test]
    fn test_string_substring() {
        test_string_substring_helper(0, 0, 'H' as i32);
        test_string_substring_helper(1, 0, 'e' as i32);
        test_string_substring_helper(1, 1, 'l' as i32);
        test_string_substring_helper(0, 3, 'l' as i32);
        test_string_substring_helper(2, 0, 'l' as i32);
        test_string_substring_helper(2, 2, 'o' as i32);
        test_string_substring_helper(4, 0, 'o' as i32);
    }

    fn test_string_substring_helper(start: i32, index: i32, expected: i32) {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run_method(
                "TestString".to_owned(),
                "subStr".to_owned(),
                "(II)I".to_owned(),
                vec![JvmValue::Int(start), JvmValue::Int(index)],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(ascii) => {
                assert_eq!(expected, ascii)
            }
            _ => panic!("expected int"),
        }
    }

    fn test_string_string_builder_helper(index: usize, expected: i32) {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run_method(
                "TestString".to_owned(),
                "testSB".to_owned(),
                "(I)I".to_owned(),
                vec![JvmValue::Int(index as i32)],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(ascii) => {
                assert_eq!(expected, ascii)
            }
            _ => panic!("expected int"),
        }
        let loaded_classes: Vec<_> = jvm
            .class_loader
            .get_all_loaded_classes()
            .map(|c| c.class_file.get_class_name())
            .collect();

        let expected_loaded_classes = [
            "java/lang/String$CaseInsensitiveComparator",
            OBJECT_CLASS_NAME,
            STRING_CLASS_NAME,
            "TestString",
            "java/io/ObjectStreamField",
        ];
        for expected in expected_loaded_classes {
            assert!(loaded_classes.contains(&expected));
        }
    }

    fn test_string_concat_helper(char_a: i32, char_b: i32, index: usize) {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run_method(
                "TestString".to_owned(),
                "concat".to_owned(),
                "(CCI)I".to_owned(),
                vec![
                    JvmValue::Int(char_a),
                    JvmValue::Int(char_b),
                    JvmValue::Int(index as i32),
                ],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(ascii) => {
                let char = format!("{}Hello{}", char_a as u8 as char, char_b as u8 as char)
                    .chars()
                    .collect::<Vec<_>>();
                assert_eq!(char[index] as u16, ascii as u16)
            }
            _ => panic!("expected int"),
        }
        let loaded_classes: Vec<_> = jvm
            .class_loader
            .get_all_loaded_classes()
            .map(|c| c.class_file.get_class_name())
            .collect();

        let expected_loaded_classes = [
            "java/lang/String$CaseInsensitiveComparator",
            OBJECT_CLASS_NAME,
            STRING_CLASS_NAME,
            "TestString",
            "java/io/ObjectStreamField",
        ];
        for expected in expected_loaded_classes {
            assert!(loaded_classes.contains(&expected));
        }
    }

    fn test_string_char_at_helper(index: usize) {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run_method(
                "TestString".to_owned(),
                "main".to_owned(),
                "(I)I".to_owned(),
                vec![JvmValue::Int(index as i32)],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(ascii) => {
                let char = "Hello".chars().collect::<Vec<_>>();
                assert_eq!(char[index] as u16, ascii as u16)
            }
            _ => panic!("expected int"),
        }
        let loaded_classes: Vec<_> = jvm
            .class_loader
            .get_all_loaded_classes()
            .map(|c| c.class_file.get_class_name())
            .collect();

        assert_eq!(9, jvm.class_loader.get_loaded_count());
        let expected_loaded_classes = [
            "java/lang/String$CaseInsensitiveComparator",
            OBJECT_CLASS_NAME,
            STRING_CLASS_NAME,
            "TestString",
            "java/io/ObjectStreamField",
            "java/io/Serializable",
            "java/lang/Comparable",
        ];
        for expected in expected_loaded_classes {
            assert!(loaded_classes.contains(&expected));
        }
    }

    #[test]
    fn test_comparisons_all_different() {
        test_comparisons_helper(0, 1, 2, &[67, 0, 67, 0, 0, 67, 67, 0]);
    }

    #[test]
    fn test_comparisons_all_different_non_zero() {
        test_comparisons_helper(1, 2, 3, &[0, 0, 67, 0, 0, 67, 67, 0]);
    }

    #[test]
    fn test_comparisons_all_zeroes() {
        test_comparisons_helper(0, 0, 0, &[67, 67, 0, 0, 67, 0, 67, 0]);
    }

    #[test]
    fn test_comparisons_first_equals_third_different() {
        test_comparisons_helper(3, 3, 2, &[0, 67, 0, 0, 67, 0, 67, 67]);
    }

    #[test]
    fn test_comparisons_loop_10() {
        test_comparisons_loop_helper(10);
    }

    #[test]
    fn test_comparisons_loop_2() {
        test_comparisons_loop_helper(2);
    }

    #[test]
    fn test_comparisons_loop_1() {
        test_comparisons_loop_helper(1);
    }

    #[test]
    fn test_comparisons_loop_0() {
        test_comparisons_loop_helper(0);
    }

    #[test]
    fn test_array_length_100() {
        test_array_length_helper(100);
    }

    #[test]
    fn test_array_length_1() {
        test_array_length_helper(1);
    }

    #[test]
    fn test_array_length_0() {
        test_array_length_helper(0);
    }

    #[test]
    fn test_exceptions_throw_or_catch() {
        test_exceptions_handled("throwOrCatch", 0, 42);
    }

    #[test]
    fn test_exceptions_throw_or_catch_unhandled() {
        test_exceptions_unhandled(
            "throwOrCatch",
            1,
            "ExceptionsTest$ExceptionsTestAnotherException",
        );
    }

    #[test]
    fn test_exceptions_call_and_throw() {
        test_exceptions_handled("callAndThrow", 2, 42);
    }

    #[test]
    fn test_exceptions_call_and_throw_unhandled() {
        test_exceptions_unhandled(
            "callAndThrow",
            1,
            "ExceptionsTest$ExceptionsTestAnotherException",
        );
    }

    #[test]
    fn test_exceptions_throw_in_this_method_0() {
        test_exceptions_handled("throwInThisMethod", 0, 99);
    }

    #[test]
    fn test_exceptions_throw_in_this_method_23() {
        test_exceptions_handled("throwInThisMethod", 23, 69);
    }

    #[test]
    fn test_exceptions_throw_in_called_method_2() {
        test_exceptions_handled("throwInCalledMethod", 2, 42);
    }

    #[test]
    fn test_exceptions_throw_in_called_method_3() {
        test_exceptions_handled("throwInCalledMethod", 3, 67);
    }

    #[test]
    fn test_null_pointer_exception_handled() {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run_method(
                "NullPointerExceptionTest".to_owned(),
                "catchNull".to_owned(),
                "()I".to_owned(),
                vec![],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(int) => assert_eq!(67, int),
            _ => panic!("expected int"),
        }
    }

    #[test]
    fn test_null_pointer_exception_unhandled() {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm.run_method(
            "NullPointerExceptionTest".to_owned(),
            "getNullArray".to_owned(),
            "()I".to_owned(),
            vec![],
        );

        match result {
            Err(err) => match *err {
                JvmError::UnhandledException {
                    reference: _,
                    class_name,
                    fields: _,
                } => assert_eq!(NULL_POINTER_EXCEPTION_NAME, class_name),
                _ => panic!("expected unhandled exception"),
            },
            _ => panic!("expected error"),
        }
    }

    #[test]
    fn test_array_exceptions_handled_out_of_bounds_left() {
        test_array_exceptions_handled_helper(5, -1, -1);
    }

    #[test]
    fn test_array_exceptions_handled_out_of_bounds_right() {
        test_array_exceptions_handled_helper(5, 5, -1);
    }

    #[test]
    fn test_array_exceptions_handled_negative_size() {
        test_array_exceptions_handled_helper(-5, 10, -2);
    }

    #[test]
    fn test_array_exceptions_unhandled_out_of_bounds_left() {
        test_array_exceptions_unhandled_helper(5, -1, ARRAY_INDEX_OUT_OF_BOUNDS_EXCEPTION_NAME);
    }

    #[test]
    fn test_array_exceptions_unhandled_out_of_bounds_right() {
        test_array_exceptions_unhandled_helper(5, 5, ARRAY_INDEX_OUT_OF_BOUNDS_EXCEPTION_NAME);
    }

    #[test]
    fn test_array_exceptions_unhandled_negative_size() {
        test_array_exceptions_unhandled_helper(-5, 10, NEGATIVE_ARRAY_SIZE_EXCEPTION_NAME);
    }

    #[test]
    fn test_instanceof() {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes/".to_owned())]);
        let result = jvm
            .run_method(
                "InstanceofTest".to_owned(),
                "test".to_owned(),
                "()[I".to_owned(),
                vec![],
            )
            .unwrap()
            .unwrap();

        let array_ref = match result {
            JvmValue::Reference(Some(r)) => r,
            _ => panic!("expected valid reference"),
        };
        let array = match jvm.heap.get(array_ref) {
            HeapObject::IntArray(items) => items,
            _ => panic!("Expected array"),
        };

        assert_eq!(vec![1, 1, 1, 1, 1, 1, 0, 0, 0, 0], array.clone());
    }

    #[test]
    fn test_interface_test_direct() {
        test_interface_helper(1, 101, "testDirect", 1);
        test_interface_helper(1, 101, "testDirect2", 5);
    }

    #[test]
    fn test_interface_test_parent_as_interface() {
        test_interface_helper(1, 11, "testParentAsInterface", 1);
        test_interface_helper(1, 11, "testParentAsInterface2", 5);
    }

    #[test]
    fn test_interface_test_extended_interface() {
        test_interface_helper(1, 101, "testExtendedInterface", 1);
        test_interface_helper(1, 101, "testExtendedInterface2", 5);
    }

    #[test]
    fn test_interface_test_multi_k() {
        test_interface_helper(1, 6, "testMultiK", 1);
        test_interface_helper(1, 6, "testMultiK2", 5);
    }

    #[test]
    fn test_interface_test_multi_k_child() {
        test_interface_helper(1, 2, "testMultiKChild", 3);
        test_interface_helper(1, 2, "testMultiKChild2", 11);
    }

    #[test]
    fn test_interface_test_multi_l() {
        test_interface_helper(1, 16, "testMultiL", 1);
        test_interface_helper(1, 16, "testMultiL2", 5);
    }

    #[test]
    fn test_interface_test_multi_i_on_child() {
        test_interface_helper(1, 201, "testMultiIOnChild", 1);
        test_interface_helper(1, 201, "testMultiIOnChild2", 5);
    }

    #[test]
    fn test_interface_test_default_interface() {
        test_interface_helper(1, 51, "testDefaultInterface", 1);
        test_interface_helper(1, 51, "testDefaultInterface2", 5);
    }

    fn test_interface_helper(
        input: i32,
        output: i32,
        method_name: impl Into<String>,
        cache_hits: usize,
    ) {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/InterfaceTest.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "InterfaceTest".to_owned(),
                method_name.into(),
                "(I)I".to_owned(),
                vec![JvmValue::Int(input)],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(int) => assert_eq!(output, int),
            _ => panic!("expected int"),
        }
        assert_interfaces_not_duplicated(jvm.class_loader.get_all_loaded_classes());
        assert_eq!(
            cache_hits,
            jvm.class_loader
                .get("InterfaceTest")
                .unwrap()
                .state
                .borrow()
                .cache
                .get_cache_hits()
        );
    }

    fn test_array_exceptions_unhandled_helper(size: i32, index: i32, expected: &str) {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes/".to_owned())]);
        let result = jvm.run_method(
            "ArrayExceptionsTest".to_owned(),
            "index".to_owned(),
            "(II)I".to_owned(),
            vec![JvmValue::Int(size), JvmValue::Int(index)],
        );

        match result {
            Err(err) => match *err {
                JvmError::UnhandledException {
                    reference: _,
                    class_name,
                    fields: _,
                } => assert_eq!(expected, class_name),
                _ => panic!("expected UnhandledException"),
            },
            _ => panic!("expected error"),
        }
    }

    fn test_array_exceptions_handled_helper(size: i32, index: i32, expected: i32) {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes/".to_owned())]);
        let result = jvm
            .run_method(
                "ArrayExceptionsTest".to_owned(),
                "indexCatch".to_owned(),
                "(II)I".to_owned(),
                vec![JvmValue::Int(size), JvmValue::Int(index)],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(int) => assert_eq!(expected, int),
            _ => panic!("expected int"),
        }
    }

    #[test]
    fn test_exceptions_multiple_throw_1() {
        test_exceptions_multiple_throw_helper(1, 1);
    }

    #[test]
    fn test_exceptions_multiple_throw_2() {
        test_exceptions_multiple_throw_helper(2, 2);
    }

    #[test]
    fn test_exceptions_multiple_throw_3() {
        test_exceptions_multiple_throw_helper(3, 30);
    }

    #[test]
    fn test_exceptions_multiple_throw_4() {
        test_exceptions_multiple_throw_helper(4, 40);
    }

    #[test]
    fn test_exceptions_multiple_throw_5() {
        test_exceptions_multiple_throw_helper(5, 5 * 55555);
    }

    #[test]
    fn test_cache_static_once() {
        test_cache_static_helper(1);
    }

    #[test]
    fn test_cache_static_many() {
        test_cache_static_helper(100);
    }

    #[test]
    fn test_cache_virtual_once() {
        test_cache_virtual_helper(1);
    }

    #[test]
    fn test_cache_virtual_many() {
        test_cache_virtual_helper(100);
    }

    #[test]
    fn test_cache_interface_once() {
        test_cache_interface_helper(1);
    }

    #[test]
    #[sequential]
    fn test_cache_interface_many() {
        test_cache_interface_helper(100);
    }

    #[test]
    #[sequential]
    fn test_print_hello_world() {
        test_print_helper("testPrintSimple", "Hello, World!\n");
    }

    #[test]
    #[sequential]
    fn test_print_int() {
        test_print_helper("testPrintInteger", "67\n");
    }

    #[test]
    #[sequential]
    fn test_print_float() {
        test_print_helper("testPrintFloat", "0.2\n0.1");
    }

    #[test]
    #[sequential]
    fn test_run_main() {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes/".to_owned())]);
        let args = vec!["Hello".to_owned(), "World".to_owned(), "!!!".to_owned()];
        jvm.run_main("PrintTest".to_owned(), args).unwrap();

        assert_eq!("Hello\nWorld\n!!!\n", *PRINT_LOG.lock().unwrap());
        PRINT_LOG.lock().unwrap().clear();
    }

    fn test_print_helper(method: impl Into<String>, expected_print: &str) {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes/".to_owned())]);
        let result = jvm
            .run_with_init(
                "PrintTest".to_owned(),
                method.into(),
                "()V".to_owned(),
                vec![],
            )
            .unwrap();

        assert!(result.is_none());
        assert_eq!(expected_print, *PRINT_LOG.lock().unwrap());
        PRINT_LOG.lock().unwrap().clear();
    }

    #[test]
    fn test_string_pool_once() {
        test_string_pool_helper(1);
    }

    #[test]
    fn test_string_pool_many() {
        test_string_pool_helper(20);
    }

    #[test]
    #[sequential]
    fn test_list_multiple() {
        test_list_helper(vec![
            "Hello".to_owned(),
            "World!".to_owned(),
            "Test".to_owned(),
            "String".to_owned(),
            "!!".to_owned(),
        ]);
    }

    #[test]
    #[sequential]
    fn test_list_one() {
        test_list_helper(vec!["Hello".to_owned()]);
    }

    #[test]
    #[sequential]
    fn test_list_empty() {
        test_list_helper(vec![]);
    }

    #[test]
    fn test_lookup_switch_non_default() {
        test_switch_helper("testLookupSwitchInt", 67, 67);
    }

    #[test]
    fn test_lookup_switch_default() {
        test_switch_helper("testLookupSwitchInt", -123, 99);
    }

    #[test]
    fn test_table_switch_non_default() {
        test_switch_helper("testTableSwitchInt", 4, 67);
    }

    #[test]
    fn test_table_switch_default() {
        test_switch_helper("testTableSwitchInt", -123, 99);
    }

    #[test]
    fn test_hash_code_string() {
        test_hash_code_helper("stringHash", 341682506);
    }

    #[test]
    fn test_hash_code_int() {
        test_hash_code_helper("intHash", 67);
    }

    #[test]
    fn test_hash_code_object() {
        let _ = test_hash_code_get_value("objectHash");
    }

    fn test_hash_code_get_value(method: impl Into<String>) -> i32 {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes/".to_owned())]);
        let result = jvm
            .run_method(
                "HashCodeTest".to_owned(),
                method.into(),
                "()I".to_owned(),
                vec![],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(int) => int,
            _ => panic!("expected int"),
        }
    }

    fn test_hash_code_helper(method: impl Into<String>, expected: i32) {
        assert_eq!(expected, test_hash_code_get_value(method));
    }

    fn test_switch_helper(method: impl Into<String>, input: i32, expected: i32) {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes/".to_owned())]);
        let result = jvm
            .run_method(
                "SwitchTest".to_owned(),
                method.into(),
                "(I)I".to_owned(),
                vec![JvmValue::Int(input)],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(int) => assert_eq!(expected, int),
            _ => panic!("expected int"),
        }
    }

    fn test_list_helper(args: Vec<String>) {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes/".to_owned())]);
        jvm.run_main("ListTest".to_owned(), args.clone()).unwrap();

        let log = PRINT_LOG.lock().unwrap().to_owned();
        let lines: Vec<String> = log.lines().map(|s| s.to_owned()).collect();
        let args_len = args.len();
        assert_eq!(args_len + 1, lines.len());
        for (i, arg) in args.into_iter().enumerate() {
            assert_eq!(arg, lines[i]);
        }
        assert_eq!(format!("{}", args_len), *lines.last().unwrap());

        PRINT_LOG.lock().unwrap().clear();
    }

    fn test_string_pool_helper(count: i32) {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes/".to_owned())]);
        let result = jvm
            .run_method(
                "StringPoolTest".to_owned(),
                "test".to_owned(),
                "(I)I".to_owned(),
                vec![JvmValue::Int(count)],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(int) => assert_eq!(4 * count, int),
            _ => panic!("expected int"),
        }
        let test_class = jvm.class_loader.get("StringPoolTest").unwrap();
        let hits = test_class.state.borrow().cache.get_cache_hits();
        assert_eq!((count * 2 - 1) as usize, hits);
    }

    fn test_cache_interface_helper(count: usize) {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/CacheTest.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "CacheTest".to_owned(),
                "testInterface".to_owned(),
                "(III)Z".to_owned(),
                vec![
                    JvmValue::Int(count as i32),
                    JvmValue::Int(55),
                    JvmValue::Int(65),
                ],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(int) => assert_eq!(1, int),
            _ => panic!("expected int"),
        }
        let test_class = jvm.class_loader.get("CacheTest").unwrap();
        let called_class = jvm.class_loader.get("CacheTestClass").unwrap();
        assert_eq!(0, test_class.state.borrow().v_table.get_hits());
        assert_eq!(count - 1, called_class.state.borrow().v_table.get_hits());
        let hits = test_class.state.borrow().cache.get_cache_hits();
        assert_eq!(count, hits);
        assert_interfaces_not_duplicated(jvm.class_loader.get_all_loaded_classes());
    }

    fn test_cache_virtual_helper(count: usize) {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/CacheTest.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "CacheTest".to_owned(),
                "testVirtual".to_owned(),
                "(III)Z".to_owned(),
                vec![
                    JvmValue::Int(count as i32),
                    JvmValue::Int(55),
                    JvmValue::Int(65),
                ],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(int) => assert_eq!(1, int),
            _ => panic!("expected int"),
        }
        let test_class = jvm.class_loader.get("CacheTest").unwrap();
        let called_class = jvm.class_loader.get("CacheTestClass").unwrap();
        assert_eq!(0, test_class.state.borrow().v_table.get_hits());
        assert_eq!(count - 1, called_class.state.borrow().v_table.get_hits());
        let hits = test_class.state.borrow().cache.get_cache_hits();
        assert_eq!(count, hits);
        assert_interfaces_not_duplicated(jvm.class_loader.get_all_loaded_classes());
    }

    fn test_cache_static_helper(count: usize) {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/CacheTest.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "CacheTest".to_owned(),
                "testStatic".to_owned(),
                "(III)Z".to_owned(),
                vec![
                    JvmValue::Int(count as i32),
                    JvmValue::Int(55),
                    JvmValue::Int(56),
                ],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(int) => assert_eq!(1, int),
            _ => panic!("expected int"),
        }
        let test_class = jvm.class_loader.get("CacheTest").unwrap();
        let called_class = jvm.class_loader.get("CacheTestClass").unwrap();
        assert_eq!(0, test_class.state.borrow().v_table.get_hits());
        assert_eq!(0, called_class.state.borrow().v_table.get_hits());
        let hits = test_class.state.borrow().cache.get_cache_hits();
        assert_eq!(count, hits);
        assert_interfaces_not_duplicated(jvm.class_loader.get_all_loaded_classes());
    }

    fn test_exceptions_multiple_throw_helper(input: i32, expected: i32) {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/ExceptionsTest.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "ExceptionsTest".to_owned(),
                "multipleThrows".to_owned(),
                "(I)I".to_owned(),
                vec![JvmValue::Int(input)],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(int) => assert_eq!(expected, int),
            _ => panic!("expected int"),
        }
    }

    fn test_exceptions_unhandled(method: impl Into<String>, input: i32, expected: &str) {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/ExceptionsTest.jar".to_owned(),
        )]);
        let result = jvm.run_method(
            "ExceptionsTest".to_owned(),
            method.into(),
            "(I)I".to_owned(),
            vec![JvmValue::Int(input)],
        );

        match result {
            Err(err) => match *err {
                JvmError::UnhandledException {
                    reference: _,
                    class_name,
                    fields: _,
                } => assert_eq!(expected, class_name),
                _ => panic!("expected propagated exception\n{err:?}"),
            },
            _ => panic!("expected propagated exception\n{result:?}"),
        }
    }

    fn test_exceptions_handled(method: impl Into<String>, input: i32, expected: i32) {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/ExceptionsTest.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "ExceptionsTest".to_owned(),
                method.into(),
                "(I)I".to_owned(),
                vec![JvmValue::Int(input)],
            )
            .unwrap()
            .unwrap();

        match result {
            JvmValue::Int(int) => assert_eq!(expected, int),
            _ => panic!("expected int"),
        }
    }

    fn test_array_length_helper(n: i32) {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run_method(
                "ArrayLengthTest".to_owned(),
                "getLength".to_owned(),
                "(I)I".to_owned(),
                vec![JvmValue::Int(n)],
            )
            .unwrap()
            .unwrap();

        let len = match result {
            JvmValue::Int(int) => int,
            _ => panic!("expected int"),
        };

        assert_eq!(n, len);
    }

    fn test_comparisons_loop_helper(n: i32) {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run_method(
                "ComparisonsTest".to_owned(),
                "iter".to_owned(),
                "(I)[I".to_owned(),
                vec![JvmValue::Int(n)],
            )
            .unwrap()
            .unwrap();

        let array_ref = match result {
            JvmValue::Reference(Some(r)) => r,
            _ => panic!("expected valid reference"),
        };
        let array = match jvm.heap.get(array_ref) {
            HeapObject::IntArray(items) => items,
            _ => panic!("Expected array"),
        };

        assert_eq!(n.max(0) as usize, array.len());
        if n <= 0 {
            return;
        }

        let expected_outputs = 0..n;
        for (actual, expected) in array.iter().zip(expected_outputs) {
            assert_eq!(expected, *actual);
        }
    }

    fn test_comparisons_helper(a: i32, b: i32, c: i32, expected_outputs: &[i32]) {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run_method(
                "ComparisonsTest".to_owned(),
                "comp".to_owned(),
                "(III)[I".to_owned(),
                vec![JvmValue::Int(a), JvmValue::Int(b), JvmValue::Int(c)],
            )
            .unwrap()
            .unwrap();

        let array_ref = match result {
            JvmValue::Reference(Some(r)) => r,
            _ => panic!("expected valid reference"),
        };
        let array = match jvm.heap.get(array_ref) {
            HeapObject::IntArray(items) => items,
            _ => panic!("Expected array"),
        };

        assert_eq!(expected_outputs.len(), array.len());
        for (actual, expected) in array.iter().zip(expected_outputs) {
            assert_eq!(*expected, *actual);
        }
    }

    fn test_gc_helper(min_allocations: usize, expected_heap: usize, is_secondary_call: bool) {
        let contexts = vec![
            ClassSource::Directory("test_classes/".to_owned()),
            ClassSource::Jar("java_libraries/rt.jar".to_owned()),
        ];
        let class_loader = ClassLoader::new(contexts).unwrap();
        let heap = JvmHeap::new(2, min_allocations);
        let mut jvm = Jvm::new(class_loader, heap);
        let method = if is_secondary_call {
            "secondary"
        } else {
            "main"
        }
        .to_owned();
        assert!(
            jvm.run_method("GCTest".to_owned(), method, "()V".to_owned(), vec![],)
                .unwrap()
                .is_none()
        );

        assert_eq!(2, jvm.class_loader.get_loaded_count());
        assert_eq!(expected_heap, jvm.heap.get_allocated_count());
    }

    fn test_static_field_helper(
        method: impl Into<String>,
        input: Vec<JvmValue>,
        expected_outputs: Vec<i32>,
        expected_loaded: usize,
    ) {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/StaticFieldTest.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "StaticFieldTest".to_owned(),
                method.into(),
                "(II)[I".to_owned(),
                input,
            )
            .unwrap()
            .unwrap();

        let array_ref = match result {
            JvmValue::Reference(Some(r)) => r,
            _ => panic!("expected valid reference"),
        };
        let array = match jvm.heap.get(array_ref) {
            HeapObject::IntArray(items) => items,
            _ => panic!("Expected array"),
        };

        assert_eq!(expected_outputs.len(), array.len());
        for (actual, expected) in array.iter().zip(expected_outputs) {
            assert_eq!(expected, *actual);
        }
        assert_eq!(expected_loaded, jvm.class_loader.get_loaded_count());
    }

    fn test_field_shadowing_helper(method: impl Into<String>) {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/FieldShadowingTest.jar".to_owned(),
        )]);
        let result = jvm
            .run_method(
                "FieldShadowingTest".to_owned(),
                method.into(),
                "()[I".to_owned(),
                vec![],
            )
            .unwrap()
            .unwrap();

        let array_ref = match result {
            JvmValue::Reference(Some(r)) => r,
            _ => panic!("expected valid reference"),
        };
        let array = match jvm.heap.get(array_ref) {
            HeapObject::IntArray(items) => items,
            _ => panic!("Expected array"),
        };

        assert_eq!(5, array.len());
        assert_eq!(10, array[0]);
        assert_eq!(20, array[1]);
        assert_eq!(666, array[2]);
        assert_eq!(20, array[3]);
        assert_eq!(30, array[4]);
        assert_eq!(3, jvm.class_loader.get_loaded_count());
    }

    fn test_single_class_static_method_calls_helper(
        param1: i32,
        param2: i32,
        expected_result: i32,
    ) {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run_method(
                "TestMethodCall".to_owned(),
                "mainCall".to_owned(),
                "(II)I".to_owned(),
                vec![JvmValue::Int(param1), JvmValue::Int(param2)],
            )
            .unwrap()
            .unwrap();
        match result {
            JvmValue::Int(x) => assert_eq!(expected_result, x),
            _ => panic!("Expected int result"),
        }
        assert_eq!(1, jvm.class_loader.get_total_cache_hits());
    }

    fn create_jvm(mut contexts: Vec<ClassSource>) -> Jvm {
        contexts.push(ClassSource::Jar("java_libraries/rt.jar".to_owned()));
        contexts.push(ClassSource::Jar("java_libraries/jvankomrt.jar".to_owned()));
        let class_loader = ClassLoader::new(contexts).unwrap();
        let heap = JvmHeap::new(2, 100);
        Jvm::new(class_loader, heap)
    }

    fn assert_interfaces_not_duplicated<'a>(classes: impl Iterator<Item = &'a Rc<JvmClass>>) {
        for class in classes {
            let state = class.state.borrow();
            assert_eq!(class.class_file.interfaces.len(), state.interfaces.len());
            for interface in &state.interfaces {
                let count = state
                    .interfaces
                    .iter()
                    .filter(|int| {
                        interface.class_file.get_class_name() == int.class_file.get_class_name()
                    })
                    .count();
                assert_eq!(1, count)
            }
        }
    }
}
