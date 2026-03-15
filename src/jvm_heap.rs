use std::{num::NonZeroUsize, rc::Rc};

use crate::jvm_model::{HeapObject, JvmClass, JvmThread, JvmValue};

#[derive(Debug, Clone)]
pub struct JvmHeap {
    heap: Vec<Option<HeapObject>>,
    free_slots: Vec<usize>,
    min_allocations_before_gc: usize,
    /// allocations before GC
    allocations_counter: usize,
    /// if GC should run
    pub should_gc: bool,
}
impl JvmHeap {
    pub fn new(initial_heap_size: usize, min_allocations_before_gc: usize) -> Self {
        let heap_size = initial_heap_size.max(2);

        Self {
            heap: vec![None; heap_size],
            free_slots: (1..heap_size).collect(),
            should_gc: false,
            allocations_counter: 0,
            min_allocations_before_gc,
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
        self.allocations_counter += 1;

        let reference = if let Some(free_index) = self.free_slots.pop() {
            debug_assert!(self.heap[free_index].is_none());
            self.heap[free_index] = Some(object);
            NonZeroUsize::new(free_index).expect("Index should not be zero")
        } else {
            let new_index = self.heap.len();
            self.heap.push(Some(object));
            NonZeroUsize::new(new_index).expect("Index should not be zero")
        };

        self.should_gc = self.free_slots.is_empty()
            && self.allocations_counter >= self.min_allocations_before_gc;

        reference
    }

    pub fn get_allocated_count(&self) -> usize {
        self.heap.len() - self.free_slots.len() - 1
    }

    pub fn perform_gc<'a>(
        &mut self,
        threads: &[&JvmThread],
        strings_in_string_pool: impl Iterator<Item = NonZeroUsize>,
        loaded_classes: impl Iterator<Item = &'a Rc<JvmClass>>,
    ) {
        self.allocations_counter = 0;
        self.should_gc = false;

        let mut reachable_objects = vec![false; self.heap.len()];
        let mut object_refs = vec![];

        Self::find_inital_reachable(
            threads,
            loaded_classes,
            strings_in_string_pool,
            &mut reachable_objects,
            &mut object_refs,
        );
        self.find_secondary_reachable(&mut reachable_objects, &mut object_refs);
        self.free_undreachable_objects(&reachable_objects);
    }

