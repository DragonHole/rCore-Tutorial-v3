use crate::mm::{MemorySet, MapPermission, PhysPageNum, KERNEL_SPACE, VirtAddr, VirtPageNum, VPNRange};
use crate::trap::{TrapContext, trap_handler};
use crate::config::{TRAP_CONTEXT, kernel_stack_position, PAGE_SIZE};
use super::TaskContext;

pub struct TaskControlBlock {
    pub task_status: TaskStatus,
    pub task_cx: TaskContext,
    pub memory_set: MemorySet,
    pub trap_cx_ppn: PhysPageNum,
    pub base_size: usize,
    shame: usize,
}

impl TaskControlBlock {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    pub fn map_memory_area(&mut self, start: usize, len: usize, port: usize) -> isize {
        // needs to be page aligned
        if !VirtAddr::from(start).aligned() {
            return -1;
        }

        //println!("port : {:#b}, shame : {:#b}", port, self.shame);
        //self.shame += 1;

        // make sure the upper 63 bits are zero, and lowest 3 bits not all zero
        if (port & !0x7 != 0) || (port & 0x7 == 0) {
            return -1;
        }

        let mut permission: MapPermission = MapPermission::U;

        if port & (1 << 0) != 0 {
            permission |= MapPermission::R;
        }
        if port & (1 << 1) != 0 {
            permission |= MapPermission::W;
        }
        if port & (1 << 2) != 0 {
            permission |= MapPermission::X;
        }

        // let mut permission = match port {
        //     1 => MapPermission::U | MapPermission::R,
        //     2 => MapPermission::U | MapPermission::W,
        //     3 => MapPermission::U | MapPermission::R | MapPermission::W,
        //     4 => MapPermission::U | MapPermission::X,
        //     5 => MapPermission::U | MapPermission::R | MapPermission::X,
        //     6 => MapPermission::U | MapPermission::X | MapPermission::W,
        //     _ => MapPermission::U | MapPermission::R | MapPermission::W | MapPermission::X,
        // };

        let from: usize = start;
        let to: usize = start + len;

        let new_range = VPNRange::new(VirtPageNum::from(VirtAddr::from(start)), VirtPageNum::from(VirtAddr::from(start+len).ceil()));
        println!("new: {} {}", new_range.get_start().0, new_range.get_end().0);

        //check overlap
        for area in &self.memory_set.areas {
            // println!("{} {} {} {}", area.vpn_range.get_start().0, area.vpn_range.get_end().0, new_range.get_start().0, new_range.get_end().0);
            
            if new_range.is_overlap(area.vpn_range) {   // err upon any conflict
               return -1;
            }
        }

        // println!("from to {} {}", from, to);
        // for vpn in from..to {
        //     if true == inner.tasks[current].memory_set.find_vpn(VirtPageNum::from(vpn)) {
        //         return -5;
        //     }
        // }

        //permission = MapPermission::U | MapPermission::R | MapPermission::W | MapPermission::X;

        // println!("{:#x}, {:#x}", VirtAddr::from(new_range.get_start()).0, VirtAddr::from(new_range.get_end()).0);
        self.memory_set.insert_framed_area(VirtAddr::from(new_range.get_start()) , VirtAddr::from(new_range.get_end()), permission);
        // self.memory_set.insert_framed_area(VirtAddr::from(0x10000000), VirtAddr::from(0x10001000), permission);

        // for vpn in from..to {
        //     if false == self.memory_set.find_vpn(VirtPageNum::from(vpn)) {
        //         return -9;
        //     }
        // }

        0
    }
    
    pub fn new(elf_data: &[u8], app_id: usize) -> Self {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        let task_status = TaskStatus::Ready;
        // map a kernel-stack in kernel space
        let (kernel_stack_bottom, kernel_stack_top) = kernel_stack_position(app_id);
        KERNEL_SPACE
            .exclusive_access()
            .insert_framed_area(
                kernel_stack_bottom.into(),
                kernel_stack_top.into(),
                MapPermission::R | MapPermission::W,
            );
        let task_control_block = Self {
            task_status,
            task_cx: TaskContext::goto_trap_return(kernel_stack_top),
            memory_set,
            trap_cx_ppn,
            base_size: user_sp,
            shame: 0,
        };
        // prepare TrapContext in user space
        let trap_cx = task_control_block.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    Ready,
    Running,
    Exited,
}