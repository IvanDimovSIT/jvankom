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
        loaded_classes: impl Iterator<Item = &'a Rc<JvmClass>>,
    ) {
        self.allocations_counter = 0;
        self.should_gc = false;

        let mut reachable_objects = vec![false; self.heap.len()];
        let mut object_refs = vec![];

        Self::find_inital_reachable(
            threads,
            loaded_classes,
            &mut reachable_objects,
            &mut object_refs,
        );
        self.find_secondary_reachable(&mut reachable_objects, &mut object_refs);
        self.free_undreachable_objects(&reachable_objects);
    }

    fn find_inital_reachable<'a>(
        threads: &[&JvmThread],
        loaded_classes: impl Iterator<Item = &'a Rc<JvmClass>>,
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
                HeapObject::ObjectArray(refs) => {
                    for reference in refs.iter().flatten() {
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
