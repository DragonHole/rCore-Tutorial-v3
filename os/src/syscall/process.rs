use crate::task::{
    suspend_current_and_run_next,
    exit_current_and_run_next,
};
use crate::timer::get_time_us;
use crate::mm::{MemorySet, VirtAddr, VPNRange, MapPermission};

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
    // needs to be page aligned
    if !VirtAddr::from(start).aligned() {
        return -1;
    }

    // make sure the upper 63 bits are zero
    if (port & ((1 << 61) - 1) << 3) >> 3 != 0 {
        return -2;
    }

    let mut current_addrspace: MemorySet = current_user_memoryset();
    let new_range = VPNRange::new(start.into(), (start+len).into());

    // check overlap
    for area in current_addrspace.areas.into_iter() {
        if current_addrspace.isoverlap(area.vpn_range) {   // err upon any conflict
            return -1;
        }
    }

    let mut permission: MapPermission = MapPermission::U;

    if port & (1 << 0) != 0 {
        permission &= MapPermission::R;
    }
    if port & (1 << 1) != 0 {
        permission &= MapPermission::W;
    }
    if port & (1 << 2) != 0 {
        permission &= MapPermission::W;
    }

    current_addrspace.insert_framed_area(new_range.get_start().into() , new_range.get_end().into(), permission);

    0
}