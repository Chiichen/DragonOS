use alloc::sync::Arc;
use bitflags::bitflags;

use super::Syscall;

//https://code.dragonos.org.cn/xref/linux-6.1.9/include/uapi/linux/rseq.h#40
#[repr(C, align(32))]
struct RseqCs {
    version: u32,
    flags: RseqCsFlag,
    start_ip: u64,
    post_commit_offset: u64,
    abort_ip: u64,
}

//https://code.dragonos.org.cn/xref/linux-6.1.9/include/uapi/linux/rseq.h#62
#[repr(C, align(32))]
pub struct Rseq {
    cpu_id_start: u32,
    cpu_id: u32,
    rseq_cs: Option<Arc<RseqCs>>,
}

bitflags! {
   struct RseqCsFlag :u32{
        const RSEQ_CS_FLAG_NO_RESTART_ON_PREEMPT = 1<<0;
        const RSEQ_CS_FLAG_NO_RESTART_ON_SIGNAL=1<<1;
        const RSEQ_CS_FLAG_NO_RESTART_ON_MIGRATE=1<<2;
    }

}

impl Syscall {}
