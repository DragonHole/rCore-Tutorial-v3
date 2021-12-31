use crate::task::{
    suspend_current_and_run_next,
    exit_current_and_run_next,
};
use crate::task;
use crate::timer::get_time_us;
use crate::mm::{VirtAddr, MapPermission};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    println!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    let _us = get_time_us();
    // unsafe {
    //     *ts = TimeVal {
    //         sec: us / 1_000_000,
    //         usec: us % 1_000_000,
    //     };
    // }
    0
}

pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    task::map_new_memory(start, len, port)
}

fn sys_munmap(start: usize, len: usize) -> isize {
    true
}