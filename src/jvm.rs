use crate::{
    bytecode::BYTECODE_TABLE,
    class_loader::{ClassLoader, LoadedClass},
    jvm_model::{JvmContext, JvmError, JvmHeap, JvmResult, JvmStackFrame, JvmThread, JvmValue},
    method_call_cache::MethodCallCache,
};

pub struct JVM {
    class_loader: ClassLoader,
    threads: Vec<JvmThread>,
    heap: JvmHeap,
    method_call_cache: MethodCallCache,
}
impl JVM {
    pub fn new(class_loader: ClassLoader) -> Self {
        Self {
            class_loader,
            threads: vec![],
            heap: JvmHeap::new(),
            method_call_cache: MethodCallCache::new(),
        }
    }

    pub fn initialise_class(thread: &mut JvmThread, loaded_class: &LoadedClass) -> JvmResult<()> {
        const INITIALISE_CLASS_METHOD: &str = "<clinit>";
        const INITIALISE_CLASS_DESCIPTOR: &str = "()V";
        if loaded_class.is_initialised {
            return Ok(());
        }

        let (method_index, bytecode_index) = if let Some((m_index, b_index)) = loaded_class
            .class
            .get_method_and_bytecode_index(INITIALISE_CLASS_METHOD, INITIALISE_CLASS_DESCIPTOR)
        {
            (m_index, b_index)
        } else {
            return Ok(());
        };

        thread.push(JvmStackFrame::new(
            loaded_class.class.clone(),
            method_index,
            bytecode_index,
            vec![],
        ));

        Ok(())
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
            .class
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

        let mut thread = JvmThread::new();
        let stack_frame = JvmStackFrame::new(
            loaded_class.class.clone(),
            method_index,
            bytecode_index,
            params,
        );
        thread.push(stack_frame);
        Self::initialise_class(&mut thread, &loaded_class)?;
        self.threads.push(thread);

        self.run_thread()
    }

    fn run_thread(&mut self) -> JvmResult<Option<JvmValue>> {
        assert!(!self.threads.is_empty());
        let current_thread = &mut self.threads[0];

        while let Some(frame) = current_thread.peek() {
            let instruction = {
                let method = &frame.class.methods[frame.method_index];
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
                    method_call_cache: &mut self.method_call_cache,
                },
            )?;
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use crate::class_loader::ClassSource;

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

        assert_eq!(2, jvm.method_call_cache.get_cache_hits());
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
    }

    fn create_jvm(contexts: Vec<ClassSource>) -> JVM {
        let class_loader = ClassLoader::new(contexts).unwrap();
        JVM::new(class_loader)
    }
}
