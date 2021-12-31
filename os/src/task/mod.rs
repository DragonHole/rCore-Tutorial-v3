mod context;
mod switch;
mod task;

use crate::loader::{get_num_app, get_app_data};
use crate::trap::TrapContext;
use crate::sync::UPSafeCell;
use crate::mm::{MapPermission, VirtAddr};
use lazy_static::*;
use switch::__switch;
use task::{TaskControlBlock, TaskStatus};
use alloc::vec::Vec;

pub use context::TaskContext;

pub struct TaskManager {
    num_app: usize,
    inner: UPSafeCell<TaskManagerInner>,
}

struct TaskManagerInner {
    tasks: Vec<TaskControlBlock>,
    current_task: usize,
}

lazy_static! {
    pub static ref TASK_MANAGER: TaskManager = {
        println!("init TASK_MANAGER");
        let num_app = get_num_app();
        println!("num_app = {}", num_app);
        let mut tasks: Vec<TaskControlBlock> = Vec::new();
        for i in 0..num_app {
            tasks.push(TaskControlBlock::new(
                get_app_data(i),
                i,
            ));
        }
        TaskManager {
            num_app,
            inner: unsafe { UPSafeCell::new(TaskManagerInner {
                tasks,
                current_task: 0,
            })},
        }
    };
}

impl TaskManager {
    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let next_task = &mut inner.tasks[0];
        next_task.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &next_task.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        // before this, we should drop local variables that must be dropped manually
        unsafe {
            __switch(
                &mut _unused as *mut _,
                next_task_cx_ptr,
            );
        }
        panic!("unreachable in run_first_task!");
    }

    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].task_status = TaskStatus::Ready;
    }

    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].task_status = TaskStatus::Exited;
    }

    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| {
                inner.tasks[*id].task_status == TaskStatus::Ready
            })
    }

    fn get_current_token(&self) -> usize {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_user_token()
    }

    fn map_memory_area(&self, start: usize, len: usize, port: usize) -> isize {
        let mut inner = self.inner.exclusive_access();
        let current_task = inner.current_task;
        inner.tasks[current_task].map_memory_area(start, len, port)
    }

    fn mmap(&self, start: usize, len: usize, port: usize) -> isize {
        if len == 0 {
            return 0;
        }
        if len > 1073741824{
            return -1;
        }
        if start % 4096 != 0 {
            return -1;
        }
        let mut length = len;
        if len % 4096 != 0 {
            length = len + (4096 - len % 4096);
        }
        if (port & !0x7 != 0) || (port & 0x7 == 0) {
            return -1;
        }
        
        // println!("@");
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        // println!("Start : {:#X}", VirtPageNum::from(start/4096).0);
        let from:usize = start / 4096;
        let to:usize = (start + length) / 4096;
        // println!("from to {} {}", from, to);
        // for vpn in from..to {
        //     if true == inner.tasks[current].memory_set.find_vpn(VirtPageNum::from(vpn)) {
        //         return -1;
        //     }
        // }
        
        let permission = match port {
            1 => MapPermission::U | MapPermission::R,
            2 => MapPermission::U | MapPermission::W,
            3 => MapPermission::U | MapPermission::R | MapPermission::W,
            4 => MapPermission::U | MapPermission::X,
            5 => MapPermission::U | MapPermission::R | MapPermission::X,
            6 => MapPermission::U | MapPermission::X | MapPermission::W,
            _ => MapPermission::U | MapPermission::R | MapPermission::W | MapPermission::X,
        };
    
        inner.tasks[current].memory_set.insert_framed_area(VirtAddr::from(start), VirtAddr::from(start+length), permission);
    
        // for vpn in from..to {
        //     if false == inner.tasks[current].memory_set.find_vpn(VirtPageNum::from(vpn)) {
        //         return -1;
        //     }
        // }
        0
    }

    fn get_current_trap_cx(&self) -> &mut TrapContext {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_trap_cx()
    }

    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            drop(inner);
            // before this, we should drop local variables that must be dropped manually
            unsafe {
                __switch(
                    current_task_cx_ptr,
                    next_task_cx_ptr,
                );
            }
            // go back to user mode
        } else {
            panic!("All applications completed!");
        }
    }
}

pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}

pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}

pub fn current_user_token() -> usize {
    TASK_MANAGER.get_current_token()
}

pub fn current_trap_cx() -> &'static mut TrapContext {
    TASK_MANAGER.get_current_trap_cx()
}

// 这里只是开放给外界的接口
pub fn map_new_memory(start: usize, len: usize, port: usize) -> isize {
    TASK_MANAGER.map_memory_area(start, len, port)
    //TASK_MANAGER.mmap(start, len, port) // not mine
}
