use core::sync::atomic::compiler_fence;

use alloc::{string::ToString, sync::Arc};

use crate::{
    arch::interrupt::TrapFrame,
    filesystem::procfs::procfs_register_pid,
    ipc::{signal::flush_signal_handlers, signal_types::SigHandStruct},
    kdebug, kerror,
    libs::rwlock::RwLock,
    process::ProcessFlags,
    syscall::SystemError,
};

use super::{
    kthread::{KernelThreadPcbPrivate, WorkerPrivate},
    KernelStack, Pid, ProcessControlBlock, ProcessManager,
};

bitflags! {
    /// 进程克隆标志
    pub struct CloneFlags: u32 {
        /// 在进程间共享文件系统信息
        const CLONE_FS = (1 << 0);
        /// 克隆时，与父进程共享信号结构体
        const CLONE_SIGNAL = (1 << 1);
        /// 克隆时，与父进程共享信号处理结构体
        const CLONE_SIGHAND = (1 << 2);
        /// 克隆时，将原本被设置为SIG_IGNORE的信号，设置回SIG_DEFAULT
        const CLONE_CLEAR_SIGHAND = (1 << 3);
        /// 在进程间共享虚拟内存空间
        const CLONE_VM = (1 << 4);
        /// 拷贝线程
        const CLONE_THREAD = (1 << 5);
        /// 共享打开的文件
        const CLONE_FILES = (1 << 6);
    }
}

impl ProcessManager {
    /// 创建一个新进程
    ///
    /// ## 参数
    ///
    /// - `current_trapframe`: 当前进程的trapframe
    /// - `clone_flags`: 进程克隆标志
    ///
    /// ## 返回值
    ///
    /// - 成功：返回新进程的pid
    /// - 失败：返回Err(SystemError)，fork失败的话，子线程不会执行。
    ///
    /// ## Safety
    ///
    /// - fork失败的话，子线程不会执行。
    pub fn fork(
        current_trapframe: &mut TrapFrame,
        clone_flags: CloneFlags,
    ) -> Result<Pid, SystemError> {
        let current_pcb = ProcessManager::current_pcb();
        let new_kstack = KernelStack::new()?;
        let name = current_pcb.basic().name().to_string();
        let pcb = ProcessControlBlock::new(name, new_kstack);

        // 克隆架构相关信息
        *pcb.arch_info() = current_pcb.arch_info_irqsave().clone();

        // 为内核线程设置worker private字段。（也许由内核线程机制去做会更好？）
        if current_pcb.flags().contains(ProcessFlags::KTHREAD) {
            *pcb.worker_private() = Some(WorkerPrivate::KernelThread(KernelThreadPcbPrivate::new()))
        }

        // 拷贝标志位
        ProcessManager::copy_flags(&clone_flags, &pcb).unwrap_or_else(|e| {
            panic!(
                "fork: Failed to copy flags from current process, current pid: [{:?}], new pid: [{:?}]. Error: {:?}",
                current_pcb.pid(), pcb.pid(), e
            )
        });

        // 拷贝用户地址空间
        ProcessManager::copy_mm(&clone_flags, &current_pcb, &pcb).unwrap_or_else(|e| {
            panic!(
                "fork: Failed to copy mm from current process, current pid: [{:?}], new pid: [{:?}]. Error: {:?}",
                current_pcb.pid(), pcb.pid(), e
            )
        });

        // 拷贝文件描述符表
        ProcessManager::copy_files(&clone_flags, &current_pcb, &pcb).unwrap_or_else(|e| {
            panic!(
                "fork: Failed to copy files from current process, current pid: [{:?}], new pid: [{:?}]. Error: {:?}",
                current_pcb.pid(), pcb.pid(), e
            )
        });

        ProcessManager::copy_sighand(&clone_flags, &current_pcb, &pcb).unwrap_or_else(|e| {
            panic!(
                "fork: Failed to copy sighands from current process, current pid: [{:?}], new pid: [{:?}]. Error: {:?}",
                current_pcb.pid(), pcb.pid(), e
            )
        });

        // todo: 拷贝信号相关数据

        // 拷贝线程
        ProcessManager::copy_thread(&clone_flags, &current_pcb, &pcb, &current_trapframe).unwrap_or_else(|e| {
            panic!(
                "fork: Failed to copy thread from current process, current pid: [{:?}], new pid: [{:?}]. Error: {:?}",
                current_pcb.pid(), pcb.pid(), e
            )
        });

        ProcessManager::add_pcb(pcb.clone());

        // 向procfs注册进程
        procfs_register_pid(pcb.pid()).unwrap_or_else(|e| {
            panic!(
                "fork: Failed to register pid to procfs, pid: [{:?}]. Error: {:?}",
                pcb.pid(),
                e
            )
        });

        ProcessManager::wakeup(&pcb).unwrap_or_else(|e| {
            panic!(
                "fork: Failed to wakeup new process, pid: [{:?}]. Error: {:?}",
                pcb.pid(),
                e
            )
        });

        return Ok(pcb.pid());
    }