    fn find_inital_reachable<'a>(
        threads: &[&JvmThread],
        loaded_classes: impl Iterator<Item = &'a Rc<JvmClass>>,
        strings_in_string_pool: impl Iterator<Item = NonZeroUsize>,
        reachable_objects: &mut [bool],
        object_refs: &mut Vec<NonZeroUsize>,
    ) {
        for class in loaded_classes {
            if let Some(static_fields) = &class.state.borrow().static_fields {
                for field in static_fields {
                    Self::mark_reachable_if_ref(field.value, reachable_objects, object_refs);
                }
            }
        }

        for string_ref in strings_in_string_pool {
            if !reachable_objects[string_ref.get()] {
                object_refs.push(string_ref);
                reachable_objects[string_ref.get()] = true;
            }
        }

        for thread in threads {
            for frame in thread.get_stack_frames() {
                for var in &frame.local_variables {
                    Self::mark_reachable_if_ref(*var, reachable_objects, object_refs);
                }

                for operand in &frame.operand_stack {
                    Self::mark_reachable_if_ref(*operand, reachable_objects, object_refs);
                }

                if let Some(return_value) = frame.return_value {
                    Self::mark_reachable_if_ref(return_value, reachable_objects, object_refs);
                }
            }
        }
    }

    fn find_secondary_reachable(
        &self,
        reachable_objects: &mut [bool],
        object_refs: &mut Vec<NonZeroUsize>,
    ) {
        while let Some(obj_ref) = object_refs.pop() {
            let obj = self.heap[obj_ref.get()]
                .as_ref()
                .expect("Reference is invalid");

            match obj {
                HeapObject::Object { class: _, fields } => {
                    for field in fields {
                        Self::mark_reachable_if_ref(*field, reachable_objects, object_refs);
                    }
                }
                HeapObject::ObjectArray(arr) => {
                    for reference in arr.array.iter().flatten() {
                        if !reachable_objects[reference.get()] {
                            object_refs.push(*reference);
                            reachable_objects[reference.get()] = true;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn free_undreachable_objects(&mut self, reachable_objects: &[bool]) {
        for (index, obj) in self.heap.iter_mut().enumerate().skip(1) {
            if reachable_objects[index] || obj.is_none() {
                continue;
            }

            *obj = None;
            self.free_slots.push(index);
        }
    }

    fn mark_reachable_if_ref(
        value: JvmValue,
        reachable_objects: &mut [bool],
        object_refs: &mut Vec<NonZeroUsize>,
    ) {
        if let JvmValue::Reference(Some(reference)) = value
            && !reachable_objects[reference.get()]
        {
            object_refs.push(reference);
            reachable_objects[reference.get()] = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        class_loader::{ClassLoader, ClassSource},
        jvm_model::{JvmStackFrame, ObjectArray, ObjectArrayType},
    };

    use super::*;

    #[test]
    fn test_allocate() {
        let mut heap = JvmHeap::new(2, 100);
        let mut class_loader = get_class_loader();
        let r1 = heap.allocate(get_mock_obj(&mut class_loader, None, None));
        let r2 = heap.allocate(get_mock_obj(&mut class_loader, None, None));
        let r3 = heap.allocate(get_mock_obj(&mut class_loader, None, None));

        assert!(r1 != r2 && r1 != r3 && r2 != r3);
        assert_eq!(4, heap.heap.len());
        assert_eq!(3, heap.get_allocated_count());
    }

    #[test]
    fn test_gc_frees_all_unreachable_objects() {
        let mut heap = JvmHeap::new(2, 0);
        let mut class_loader = get_class_loader();

        heap.allocate(get_mock_obj(&mut class_loader, None, None));
        heap.allocate(get_mock_obj(&mut class_loader, None, None));
        heap.allocate(get_mock_obj(&mut class_loader, None, None));
        assert_eq!(3, heap.get_allocated_count());

        heap.perform_gc(&[], std::iter::empty(), std::iter::empty());

        assert_eq!(0, heap.get_allocated_count());
    }

    #[test]
    fn test_gc_frees_all_reachable_objects() {
        let mut heap = JvmHeap::new(2, 0);
        let mut class_loader = get_class_loader();

        let r1 = heap.allocate(get_mock_obj(&mut class_loader, None, None));
        let r2 = heap.allocate(get_mock_obj(&mut class_loader, None, None));
        let r3 = heap.allocate(get_mock_obj(&mut class_loader, Some(r1), Some(r2)));
        assert_eq!(3, heap.get_allocated_count());

        let t = mock_thread(&mut class_loader, Some(r3));
        heap.perform_gc(&[&t], std::iter::empty(), std::iter::empty());

        assert_eq!(3, heap.get_allocated_count());
    }

    #[test]
    fn test_gc_resets_should_gc_flag() {
        let mut heap = JvmHeap::new(2, 1);
        let mut class_loader = get_class_loader();

        heap.allocate(get_mock_obj(&mut class_loader, None, None));
        heap.allocate(get_mock_obj(&mut class_loader, None, None));
        assert!(heap.should_gc);

        heap.perform_gc(&[], std::iter::empty(), std::iter::empty());

        assert!(!heap.should_gc);
    }

    #[test]
    fn test_gc_freed_slots_are_reused() {
        let mut heap = JvmHeap::new(4, 100);
        let mut class_loader = get_class_loader();

        heap.allocate(get_mock_obj(&mut class_loader, None, None));
        heap.allocate(get_mock_obj(&mut class_loader, None, None));
        heap.allocate(get_mock_obj(&mut class_loader, None, None));
        let heap_len_before = heap.heap.len();

        heap.perform_gc(&[], std::iter::empty(), std::iter::empty());

        heap.allocate(get_mock_obj(&mut class_loader, None, None));
        heap.allocate(get_mock_obj(&mut class_loader, None, None));
        heap.allocate(get_mock_obj(&mut class_loader, None, None));

        assert_eq!(heap_len_before, heap.heap.len());
    }

    #[test]
    fn test_gc_array_elements_kept_when_array_is_reachable() {
        let mut heap = JvmHeap::new(4, 100);
        let mut class_loader = get_class_loader();

        let r1 = heap.allocate(get_mock_obj(&mut class_loader, None, None));
        let r2 = heap.allocate(get_mock_obj(&mut class_loader, None, None));
        let arr = heap.allocate(get_mock_arr(&mut class_loader, Some(r1), Some(r2)));

        let t = mock_thread(&mut class_loader, Some(arr));
        heap.perform_gc(&[&t], std::iter::empty(), std::iter::empty());

        assert_eq!(3, heap.get_allocated_count());
    }

    #[test]
    fn test_gc_self_referential_object_is_freed() {
        let mut heap = JvmHeap::new(2, 0);
        let mut class_loader = get_class_loader();

        let r1 = heap.allocate(get_mock_obj(&mut class_loader, None, None));
        let r2 = heap.allocate(get_mock_obj(&mut class_loader, None, None));
        let object = heap.get(r1);
        match object {
            HeapObject::Object { class: _, fields } => {
                fields[0] = JvmValue::Reference(Some(r1));
                fields[1] = JvmValue::Reference(Some(r2));
            }
            _ => panic!("Expected object"),
        }

        let r3 = heap.allocate(get_mock_arr(&mut class_loader, None, None));
        let r4 = heap.allocate(get_mock_obj(&mut class_loader, Some(r3), None));

        let thread = mock_thread(&mut class_loader, Some(r4));
        heap.perform_gc(&[&thread], std::iter::empty(), std::iter::empty());

        assert_eq!(2, heap.get_allocated_count());
    }

    fn mock_thread(class_loader: &mut ClassLoader, reference: Option<NonZeroUsize>) -> JvmThread {
        let mut t = JvmThread::new();
        t.push(JvmStackFrame::new(
            class_loader.get("Test").unwrap(),
            0,
            0,
            vec![JvmValue::Reference(reference)],
        ));

        t
    }

    fn get_mock_obj(
        class_loader: &mut ClassLoader,
        reference1: Option<NonZeroUsize>,
        reference2: Option<NonZeroUsize>,
    ) -> HeapObject {
        HeapObject::Object {
            class: class_loader.get("Test").unwrap(),
            fields: vec![
                JvmValue::Reference(reference1),
                JvmValue::Reference(reference2),
            ],
        }
    }

    fn get_mock_arr(
        class_loader: &mut ClassLoader,
        reference1: Option<NonZeroUsize>,
        reference2: Option<NonZeroUsize>,
    ) -> HeapObject {
        let arr = ObjectArray {
            array: vec![reference1, reference2],
            dimension: NonZeroUsize::new(1).unwrap(),
            object_array_type: ObjectArrayType::Class(class_loader.get("Test").unwrap()),
        };
        HeapObject::ObjectArray(arr)
    }

    fn get_class_loader() -> ClassLoader {
        ClassLoader::new(vec![ClassSource::Directory("test_classes".to_owned())]).unwrap()
    }
}
