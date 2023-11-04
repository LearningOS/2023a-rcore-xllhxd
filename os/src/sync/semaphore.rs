//! Semaphore

use crate::sync::UPSafeCell;
use crate::task::{block_current_and_run_next, current_task, wakeup_task, TaskControlBlock};
use alloc::vec::Vec;
use alloc::{collections::VecDeque, sync::Arc};

/// semaphore structure
pub struct Semaphore {
    /// semaphore inner
    pub inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub allocated: Vec<usize>,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Semaphore {
    /// Create a new semaphore
    pub fn new(res_count: usize) -> Self {
        trace!("kernel: Semaphore::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    allocated: Vec::new(),
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }

    /// up operation of semaphore
    pub fn up(&self) {
        trace!("kernel: Semaphore::up");
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                wakeup_task(task);
            }
        }
    }

    /// down operation of semaphore
    pub fn down(&self) {
        trace!("kernel: Semaphore::down");
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
        }
        else {
            let task = current_task().unwrap();
            let task_inner = task.inner_exclusive_access();
            let tid = task_inner.res.as_ref().unwrap().tid;
            inner.allocated.push(tid);
            drop(task_inner);
        }
    }
    ///
    pub fn get_available(&self) -> isize {
        let inner = self.inner.exclusive_access();
        if inner.count < 0 {
            0
        }
        else {
            inner.count
        }
    }
    ///
    pub fn get_allocated(&self) -> Option<Vec<usize>> {
        let inner = self.inner.exclusive_access();
        if inner.allocated.len() > 0 {
            Some(inner.allocated.clone())
        }
        else {
            None
        }
    }
    ///
    pub fn get_need(&self) -> Option<Vec<usize>> {
        let inner = self.inner.exclusive_access();

        let wait_task_count = inner.wait_queue.len();
        if wait_task_count == 0 {
            None
        }
        else {
            let mut task_need_sem_id = Vec::new();
            for i in 0..wait_task_count {
                let task = &inner.wait_queue[i];
                let task_inner = task.inner_exclusive_access();
                task_need_sem_id.push(task_inner.res.as_ref().unwrap().tid);
            }
            Some(task_need_sem_id)
        }
    }
}
