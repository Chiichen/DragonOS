#pragma once
#include <driver/usb/usb.h>
#include <driver/pci/pci.h>

#define MAX_XHCI_HOST_CONTROLLERS 8

// xhci Capability Registers offset
#define XHCI_CAPS_CAPLENGTH 0x00 // Cap 寄存器组的长度
#define XHCI_CAPS_RESERVED 0x01
#define XHCI_CAPS_HCIVERSION 0x02 // 接口版本号
#define XHCI_CAPS_HCSPARAMS1 0x04
#define XHCI_CAPS_HCSPARAMS2 0x08
#define XHCI_CAPS_HCSPARAMS3 0x0c
#define XHCI_CAPS_HCCPARAMS1 0x10 // capability params 1
#define XHCI_CAPS_DBOFF 0x14      // Doorbell offset
#define XHCI_CAPS_RTSOFF 0x18     // Runtime register space offset
#define XHCI_CAPS_HCCPARAMS2 0x1c // capability params 2

struct xhci_caps_HCSPARAMS1_reg_t
{
    unsigned max_slots : 8;  // 最大插槽数
    unsigned max_intrs : 11; // 最大中断数
    unsigned reserved : 5;
    unsigned max_ports : 8; // 最大端口数
} __attribute__((packed));

struct xhci_caps_HCSPARAMS2_reg_t
{
    unsigned ist : 4;      // 同步调度阈值
    unsigned ERST_Max : 4; // Event Ring Segment Table Max
    unsigned Reserved : 13;
    unsigned max_scratchpad_buf_HI5 : 5; // 草稿行buffer地址（高5bit）
    unsigned spr : 1;                    // scratchpad restore
    unsigned max_scratchpad_buf_LO5 : 5; // 草稿行buffer地址（低5bit）
} __attribute__((packed));

struct xhci_caps_HCSPARAMS3_reg_t
{
    uint8_t u1_device_exit_latency; // 0~10ms
    uint8_t Reserved;
    uint16_t u2_device_exit_latency; // 0~2047ms
} __attribute__((packed));

struct xhci_caps_HCCPARAMS1_reg_t
{
    unsigned ac64 : 1; // 64-bit addressing capability
    unsigned bnc : 1;  // bw negotiation capability
    unsigned csz : 1;  // context size
    unsigned ppc : 1;  // 端口电源控制
    unsigned pind : 1; // port indicators
    unsigned lhrc : 1; // Light HC reset capability
    unsigned ltc : 1;  // latency tolerance messaging capability
    unsigned nss : 1;  // no secondary SID support

    unsigned pae : 1;        // parse all event data
    unsigned spc : 1;        // Stopped - Short packet capability
    unsigned sec : 1;        // Stopped EDTLA capability
    unsigned cfc : 1;        // Continuous Frame ID capability
    unsigned MaxPSASize : 4; // Max Primary Stream Array Size

    uint16_t xECP; // xhci extended capabilities pointer

} __attribute__((packed));

struct xhci_caps_HCCPARAMS2_reg_t
{
    unsigned u3c : 1; // U3 Entry Capability
    unsigned cmc : 1; // ConfigEP command Max exit latency too large
    unsigned fsc : 1; // Force Save Context Capability
    unsigned ctc : 1; // Compliance Transition Capability
    unsigned lec : 1; // large ESIT payload capability
    unsigned cic : 1; // configuration information capability
    unsigned Reserved : 26;
} __attribute__((packed));

// xhci operational registers offset
#define XHCI_OPS_USBCMD 0x00   // USB Command
#define XHCI_OPS_USBSTS 0x04   // USB status
#define XHCI_OPS_PAGESIZE 0x08 // Page size
#define XHCI_OPS_DNCTRL 0x14   // Device notification control
#define XHCI_OPS_CRCR 0x18     // Command ring control
#define XHCI_OPS_DCBAAP 0x30   // Device context base address array pointer
#define XHCI_OPS_CONFIG 0x38   // configuire
#define XHCI_OPS_PRS 0x400     // Port register sets

struct xhci_ops_usbcmd_reg_t
{
    unsigned rs : 1;          // Run/Stop
    unsigned hcrst : 1;       // host controller reset
    unsigned inte : 1;        // Interrupt enable
    unsigned hsee : 1;        // Host system error enable
    unsigned rsvd_psvd1 : 3;  // Reserved and preserved
    unsigned lhcrst : 1;      // light host controller reset
    unsigned css : 1;         // controller save state
    unsigned crs : 1;         // controller restore state
    unsigned ewe : 1;         // enable wrap event
    unsigned ue3s : 1;        // enable U3 MFINDEX Stop
    unsigned spe : 1;         // stopped short packet enable
    unsigned cme : 1;         // CEM Enable
    unsigned rsvd_psvd2 : 18; // Reserved and preserved
} __attribute__((packed));

struct xhci_ops_usbsts_reg_t
{
    unsigned HCHalted : 1;
    unsigned rsvd_psvd1 : 1;  // Reserved and preserved
    unsigned hse : 1;         // Host system error
    unsigned eint : 1;        // event interrupt
    unsigned pcd : 1;         // Port change detected
    unsigned rsvd_zerod : 3;  // Reserved and Zero'd
    unsigned sss : 1;         // Save State Status
    unsigned rss : 1;         // restore state status
    unsigned sre : 1;         // save/restore error
    unsigned cnr : 1;         // controller not ready
    unsigned hce : 1;         // host controller error
    unsigned rsvd_psvd2 : 19; // Reserved and Preserved
} __attribute__((packed));

struct xhci_ops_pagesize_reg_t
{
    uint16_t page_size; // The actual pagesize is ((this field)<<12)
    uint16_t reserved;
} __attribute__((packed));

struct xhci_ops_dnctrl_reg_t
{
    uint16_t value;
    uint16_t reserved;
} __attribute__((packed));

struct xhci_ops_config_reg_t
{
    uint8_t MaxSlotsEn;      // Max slots enabled
    unsigned u3e : 1;        // U3 Entry Enable
    unsigned cie : 1;        // Configuration information enable
    unsigned rsvd_psvd : 22; // Reserved and Preserved
} __attribute__((packed));

/**
 * @brief xhci端口信息
 *
 */
struct xhci_port_info_t
{
    uint8_t flags;           // port flags
    uint8_t paired_port_num; // 与当前端口所配对的另一个端口（相同物理接口的不同速度的port）
    uint8_t offset;          // offset of this port within this protocal
    uint8_t reserved;
} __attribute__((packed));

struct xhci_host_controller_t
{
    struct pci_device_structure_general_device_t *pci_dev_hdr; // 指向pci header结构体的指针
    int controller_id;                                         // 操作系统给controller的编号
    uint64_t vbase;                                            // 虚拟地址base（bar0映射到的虚拟地址）
    uint64_t vbase_op;                                         // Operational registers 起始虚拟地址
    struct xhci_port_info_t *ports;                            // 指向端口信息数组的指针
};

/**
 * @brief 初始化xhci控制器
 *
 * @param header 指定控制器的pci device头部
 */
void xhci_init(struct pci_device_structure_general_device_t *header);