    fn copy_flags(
        clone_flags: &CloneFlags,
        new_pcb: &Arc<ProcessControlBlock>,
    ) -> Result<(), SystemError> {
        if clone_flags.contains(CloneFlags::CLONE_VM) {
            new_pcb.flags().insert(ProcessFlags::VFORK);
        }
        *new_pcb.flags.lock() = ProcessManager::current_pcb().flags().clone();
        return Ok(());
    }

    /// 拷贝进程的地址空间
    ///
    /// ## 参数
    ///
    /// - `clone_vm`: 是否与父进程共享地址空间。true表示共享
    /// - `new_pcb`: 新进程的pcb
    ///
    /// ## 返回值
    ///
    /// - 成功：返回Ok(())
    /// - 失败：返回Err(SystemError)
    ///
    /// ## Panic
    ///
    /// - 如果当前进程没有用户地址空间，则panic
    fn copy_mm(
        clone_flags: &CloneFlags,
        current_pcb: &Arc<ProcessControlBlock>,
        new_pcb: &Arc<ProcessControlBlock>,
    ) -> Result<(), SystemError> {
        let old_address_space = current_pcb.basic().user_vm().unwrap_or_else(|| {
            panic!(
                "copy_mm: Failed to get address space of current process, current pid: [{:?}]",
                current_pcb.pid()
            )
        });

        if clone_flags.contains(CloneFlags::CLONE_VM) {
            unsafe { new_pcb.basic_mut().set_user_vm(Some(old_address_space)) };
            return Ok(());
        }

        let new_address_space = old_address_space.write().try_clone().unwrap_or_else(|e| {
            panic!(
                "copy_mm: Failed to clone address space of current process, current pid: [{:?}], new pid: [{:?}]. Error: {:?}",
                current_pcb.pid(), new_pcb.pid(), e
            )
        });
        unsafe { new_pcb.basic_mut().set_user_vm(Some(new_address_space)) };
        return Ok(());
    }

    fn copy_files(
        clone_flags: &CloneFlags,
        current_pcb: &Arc<ProcessControlBlock>,
        new_pcb: &Arc<ProcessControlBlock>,
    ) -> Result<(), SystemError> {
        // 如果不共享文件描述符表，则拷贝文件描述符表
        if !clone_flags.contains(CloneFlags::CLONE_FILES) {
            let new_fd_table = current_pcb.basic().fd_table().unwrap().read().clone();
            let new_fd_table = Arc::new(RwLock::new(new_fd_table));
            new_pcb.basic_mut().set_fd_table(Some(new_fd_table));
        } else {
            // 如果共享文件描述符表，则直接拷贝指针
            new_pcb
                .basic_mut()
                .set_fd_table(current_pcb.basic().fd_table().clone());
        }

        return Ok(());
    }

    #[allow(dead_code)]
    fn copy_sighand(
        clone_flags: &CloneFlags,
        current_pcb: &Arc<ProcessControlBlock>,
        new_pcb: &Arc<ProcessControlBlock>,
    ) -> Result<(), SystemError> {
        kdebug!("process_copy_sighand");

        // 因为在信号处理里面，我们没有使用内部可变的锁来保护 Arc 的只读特性，而是通过裸指针绕过了这个规则
        // 所以不能跨进程直接复制 Arc 指针，只能重新创建一个实例
        let sig_hand_struct: Arc<SigHandStruct> = Arc::new(SigHandStruct::default());

        new_pcb.sig_struct().handler = sig_hand_struct;
        compiler_fence(core::sync::atomic::Ordering::SeqCst);
        // // 将信号的处理函数设置为default(除了那些被手动屏蔽的)
        if clone_flags.contains(CloneFlags::CLONE_CLEAR_SIGHAND) {
            compiler_fence(core::sync::atomic::Ordering::SeqCst);

            flush_signal_handlers(new_pcb.clone(), false);
            compiler_fence(core::sync::atomic::Ordering::SeqCst);
        }
        compiler_fence(core::sync::atomic::Ordering::SeqCst);

        if clone_flags.contains(CloneFlags::CLONE_SIGHAND) {
            let new_sig_hand_struct: &mut SigHandStruct;
            let sig_hand_struct_ptr =
                Arc::as_ptr(&new_pcb.sig_struct_irq().handler) as *mut SigHandStruct;
            unsafe {
                let r = sig_hand_struct_ptr.as_mut();
                if r.is_none() {
                    kerror!(
                        "error to copy sig action since the convertion from raw pointer failed"
                    );
                    return Err(SystemError::EINVAL);
                }
                new_sig_hand_struct = r.unwrap();
            }
            let current_sig_hand_struct = current_pcb.sig_struct();
            for (index, action) in new_sig_hand_struct.0.iter_mut().enumerate() {
                compiler_fence(core::sync::atomic::Ordering::SeqCst);
                (*action) = current_sig_hand_struct.handler.0[index].clone();
                compiler_fence(core::sync::atomic::Ordering::SeqCst);
            }
        }
        compiler_fence(core::sync::atomic::Ordering::SeqCst);
        return Ok(());
    }
}
