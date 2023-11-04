//! Mutex (spin-like and blocking(sleep))

use super::UPSafeCell;
use crate::task::TaskControlBlock;
use crate::task::{block_current_and_run_next, suspend_current_and_run_next};
use crate::task::{current_task, wakeup_task};
use alloc::vec::Vec;
use alloc::{collections::VecDeque, sync::Arc, vec};

/// Mutex trait
pub trait Mutex: Sync + Send {
    /// Lock the mutex
    fn lock(&self);
    /// Unlock the mutex
    fn unlock(&self);
    ///
    fn get_available(&self) -> isize;
    ///
    fn get_allocated(&self) -> Option<Vec<usize>>;
    ///
    fn get_need(&self) -> Option<Vec<usize>>;
}

/// Spinlock Mutex struct
pub struct MutexSpin {
    allocated: UPSafeCell<usize>,
    locked: UPSafeCell<bool>,
}

impl MutexSpin {
    /// Create a new spinlock mutex
    pub fn new() -> Self {
        Self {
            allocated: unsafe {
                UPSafeCell::new(0)
            },
            locked: unsafe { UPSafeCell::new(false) },
        }
    }
}

impl Mutex for MutexSpin {
    /// Lock the spinlock mutex
    fn lock(&self) {
        trace!("kernel: MutexSpin::lock");
        loop {
            let mut locked = self.locked.exclusive_access();
            if *locked {
                drop(locked);
                suspend_current_and_run_next();
                continue;
            } else {
                *locked = true;
                let mut allocated = self.allocated.exclusive_access();
                let task = current_task().unwrap();
                let task_inner = task.inner_exclusive_access();
                *allocated = task_inner.res.as_ref().unwrap().tid;
                return;
            }
        }
    }

    fn unlock(&self) {
        trace!("kernel: MutexSpin::unlock");
        let mut locked = self.locked.exclusive_access();
        *locked = false;
    }

    fn get_available(&self) -> isize {
        let locked = self.locked.exclusive_access();
        if *locked {
            0
        }
        else {
            1
        }
    }

    fn get_allocated(&self) -> Option<Vec<usize>> {
        let locked = self.locked.exclusive_access();
        if *locked {
            Some(vec![*self.allocated.exclusive_access()])
        }
        else {
            None
        }
    }

    fn get_need(&self) -> Option<Vec<usize>> {
        None
    }
}

/// Blocking Mutex struct
pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
}

pub struct MutexBlockingInner {
    locked: bool,
    allocated: usize,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl MutexBlocking {
    /// Create a new blocking mutex
    pub fn new() -> Self {
        trace!("kernel: MutexBlocking::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    allocated: 0 as usize,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }
}

impl Mutex for MutexBlocking {
    /// lock the blocking mutex
    fn lock(&self) {
        trace!("kernel: MutexBlocking::lock");
        let mut mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(current_task().unwrap());
            drop(mutex_inner);
            block_current_and_run_next();
        } else {
            let task = current_task().unwrap();
            let task_inner = task.inner_exclusive_access();
            mutex_inner.allocated = task_inner.res.as_ref().unwrap().tid;
            drop(task_inner);
            mutex_inner.locked = true;
        }
    }

    /// unlock the blocking mutex
    fn unlock(&self) {
        trace!("kernel: MutexBlocking::unlock");
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            let task = current_task().unwrap();
            let task_inner = task.inner_exclusive_access();
            mutex_inner.allocated = task_inner.res.as_ref().unwrap().tid;            
            wakeup_task(waking_task);
        } else {
            mutex_inner.locked = false;
        }
    }
    ///
    fn get_available(&self) -> isize {
        let mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            0
        }
        else {
            1
        }
    }
    ///
    fn get_allocated(&self) -> Option<Vec<usize>> {
        let mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            Some(vec![mutex_inner.allocated])
        }
        else {
            None
        }
    }
    ///
    fn get_need(&self) -> Option<Vec<usize>> {
        let mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            let wait_task_count = mutex_inner.wait_queue.len();
            if wait_task_count == 0 {
                None
            }
            else {
                let mut need = Vec::new();
                for i in 0..wait_task_count {
                    let task = &mutex_inner.wait_queue[i];
                    let task_inner = task.inner_exclusive_access();
                    need.push(task_inner.res.as_ref().unwrap().tid);
                }
                Some(need)
            }
        }
        else {
            None
        }
    }
}
