use std::rc::Rc;

use crate::{
    bytecode::BYTECODE_TABLE,
    class_loader::ClassLoader,
    field_initialisation::determine_static_fields,
    jvm_model::{
        JvmCache, JvmClass, JvmContext, JvmError, JvmHeap, JvmResult, JvmStackFrame, JvmThread,
        JvmValue,
    },
    native_method_resolver::NativeMethodResolver,
};

pub struct JVM {
    class_loader: ClassLoader,
    threads: Vec<JvmThread>,
    heap: JvmHeap,
    cache: JvmCache,
    native_method_resolver: NativeMethodResolver,
}
impl JVM {
    pub fn new(class_loader: ClassLoader) -> Self {
        Self {
            class_loader,
            threads: vec![],
            heap: JvmHeap::new(),
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
        state.is_initialised = true;
        if state.static_fields.is_none() {
            state.static_fields = Some(determine_static_fields(&loaded_class.class_file));
        }
        drop(state);

        // recusrively load super classes
        if let Some(super_class_name) = loaded_class.class_file.get_super_class_name() {
            let super_class = class_loader.get(super_class_name)?;
            loaded_class.state.borrow_mut().super_class = Some(super_class.clone());
            return Self::initialise_class(thread, &super_class, class_loader, class_name);
        }

        Ok(())
    }

    pub fn get_threads(self) -> Vec<JvmThread> {
        self.threads
    }

    pub fn run(
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
                if frame.should_return {
                    if frame.is_void {
                        current_thread.pop();
                    } else {
                        let old_frame = current_thread.pop().unwrap();
                        let return_value = if let Some(value) = old_frame.return_value {
                            value
                        } else {
                            return Err(JvmError::MissingReturnValue.bx());
                        };

                        if current_thread.has_frames() {
                            current_thread
                                .peek()
                                .unwrap()
                                .operand_stack
                                .push(return_value);
                        } else {
                            return Ok(Some(return_value));
                        }
                    };

                    continue;
                }
                debug_assert!(frame.program_counter < bytecode.code.len());
                // DEBUG
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
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use crate::{class_loader::ClassSource, jvm_model::HeapObject};

    use super::*;

    #[test]
    fn test_sum() {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run(
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
            .run(
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
            .run(
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
            .run(
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
            .run(
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
            .run(
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

        assert_eq!(2, jvm.cache.method_call_cache.get_cache_hits());
    }

    #[test]
    fn test_parameter_overload() {
        let mut jvm = create_jvm(vec![ClassSource::Directory("test_classes".to_owned())]);
        let result = jvm
            .run(
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
            .run(
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
    fn test_cross_class_call() {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/CrossCallTest.jar".to_owned(),
        )]);
        let result = jvm
            .run(
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

        assert_eq!(2, jvm.cache.method_call_cache.get_cache_hits());
        assert_eq!(3, jvm.class_loader.get_loaded_count());
    }

    #[test]
    fn test_virtual_call_self() {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/VirtualCallTest.jar".to_owned(),
        )]);
        let result = jvm
            .run(
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
        assert_eq!(1, jvm.cache.method_call_cache.get_cache_hits());
    }

    #[test]
    fn test_virtual_call_other() {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/VirtualCallTest.jar".to_owned(),
        )]);
        let result = jvm
            .run(
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
        assert_eq!(1, jvm.cache.method_call_cache.get_cache_hits());
    }

    #[test]
    fn test_virtual_call_abstract() {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/VirtualCallTest.jar".to_owned(),
        )]);
        let result = jvm
            .run(
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
        assert_eq!(1, jvm.cache.method_call_cache.get_cache_hits());
    }

    #[test]
    fn test_virtual_call_other_with_private() {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/VirtualCallTest.jar".to_owned(),
        )]);
        let result = jvm
            .run(
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
        assert_eq!(3, jvm.cache.method_call_cache.get_cache_hits());
    }

    #[test]
    fn test_non_static_fields() {
        let mut jvm = create_jvm(vec![ClassSource::Jar(
            "test_classes/NonStaticFieldTest.jar".to_owned(),
        )]);
        let result = jvm
            .run(
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
            .run(
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
            .run(
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
            .run(
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
            .run(
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
        assert_eq!(1, jvm.cache.method_call_cache.get_cache_hits());
    }

    fn create_jvm(mut contexts: Vec<ClassSource>) -> JVM {
        contexts.push(ClassSource::Jar("java_libraries/rt.jar".to_owned()));
        let class_loader = ClassLoader::new(contexts).unwrap();
        JVM::new(class_loader)
    }
}
