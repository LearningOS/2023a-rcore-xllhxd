use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    let detect = process_inner.detect_deadlock;

    if detect {
        let task_count = process_inner.tasks.len();
        let mutex_count = process_inner.mutex_list.len();

        if mutex_count <= 0 {
            return -0xdead;
        }

        let mut available = vec![0; mutex_count];
        let mut allocated = vec![vec![0; mutex_count]; task_count];
        let mut need = vec![vec![0; mutex_count]; task_count];

        // 初始化可获得锁，已经分配的锁，和需要的锁
        for i in 0..mutex_count {
            if let Some(mutex_i) = &process_inner.mutex_list[i] {
                available[i] = mutex_i.get_available();
                if let Some(_allocated) = mutex_i.get_allocated() {
                    for j in 0.._allocated.len() {
                        allocated[_allocated[j]][i] += 1;
                    }
                }
                if let Some(_need) = mutex_i.get_need() {
                    for j in 0.._need.len() {
                        need[_need[j]][i] += 1;
                    }
                }
            }
        }

        let task = current_task().unwrap();
        let task_inner = task.inner_exclusive_access();
        let task_res = task_inner.res.as_ref().unwrap();
        let tid = task_res.tid;

        need[tid][mutex_id] += 1;

        let mut if_finish = vec![false; task_count];

        loop {
            let mut task_id: isize = -1;

            for i in 0..task_count {
                let mut task_id_can_run = true;

                for j in 0..mutex_count {
                    if if_finish[i] || need[i][j] > available[j] {
                        task_id_can_run = false;
                        break;
                    }
                }

                if task_id_can_run {
                    task_id = i as isize;
                    break;
                }
            }

            if task_id == -1 as isize {
                break;
            }

            // 释放可以运行的线程的资源
            for i in 0..mutex_count {
                available[i] += allocated[task_id as usize][i];
                if_finish[task_id as usize] = true;
            }
        }

        for i in 0..task_count {
            if !if_finish[i] {
                return -0xdead;
            } 
        }
    }
    drop(process_inner);
    drop(process);
    mutex.lock();
    0
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    
    let detect = process_inner.detect_deadlock;
    if detect {
        let task_count = process_inner.tasks.len();
        let sem_count = process_inner.semaphore_list.len();
        if sem_count <= 0 {
            return -0xdead;
        }

        let mut available = vec![0; sem_count];
        let mut allocated = vec![vec![0; sem_count]; task_count];
        let mut need = vec![vec![0; sem_count]; task_count];

        // 初始化上述三个向量
        for i in 0..sem_count {
            if let Some(sem_i) = &process_inner.semaphore_list[i] {
                available[i] = sem_i.get_available();
                if let Some(_allocated) = sem_i.get_allocated() {
                    for j in 0.._allocated.len() {
                        allocated[_allocated[j]][i] += 1;
                    }
                }
                if let Some(_need) = sem_i.get_need() {
                    for j in 0.._need.len() {
                        need[_need[j]][i] += 1;
                    }
                }
            }
        }

        let task = current_task().unwrap();
        let task_inner = task.inner_exclusive_access();
        let tid = task_inner.res.as_ref().unwrap().tid;
        
        need[tid][sem_id] += 1;

        let mut if_finish = vec![false; task_count];
        loop {
            let mut tid2: isize = -1;
            for i in 0..task_count {
                let mut can_run = true;
                for j in 0..sem_count {
                    if if_finish[i] || need[i][j] > available[j] {
                        can_run = false;
                        break;
                    }
                }
                if can_run {
                    tid2 = i as isize;
                    break;
                }
            }
            if tid2 == -1 as isize {
                break;
            }
            for j in 0..sem_count {
                available[j] += allocated[tid2 as usize][j];
                if_finish[tid2 as usize] = true;
            }
        }

        for i in 0..task_count {
            if !if_finish[i] {
                return -0xdead;
            }
        }

    }
    drop(process_inner);
    sem.down();
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(_enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect");
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();

    match _enabled {
        0 => {
            process_inner.detect_deadlock = false;
            0
        },
        1 => {
            process_inner.detect_deadlock = true;
            0
        },
        _ =>  {
            -1
        }
    }
}
