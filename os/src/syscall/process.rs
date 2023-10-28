//! Process management syscalls
use crate::{
    config::{MAX_SYSCALL_NUM, PAGE_SIZE},
    task::{
        change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus, current_user_token, update_task_info, mmap, get_current_task_page_table_entry,
        munmap,
    }, 
    timer::get_time_us,
    mm::{translate_pointer, VirtAddr, MapPermission, VPNRange},
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    pub status: TaskStatus,
    /// The numbers of syscall called by task
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    pub time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let kerner_time = translate_pointer(_ts, current_user_token());

    unsafe {
        *kerner_time = TimeVal {
            sec: us / 1000000,
            usec: us % 1000000,
        }
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info");
    let kernel_taskinfo = translate_pointer(_ti, current_user_token());
    update_task_info(kernel_taskinfo);
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap");
    // handle the error mentioned in the book
    if _start % PAGE_SIZE != 0{
        return -1;
    }

    if _port & !0x7 != 0 || _port & 0x7 == 0 {
        return -1;
    }
    
    let start_vpn = VirtAddr::from(_start).floor();
    let end_vpn = VirtAddr::from(_start + _len).ceil();
    let iter_vpn = VPNRange::new(start_vpn, end_vpn);
    for vpn in iter_vpn {
        if let Some(pte) = get_current_task_page_table_entry(vpn) {
            if pte.is_valid() {
                return -1;
            }
        }
    }

    mmap(VirtAddr::from(_start), 
        VirtAddr::from(_start + _len),  
        MapPermission::from_bits_truncate((_port << 1) as u8 )| MapPermission::U
    );
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap");
    // 对齐
    if _start % PAGE_SIZE != 0{
        return -1;
    }
    // 确保：所有的虚拟内存都被映射
    let start_vpn = VirtAddr::from(_start).floor();
    let end_vpn = VirtAddr::from(_start + _len).ceil();
    let iter_vpn = VPNRange::new(start_vpn, end_vpn);
    for vpn in iter_vpn {
        if let Some(pte) = get_current_task_page_table_entry(vpn) {
            if !pte.is_valid() {
                return -1;
            }
        }
        else {
            return -1;
        }
    }

    munmap(VirtAddr::from(_start), VirtAddr::from(_start + _len));
    0
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
