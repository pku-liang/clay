use crate::ast::{BinaryOp, UnaryOp};
use crate::pass::{
    cdfg::{BasicBlock, BlockIdx, CDFG},
    op::{Op, OpIdx},
    regularize::{regularize_cdfg, BufferAlloc, RegularizedBB, RegularizedOp},
    sched::{Schedule, ScheduledAt},
    type_infer::TypeInferContext,
};

use super::*;
use cmt2::{cmtc::verilator::instance, cmtrs::*, *};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::Rc;
use std::{collections::HashSet, path::PathBuf};
bundle_decl! {
  pub struct RiscvRtypeInstn {
      funct7:  Type::UInt(7),
      rs2:    Type::UInt(5),
      rs1:    Type::UInt(5),
      funct3: Type::UInt(3),
      rd:     Type::UInt(5),
      opcode: Type::UInt(7)
  }
}
bundle_decl! {
    pub(crate) struct RoccCmd {
        funct:  Type::UInt(7),
        rs1:    Type::UInt(5),
        rs2:    Type::UInt(5),
        rd:     Type::UInt(5),
        xs1:    Type::UInt(1),
        xs2:    Type::UInt(1),
        xd:     Type::UInt(1),
        opcode: Type::UInt(7),
        rs1data:Type::UInt(32),
        rs2data:Type::UInt(32)
    }
}

bundle_decl! {
    pub(crate) struct RoccResp {
        rd:     Type::UInt(5),
        rddata: Type::UInt(32)
    }
}

bundle_decl! {
    struct HellaCacheCmd {
        addr:   Type::UInt(32),
        tag:    Type::UInt(8),
        cmd:    Type::UInt(5),  // XRD = 0, XWR = 1
        size:   Type::UInt(2),
        signed: Type::UInt(1),
        phys:   Type::UInt(1),
        data:   Type::UInt(32),
        mask:   Type::UInt(4)
    }
}

bundle_decl! {
    struct HellaCacheResp {
        addr:   Type::UInt(32),
        tag:    Type::UInt(8),
        cmd:    Type::UInt(5),
        size:   Type::UInt(2),
        signed: Type::UInt(1),
        data:   Type::UInt(32),
        mask:   Type::UInt(4)
    }
}

enum_decl! {
  pub(crate) enum MemoryOP {MEMWRITE, MEMREAD}
}

bundle_decl! {
    pub(crate) struct UserMemoryCmd {
        addr: Type::UInt(32),
        cmd:  Type::UInt(1),
        size: Type::UInt(2),
        data: Type::UInt(32),
        mask: Type::UInt(4),
        tag:  Type::UInt(8)
    }
}

bundle_decl! {
    pub(crate) struct UserMemoryResp {
        data: Type::UInt(32),
        tag:  Type::UInt(8)
    }
}

itfc_declare! {
    param HellaCMD_T;
    struct HellaVSlave {
        cmd_bus: input param HellaCMD_T
    };
    method cmd_to_bus(cmd_bus);
}

#[module]
fn mk_hella_vslave() -> HellaVSlave {
    let hella_cmd_t: Type = HellaCacheCmd::new().into();
    let io = io! {HellaCMD_T: hella_cmd_t.clone()};
    let cmd_to_bus = method! {
        (io.cmd_bus) {
            // dummy action
        }
    };
}

itfc_declare!(
    param HellaCMD_T;
    param HellaRESP_T;
    param MemUserCMD_T;
    param MemUserRESP_T;
    pub(crate) struct HellaAdapter {
        hella_resp_bus: input param HellaRESP_T,
        user_cmd_bus: input param MemUserCMD_T,
        user_resp_bus: output param MemUserRESP_T
    };
    method resp_from_bus(hella_resp_bus);
    method cmd_from_user(user_cmd_bus);
    method resp_to_user() -> (user_resp_bus);
);

#[module("synthesis": "true")]
fn mk_hella_adapter(hella_slave: &HellaVSlave) -> HellaAdapter {
    let hella_cmd_t: Type = HellaCacheCmd::new().into();
    let hella_resp_t: Type = HellaCacheResp::new().into();
    let memuser_cmd_t: Type = UserMemoryCmd::new().into();
    let memuser_resp_t: Type = UserMemoryResp::new().into();

    let io = io! {
      HellaCMD_T: hella_cmd_t.clone(),
      HellaRESP_T: hella_resp_t.clone(),
      MemUserCMD_T: memuser_cmd_t.clone(),
      MemUserRESP_T: memuser_resp_t.clone()
    };
    set_name!("HellaAdapter".to_string());

    let hella_slave = friend!(hella_slave);

    let hella_cmd_queue = instance!(mk_bypass_buffer(&hella_cmd_t));

    let cmd_from_user = method! {
      (io.user_cmd_bus) {
        let user_cmd = UserMemoryCmdBundle::from(io.user_cmd_bus);
        let hella_cmd_op = (&user_cmd).cmd.clone().eq(MemoryOP::MEMREAD.lit()).mux(0.uint(5), 1.uint(5));
        let hella_cmd = HellaCacheCmdBundle {
          addr: (&user_cmd).addr.clone(),
          tag: user_cmd.tag,
          cmd: hella_cmd_op,
          size: user_cmd.size,
          signed: false.uint(1),
          phys: false.uint(1),
          data: user_cmd.data,
          mask: user_cmd.mask
        };
        hella_cmd_queue.push(hella_cmd.create());
      }
    };

    // buffer slot 0
    let slot_0 = instance!(stl::reg(&Type::UInt(32)));
    let slot_0_txd = instance!(stl::reg(&Type::UInt(1)));
    let slot_0_rxd = instance!(stl::reg(&Type::UInt(1)));
    let slot_0_tag = instance!(stl::reg(&Type::UInt(32)));

    // buffer slot 1
    let slot_1 = instance!(stl::reg(&Type::UInt(32)));
    let slot_1_txd = instance!(stl::reg(&Type::UInt(1)));
    let slot_1_rxd = instance!(stl::reg(&Type::UInt(1)));
    let slot_1_tag = instance!(stl::reg(&Type::UInt(32)));

    let newer_slot = instance!(stl::reg(&Type::UInt(1)));

    let slot_0_txd_push = instance!(wire_default(&Type::UInt(1), false));
    let slot_0_txd_pop = instance!(wire_default(&Type::UInt(1), false));
    let slot_1_txd_push = instance!(wire_default(&Type::UInt(1), false));
    let slot_1_txd_pop = instance!(wire_default(&Type::UInt(1), false));

    let slot_0_rxd_push = instance!(wire_default(&Type::UInt(1), false));
    let slot_0_rxd_pop = instance!(wire_default(&Type::UInt(1), false));
    let slot_1_rxd_push = instance!(wire_default(&Type::UInt(1), false));
    let slot_1_rxd_pop = instance!(wire_default(&Type::UInt(1), false));

    let commit_cmd = always!(
      () {
        let hella_cmd = hella_cmd_queue.pop();
        let tag_addr = HellaCacheCmdBundle::from(hella_cmd.clone()).addr;
        let cmd_is_read = HellaCacheCmdBundle::from(hella_cmd.clone()).cmd.eq(0.uint(5));
        hella_slave.cmd_to_bus(hella_cmd);
        if_!(
          (cmd_is_read) {
            if_!(
              newer_slot.read().eq(1.uint(1)) {
                // next slot is 0
                // slot_0_txd.write(true.uint(1));
                slot_0_txd_push.write(true.uint(1));
                slot_0_tag.write(&tag_addr);
                newer_slot.write(0.uint(1));
              } else {
                // next slot is 1
                slot_1_txd_push.write(true.uint(1));
                slot_1_tag.write(&tag_addr);
                newer_slot.write(1.uint(1));
              }
            )
          }
        );
      }
    );

    let resp_from_bus = method! {
      // ready signal not used! should promise this by design
      (io.hella_resp_bus) {
        let hella_resp = HellaCacheRespBundle::from(io.hella_resp_bus);
        let tag_addr = hella_resp.addr;
        let cmd_is_read = hella_resp.cmd.eq(0.uint(5));

        if_!(
          (cmd_is_read) {
            // check which slot is used
            if_! (slot_0_tag.read().eq(&tag_addr) {
                // slot_0_rxd.write(true.uint(1));
                slot_0_rxd_push.write(true.uint(1));
                slot_0.write(&hella_resp.data);
              } else {
                if_! (slot_1_tag.read().eq(&tag_addr) {
                  // slot_1_rxd.write(true.uint(1));
                  slot_1_rxd_push.write(true.uint(1));
                  slot_1.write(hella_resp.data);
                });
              }
            );
          }
        );
      }
    };

    let slot_0_can_collect = instance!(stl::wire(&Type::UInt(1)));
    let slot_1_can_collect = instance!(stl::wire(&Type::UInt(1)));
    let slot_0_can_collect_logic = always! {
      () {
        let slot0_ready = slot_0_txd.read().eq(1.uint(1)) & slot_0_rxd.read().eq(1.uint(1));
        let is_earlier = slot_1_txd.read().eq(0.uint(1)) | (slot_1_txd.read().eq(1.uint(1)) & newer_slot.read().eq(1.uint(1)));
        let can_collect = slot0_ready & is_earlier;
        slot_0_can_collect.write(can_collect);
      }
    };
    let slot_1_can_collect_logic = always! {
      () {
        let slot1_ready = slot_1_txd.read().eq(1.uint(1)) & slot_1_rxd.read().eq(1.uint(1));
        let is_earlier = slot_0_txd.read().eq(0.uint(1)) | (slot_0_txd.read().eq(1.uint(1)) & newer_slot.read().eq(0.uint(1)));
        let can_collect = slot1_ready & is_earlier;
        slot_1_can_collect.write(can_collect);
      }
    };

    let resp_to_user = method! {
      [slot_0_can_collect.read() | slot_1_can_collect.read()]
      () -> (io.user_resp_bus) {
        let retval = if_! (slot_0_can_collect.read() {
          // slot_0_txd.write(false.uint(1));
          slot_0_txd_pop.write(true.uint(1));
          // slot_0_rxd.write(false.uint(1));
          slot_0_rxd_pop.write(true.uint(1));
          ret!(UserMemoryRespBundle {
            data: slot_0.read(),
            tag: slot_0_tag.read()
          }.create());
        } else {
          // slot_1_txd.write(false.uint(1));
          slot_1_txd_pop.write(true.uint(1));
          slot_1_rxd_pop.write(true.uint(1));
          if_! (slot_1_can_collect.read() {
            ret!(UserMemoryRespBundle {
              data: slot_1.read(),
              tag: slot_1_tag.read()
            }.create());
          });
        });
        ret!(var!(retval));
      }
    };

    let update_txd = always! {
      () {
        slot_0_txd.write(slot_0_txd_push.mux(1.uint(1), slot_0_txd_pop.mux(0.uint(1), slot_0_txd.read())));
        slot_1_txd.write(slot_1_txd_push.mux(1.uint(1), slot_1_txd_pop.mux(0.uint(1), slot_1_txd.read())));
        slot_0_rxd.write(slot_0_rxd_push.mux(1.uint(1), slot_0_rxd_pop.mux(0.uint(1), slot_0_rxd.read())));
        slot_1_rxd.write(slot_1_rxd_push.mux(1.uint(1), slot_1_rxd_pop.mux(0.uint(1), slot_1_rxd.read())));
      }
    };

    schedule!(resp_from_bus, commit_cmd);
    schedule!(resp_from_bus, resp_to_user);
}

itfc_declare! {
    param RoCCRESP_T;
    struct RoccVMaster {
        resp_bus: input param RoCCRESP_T
    };
    method resp_to_bus(resp_bus);
}

#[module]
fn mk_rocc_vmaster() -> RoccVMaster {
    let rocc_resp_t: Type = RoccResp::new().into();
    let io = io! {RoCCRESP_T: rocc_resp_t.clone()};
    let resp_to_bus = method! {
        (io.resp_bus) {
            // dummy action
        }
    };
}

itfc_declare!(
    param RoCCCMD_T;
    param RoCCRESP_T;
    pub(crate) struct RoCCAdapter {
        rocc_cmd_bus: input param RoCCCMD_T,
        // rocc_cmd_user: output param RoCCCMD_T,
        rocc_resp_user: input param RoCCRESP_T
    };
    method cmd_from_bus(rocc_cmd_bus);
    // method cmd_to_user() -> (rocc_cmd_user);
    method resp_from_user(rocc_resp_user);
);

#[module("synthesis": "true")]
fn mk_rocc_adapter(rocc_master: &RoccVMaster, opcodes: Vec<u32>) -> RoCCAdapter {
    let rocc_cmd_t: Type = RoccCmd::new().into();
    let rocc_resp_t: Type = RoccResp::new().into();
    let io = io! {
      RoCCCMD_T: rocc_cmd_t.clone(),
      RoCCRESP_T: rocc_resp_t.clone()
    };
    set_name!("RoCCAdapter".to_string());

    let rocc_master = friend!(rocc_master);

    let mut queues: IndexMap<u32, stl::FIFO> = IndexMap::new();
    for opcode in opcodes {
        let rocc_cmd_queue =
            named_instance!(format!("rocc_cmd_queue_{}", opcode);stl::fifo1_push(&rocc_cmd_t));
        let cmd = output!(format!("rocc_cmd_user_{}", opcode), rocc_cmd_t.clone());

        let cmd_to_user = named_method! {
          format!("cmd_to_user_{}", opcode);
          () -> (cmd) {
              ret!(rocc_cmd_queue.deq());
          }
        };

        queues.insert(opcode, rocc_cmd_queue);
    }

    let cmd_from_bus = method! {
      (io.rocc_cmd_bus) {
        let opc = RoccCmdBundle::from(io.rocc_cmd_bus.clone()).opcode;
        for (opcode, queue) in queues.iter() {
          if_!(
            opc.clone().eq(opcode.uint(7)) {
              // if the opcode matches, enqueue the command
              queue.enq(io.rocc_cmd_bus.clone());
            }
          )
        }
      }
    };

    let rocc_resp_queue = instance!(stl::fifo1_push(&rocc_resp_t));
    let resp_from_user = method! {
        (io.rocc_resp_user) {
            rocc_resp_queue.enq(io.rocc_resp_user);
        }
    };

    let commit = always!(
      () {
        rocc_master.resp_to_bus(rocc_resp_queue.deq());
      }
    );
}

itfc_declare!(
    pub struct RoCCModule {};
);

#[module("synthesis": "true")]
pub fn wire_default(t: &Type, val: impl CmtLit) -> stl::Wire {
    let io = io! {T: t};
    let inner = instance!(stl::wire(t));

    let read = method! {
      () -> (io.out) {
        ret!(inner.read());
      }
    };
    let write = method! {
      (io.in_) {
        inner.write(io.in_);
      }
    };
    let default = always! { () { inner.write(val.lit(t)); } };

    method_rel!(write C write);
    method_rel!(write C default);
    method_rel!(read CF read);
    schedule!(write, default, read);
}

itfc_declare!(
    param T;
    pub struct Buffer{
        din: input param T,
        dout: output param T
    };
    method push(din);
    method pop() -> (dout);
);

#[module("synthesis": "true")]
pub fn mk_stage_buffer(t: &Type) -> Buffer {
    // deq
    // let t = &Type::UInt(32);
    let io = io! {T: t};
    let t_bool = &Type::UInt(1);
    distinguisher!(&format!("stage_buffer"));

    let r = instance!(stl::reg(t));
    let vvvvv = instance!(stl::reg_init(t_bool, false));

    let want_push = instance!(wire_default(t_bool, false));
    let want_pop = instance!(wire_default(t_bool, false));
    let push_value = instance!(stl::wire(t));

    let pop = method! {
        [vvvvv.read()]
        () -> (io.dout) {
          want_pop.write(true);
          ret!(r.read())
        }
    };
    let push = method! {
        [!vvvvv.read() | want_pop.read()]
        (io.din) {
            want_push.write(true);
            push_value.write(io.din);
        }
    };
    let update = always! {
        () {
            let enq = want_push.read();
            if_! {
                (enq) {
                  r.write(push_value.read());
                  vvvvv.write(true);
                } else {
                    if_! {
                        want_pop.read() {
                          vvvvv.write(false);
                        }
                    }
                }
            }
        }
    };
}

#[module("synthesis": "true")]
pub fn mk_bypass_buffer(t: &Type) -> Buffer {
    // deq
    // let t = &Type::UInt(32);
    let io = io! {T: t};
    let t_bool = &Type::UInt(1);
    distinguisher!(&format!("bypass_buffer"));

    let r = instance!(stl::reg(t));
    let v = instance!(stl::reg_init(t_bool, false));

    let want_push = instance!(wire_default(t_bool, false));
    let want_pop = instance!(wire_default(t_bool, false));
    let push_value = instance!(stl::wire(t));
    let enq = instance!(wire_default(t_bool, false));

    let pop = method! {
        [want_push.read() | v.read()]
        () -> (io.dout) {
            want_pop.write(true);
            ret!(v.read().mux(r.read(), push_value.read()));
        }
    };
    let push = method! {
        [!v.read()]
        (io.din) {
            enq.write(1.uint(1));
            want_push.write(true);
            push_value.write(io.din);
        }
    };
    let update = always! {
        () {
            let deq = want_pop.read();
            if_! {
                (deq) {
                    v.write(false);
                } else {
                    if_! {
                        enq.read() {
                            r.write(push_value.read());
                            v.write(true);
                        }
                    }
                }
            }
        }
    };
}

#[module("synthesis": "true")]
fn mk_bypass_register(t: &Type) -> stl::Reg {
    let io = io! {T: t};
    let wr_wire = instance!(wire_default(&Type::UInt(1), 0));
    let dt_w = instance!(stl::wire(t));
    let dt_r = instance!(stl::reg(t));

    let read = method! {
      () -> (io.out) {
        let r = wr_wire.read().mux(dt_w.read(), dt_r.read());
        ret!(r);
      }
    };

    let write = method! {
      (io.in_) {
        dt_w.write(&io.in_);
        dt_r.write(&io.in_);
        wr_wire.write(1.uint(1));
      }
    };

    schedule!(write, read);
}

pub struct RoCCBackend {
    pub(crate) rocc_translator: RoCCAdapter,
    pub(crate) mem_translator: HellaAdapter,

    pub(crate) rs: Vec<stl::Reg>,
    pub(crate) rd_id: stl::Reg, // Bypass Register
    pub(crate) gpr_written: RefCell<bool>,
}

impl RoCCBackend {
    #[gen_fn]
    fn new(opcode: Vec<u32>) -> Self {
        // -------------------- RoCC Interface --------------------------------
        let rocc_master_virtual = itfc!("rocc".to_string(); mk_rocc_vmaster());
        let rocc_master_friend = friend!(&rocc_master_virtual);

        let roccitfc =
            instance!("roccitfc".to_string(); mk_rocc_adapter(&rocc_master_friend, opcode));

        let rocc_cmd_t: Type = RoccCmd::new().into();
        let rocc_cmd = input!("rocc_cmd".to_string(), rocc_cmd_t);

        let rocc_top_cmd = named_method! {
          "rocc_cmd".to_string();
          (rocc_cmd) {
            roccitfc.cmd_from_bus(rocc_cmd);
          }
        };

        // -------------------- HellaCache Interface ---------------------------
        let hella_slave_virual = itfc!("hella".to_string(); mk_hella_vslave());
        let hella_slave_friend = friend!(&hella_slave_virual);

        let hella_resp_t: Type = HellaCacheResp::new().into();
        let hella_resp = input!("hella_resp".to_string(), hella_resp_t);

        let hellaitfc = instance!("hellaitfc".to_string(); mk_hella_adapter(&hella_slave_friend));
        let hella_top_resp = named_method! {
          "hella_resp".to_string();
          (hella_resp) {
              hellaitfc.resp_from_bus(hella_resp);
          }
        };

        let rs1 = instance!("__backend_rs1".to_string(); mk_bypass_register(&Type::UInt(32)));
        let rs2 = instance!("__backend_rs2".to_string(); mk_bypass_register(&Type::UInt(32)));

        let rd_id = instance!("__backend_rd_id".to_string(); mk_bypass_register(&Type::UInt(5)));

        RoCCBackend {
            rocc_translator: roccitfc,
            mem_translator: hellaitfc,
            rs: vec![rs1, rs2],
            rd_id,
            gpr_written: RefCell::new(false),
        }
    }

    fn check_riscv_xlen(&self) -> usize {
        32
    }

    fn request_instruction(&self, opcode: u32) -> Var {
        let bd = self.rocc_translator.__ctx_view.invoke(
            format!("cmd_to_user_{}", opcode),
            vec![],
            1,
            None,
        );
        let inst = RoccCmdBundle::from(bd[0].clone());

        self.rs[0].write(inst.rs1data);
        self.rs[1].write(inst.rs2data);
        self.rd_id.write(&inst.rd);

        let inst_32b = inst.funct.cat(
            inst.rs2.cat(
                inst.rs1.cat(
                    inst.xd
                        .cat(inst.xs1.cat(inst.xs2.cat(inst.rd.cat(inst.opcode)))),
                ),
            ),
        );

        return inst_32b;
    }

    fn check_num_rs(&self) -> usize {
        2
    }

    fn read_gpr(&self, reg_id: u32) -> Var {
        if reg_id == 1 {
            self.rs[0].read()
        } else if reg_id == 2 {
            self.rs[1].read()
        } else {
            panic!("Invalid read from register id: {reg_id}");
        }
    }

    fn check_num_rd(&self) -> usize {
        1
    }

    fn write_gpr(&self, reg_id: u32, var: &Var) {
        self.gpr_written.replace(true);
        if reg_id == 1 {
            self.rocc_translator.resp_from_user(
                RoccRespBundle {
                    rd: self.rd_id.read(),
                    rddata: var.clone(),
                }
                .create(),
            );
        } else {
            panic!("Invalid write to register id: {reg_id}");
        }
    }

    // In RoCC, fired instructions will never be killed.
    fn fetch_kill_status(&self) -> Var {
        false.uint(1)
    }

    fn decode_instruction(&self, instn: &Var) -> RiscvRtypeInstnBundle {
        let funct7 = instn.bits(31, 25);
        let rs2 = instn.bits(24, 20);
        let rs1 = instn.bits(19, 15);
        let funct3 = instn.bits(14, 12);
        let rd = instn.bits(11, 7);
        let opcode = instn.bits(6, 0);

        RiscvRtypeInstnBundle {
            funct7,
            rs2,
            rs1,
            funct3,
            rd,
            opcode,
        }
    }

    fn read_memory_nonblocking(&self, _addr: &Var) {
        self.mem_translator.cmd_from_user(
            UserMemoryCmdBundle {
                addr: _addr.clone(),
                cmd: MemoryOP::MEMREAD.lit(),
                data: 0.uint(32),
                size: 2.uint(2),
                mask: 0.uint(4),
                tag: _addr & 0x3f.uint(8), // Only last 6 bit is valid
            }
            .create(),
        );
    }

    fn fetch_read_memory_response(&self) -> Var {
        UserMemoryRespBundle::from(self.mem_translator.resp_to_user()).data
    }

    // Mask is not working here!
    fn write_memory_nonblocking(&self, _addr: &Var, _data: &Var, _mask: &Var) {
        self.mem_translator.cmd_from_user(
            UserMemoryCmdBundle {
                addr: _addr.clone(),
                cmd: MemoryOP::MEMWRITE.lit(),
                data: _data.clone(),
                size: 2.uint(2),
                mask: 0.uint(4),
                tag: _addr & 0x3f.uint(8), // Only 6 bit is valid
            }
            .create(),
        );
    }
}

/// Helper structure to track Var values during code generation
struct OpValueTracker {
    /// Maps (bb_idx, op_idx) to the Var value
    values: HashMap<(usize, usize), Var>,
}

impl OpValueTracker {
    fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    fn set(&mut self, bb_idx: usize, op_idx: usize, var: Var) {
        self.values.insert((bb_idx, op_idx), var);
    }

    fn get(&self, bb_idx: usize, op_idx: usize) -> Option<&Var> {
        self.values.get(&(bb_idx, op_idx))
    }
}

#[module]
pub fn make_rocc_module(cdfg: &mut CDFG, schedules: &Vec<Schedule>) -> RoCCModule {
    let io = io! {};
    set_name!("clay_top_rocc".to_string());

    let backend = Box::new(generate!(RoCCBackend::new(vec![0b0001011])));

    // Step 1: Regularize CDFG with schedules
    let (reg_bbs, mut type_ctx) = regularize_cdfg(cdfg, schedules);

    // for (idx, reg_bb) in reg_bbs.iter().enumerate() {
    //     // println!("Regularized BB {}:", reg_bb.bb_idx);
    //     // print ops and schedule
    //     match &schedules[idx] {
    //         Schedule::Sequential(s) => {
    //             println!("Schedule for BB {}: Sequential n_stages={}", idx, s.latency);
    //             for (op_idx, op) in reg_bb.ops.iter().enumerate() {
    //                 print!("\t@{:?}\t", s.schedule[op_idx]);
    //                 println!("Op {}: {:?}", op_idx, op);
    //             }
    //         }
    //         Schedule::Modulo(s) => {
    //             println!("Schedule for BB {}: Modulo II={}", idx, s.II);
    //             for (op_idx, op) in reg_bb.ops.iter().enumerate() {
    //                 print!("\t@{:?}\t", s.schedule[op_idx]);
    //                 println!("Op {}: {:?}", op_idx, op);
    //             }
    //         }
    //     }
    // }

    // Step 2: Allocate state registers for all variables
    // println!("State Registers:");

    let mut state_regs = HashMap::new();
    for var in &cdfg.vars {
        let ty = type_ctx.find_state_write_type(cdfg, var);
        // println!("\tVariable: {}, Type: {:?}", var, ty);
        let ty_cmtrs: cmt2::cmtrs::Type = ty.into(); // Convert to cmtrs::Type
        let reg = named_instance!(format!("state_{}", var); stl::reg(&ty_cmtrs));
        state_regs.insert(var.clone(), reg);
    }
    // println!("Allocated {} state registers.", state_regs.len());

    // Step 3: Allocate success and enable signals for each stage
    let mut success_map = HashMap::new();
    let mut enable_map = HashMap::new();

    for reg_bb in &reg_bbs {
        let bb_idx = reg_bb.bb_idx;
        match &reg_bb.schedule {
            Schedule::Modulo(modulo) => {
                for stage in 0..modulo.latency {
                    let key = (bb_idx, stage);
                    let inst = named_instance!(
                        format!("s_bb_{}_stage_{}", bb_idx, stage);
                        wire_default(&Type::UInt(1), false)
                    );
                    success_map.insert(key, inst);

                    let en_inst = named_instance!(
                        format!("v_bb_{}_stage_{}", bb_idx, stage);
                        stl::reg_init(&Type::UInt(1), false)
                    );
                    enable_map.insert(key, en_inst);
                }
            }
            Schedule::Sequential(seq) => {
                for stage in 0..seq.latency {
                    let key = (bb_idx, stage);
                    let inst = named_instance!(
                        format!("s_bb_{}_stage_{}", bb_idx, stage);
                        wire_default(&Type::UInt(1), false)
                    );
                    success_map.insert(key, inst);

                    let en_inst = named_instance!(
                        format!("v_bb_{}_stage_{}", bb_idx, stage);
                        stl::reg_init(&Type::UInt(1), false)
                    );
                    enable_map.insert(key, en_inst);
                }
            }
        }
    }

    // Step 4: Allocate buffers for regularized operations
    // bb, op, stage
    let mut buffer_instances: HashMap<(usize, usize, usize), Rc<dyn std::any::Any>> =
        HashMap::new();
    for reg_bb in &reg_bbs {
        let sched_stages = match &reg_bb.schedule {
            Schedule::Modulo(modulo) => modulo.latency,
            Schedule::Sequential(seq) => seq.latency,
        };
        for (op_idx, reg_op) in reg_bb.ops.iter().enumerate() {
            let cur_stage = match &reg_bb.schedule {
                Schedule::Modulo(modulo) => &modulo.schedule[op_idx],
                Schedule::Sequential(seq) => &seq.schedule[op_idx],
            }
            .stage()
            .unwrap();

            match &reg_op.buffer {
                BufferAlloc::StageBuffer(name, ty, max_stage) => {
                    let ty_cmtrs: cmt2::cmtrs::Type = ty.clone().into();

                    // need buffers
                    for stage in cur_stage..*max_stage {
                        let inst = named_instance!(name.clone(); mk_stage_buffer(&ty_cmtrs));
                        buffer_instances.insert((reg_bb.bb_idx, op_idx, stage + 1), Rc::new(inst));
                    }
                }
                BufferAlloc::Register(name, ty) => {
                    let ty_cmtrs: cmt2::cmtrs::Type = ty.clone().into();
                    let inst = Rc::new(named_instance!(name.clone(); stl::reg(&ty_cmtrs)));
                    for stage in (cur_stage + 1)..sched_stages {
                        buffer_instances.insert((reg_bb.bb_idx, op_idx, stage), inst.clone());
                    }
                }
                BufferAlloc::Inline => {}
            }
        }
    }

    // Step 5: Allocate condition wires for branch conditions
    let mut cond_wires = HashMap::new();
    for reg_bb in &reg_bbs {
        let bb = &cdfg.blocks[reg_bb.bb_idx];
        if bb.cond.is_some() {
            let n_stages = reg_bb.get_n_stages();
            let wire = named_instance!(
                format!("c_bb_{}_last", reg_bb.bb_idx);
                wire_default(&Type::UInt(1), false)
            );
            cond_wires.insert((reg_bb.bb_idx, n_stages - 1), wire);

            if let Schedule::Modulo(modulo) = &reg_bb.schedule {
                if modulo.II != n_stages {
                    let wire_ii = named_instance!(
                        format!("c_bb_{}_ii", reg_bb.bb_idx);
                        wire_default(&Type::UInt(1), false)
                    );
                    cond_wires.insert((reg_bb.bb_idx, modulo.II - 1), wire_ii);
                }
            }
        }
    }

    // Step 6: Generate stage rules for each basic block
    generate!(gen_all_stage_rules(
        &backend,
        cdfg,
        &reg_bbs,
        &state_regs,
        &buffer_instances,
        &success_map,
        &enable_map,
        &cond_wires,
    ));

    // Step 7: Generate update_valids rule
    generate!(gen_update_valids_rule_impl(
        cdfg,
        &reg_bbs,
        &success_map,
        &enable_map,
        &cond_wires,
    ));
}

// ============================================================================
// Code Generation Helper Functions
// ============================================================================

/// Translate a single operation into cmt2 code
#[gen_fn]
fn translate_operation(
    backend: &RoCCBackend,
    reg_op: &RegularizedOp,
    reg_op_idx: usize,
    bb: &BasicBlock,
    reg_bb: &RegularizedBB,
    state_regs: &HashMap<String, stl::Reg>,
    buffer_instances: &HashMap<(usize, usize, usize), Var>,
    input_vars: &HashMap<String, Var>,
    operand_cache: &mut HashMap<usize, Option<Var>>,
    bb_idx: usize,
    target_stage: usize,
) -> Option<Var> {
    if buffer_instances.contains_key(&(bb_idx, reg_op_idx, target_stage)) {
        let buffer_any = buffer_instances
            .get(&(bb_idx, reg_op_idx, target_stage))
            .unwrap();
        return Some(var!(buffer_any));
    }

    if operand_cache.contains_key(&reg_op_idx) {
        return operand_cache.get(&reg_op_idx).unwrap().clone();
    }

    let op = &reg_op.op;
    let var = match op {
        Op::Lit(typed_lit) => match &typed_lit.ty {
            cmt2::cmtir::Type::UInt(w) => {
                let val_u64 = u64::try_from(typed_lit.value()).unwrap_or(0);
                Some(val_u64.uint(*w))
            }
            _ => panic!("Unsupported literal type in RoCC backend"),
        },
        Op::ExtRef(name) => {
            if input_vars.contains_key(name) {
                Some(input_vars.get(name).unwrap().clone())
            } else {
                panic!("Input variable {} not found in RoCC backend", name);
            }
        }
        Op::Unary(unary_op, op_idx) => {
            let var = generate!(translate_operation(
                backend,
                &reg_bb.ops[op_idx.0],
                op_idx.0,
                bb,
                reg_bb,
                state_regs,
                buffer_instances,
                input_vars,
                operand_cache,
                bb_idx,
                reg_op.stage,
            ))
            .unwrap();

            use crate::ast::UnaryOp;
            Some(match unary_op {
                UnaryOp::Not | UnaryOp::BitNot => !var,
                UnaryOp::Neg => var.neg(),
                UnaryOp::SignedCast | UnaryOp::UnsignedCast => var,
            })
        }
        Op::Binary(binary_op, lhs_idx, rhs_idx) => {
            let lhs = generate!(translate_operation(
                backend,
                &reg_bb.ops[lhs_idx.0],
                lhs_idx.0,
                bb,
                reg_bb,
                state_regs,
                buffer_instances,
                input_vars,
                operand_cache,
                bb_idx,
                reg_op.stage,
            ))
            .unwrap();
            let rhs = generate!(translate_operation(
                backend,
                &reg_bb.ops[rhs_idx.0],
                rhs_idx.0,
                bb,
                reg_bb,
                state_regs,
                buffer_instances,
                input_vars,
                operand_cache,
                bb_idx,
                reg_op.stage
            ))
            .unwrap();

            use crate::ast::BinaryOp;
            Some(match binary_op {
                BinaryOp::Add => lhs + rhs,
                BinaryOp::Sub => lhs - rhs,
                BinaryOp::Mul => lhs * rhs,
                BinaryOp::Div => lhs / rhs,
                BinaryOp::Rem => lhs % rhs,
                BinaryOp::And | BinaryOp::BitAnd => lhs & rhs,
                BinaryOp::Or | BinaryOp::BitOr => lhs | rhs,
                BinaryOp::BitXor => lhs ^ rhs,
                BinaryOp::LShift => {
                    // Check if rhs is a literal for static shift
                    let rhs_op = &reg_bb.ops.get(rhs_idx.0).unwrap().op;
                    if let Op::Lit(lit) = rhs_op {
                        let shift_amt = u32::try_from(lit.value()).unwrap_or(0);
                        lhs.shl(shift_amt)
                    } else {
                        // Dynamic shift
                        lhs.dshl(rhs)
                    }
                }
                BinaryOp::RShift => {
                    // Check if rhs is a literal for static shift
                    let rhs_op = &reg_bb.ops.get(rhs_idx.0).unwrap().op;
                    if let Op::Lit(lit) = rhs_op {
                        let shift_amt = u32::try_from(lit.value()).unwrap_or(0);
                        lhs.shr(shift_amt)
                    } else {
                        // Dynamic shift
                        lhs.dshr(rhs)
                    }
                }
                BinaryOp::Lt => lhs.lt(rhs),
                BinaryOp::Le => lhs.le(rhs),
                BinaryOp::Gt => lhs.gt(rhs),
                BinaryOp::Ge => lhs.ge(rhs),
                BinaryOp::Eq => lhs.eq(rhs),
                BinaryOp::Ne => lhs.ne(rhs),
            })
        }
        Op::Index(op_idx, op_idx1) => {
            let expr = generate!(translate_operation(
                backend,
                &reg_bb.ops[op_idx.0],
                op_idx.0,
                bb,
                reg_bb,
                state_regs,
                buffer_instances,
                input_vars,
                operand_cache,
                bb_idx,
                reg_op.stage
            ))
            .unwrap();
            let index = generate!(translate_operation(
                backend,
                &reg_bb.ops[op_idx1.0],
                op_idx1.0,
                bb,
                reg_bb,
                state_regs,
                buffer_instances,
                input_vars,
                operand_cache,
                bb_idx,
                reg_op.stage
            ))
            .unwrap();
            // expr.dyn_idx(&index)
            panic!("Dynamic indexing not supported in RoCC backend")
        }
        Op::Slice(op_idx, hi_idx, lo_idx) => {
            // let expr = get_operand(expr_idx.0)?;
            let expr = generate!(translate_operation(
                backend,
                &reg_bb.ops[op_idx.0],
                op_idx.0,
                bb,
                reg_bb,
                state_regs,
                buffer_instances,
                input_vars,
                operand_cache,
                bb_idx,
                reg_op.stage
            ))
            .unwrap();

            // Get hi and lo literals
            let hi_op = &reg_bb.ops.get(hi_idx.0).unwrap().op;
            let lo_op = &reg_bb.ops.get(lo_idx.0).unwrap().op;

            let mut res = None;
            {
                if let Op::Lit(hi_lit) = hi_op {
                    if let Op::Lit(lo_lit) = lo_op {
                        let hi = u32::try_from(hi_lit.value()).unwrap_or(31);
                        let lo = u32::try_from(lo_lit.value()).unwrap_or(0);
                        // Use bits() method which exists in Var
                        res = Some(expr.bits(hi, lo));
                    }
                }
            }
            // res.expect("non-constant slice range")
            res
        }
        Op::Concat(operands) => {
            if operands.is_empty() {
                panic!("empty concat operands");
            }

            let mut result = generate!(translate_operation(
                backend,
                &reg_bb.ops[operands[0].0],
                operands[0].0,
                bb,
                reg_bb,
                state_regs,
                buffer_instances,
                input_vars,
                operand_cache,
                bb_idx,
                reg_op.stage
            ))
            .unwrap();

            for op_idx in operands.iter().skip(1) {
                // let val = get_operand(op_idx.0)?;
                let val = generate!(translate_operation(
                    backend,
                    &reg_bb.ops[op_idx.0],
                    op_idx.0,
                    bb,
                    reg_bb,
                    state_regs,
                    buffer_instances,
                    input_vars,
                    operand_cache,
                    bb_idx,
                    reg_op.stage
                ))
                .unwrap();
                result = result.cat(val);
            }
            Some(result)
        }
        Op::Match(op_idx, vec) => {
            let expr = generate!(translate_operation(
                backend,
                &reg_bb.ops[op_idx.0],
                op_idx.0,
                bb,
                reg_bb,
                state_regs,
                buffer_instances,
                input_vars,
                operand_cache,
                bb_idx,
                reg_op.stage
            ))
            .unwrap();
            let pairs = vec.iter().map(|(k, v)| {
                // let k = generate!(self.eval_expr(ctx, *k))?;
                let k = generate!(translate_operation(
                    backend,
                    &reg_bb.ops[k.0],
                    k.0,
                    bb,
                    reg_bb,
                    state_regs,
                    buffer_instances,
                    input_vars,
                    operand_cache,
                    bb_idx,
                    reg_op.stage
                ))
                .unwrap();
                let v = generate!(translate_operation(
                    backend,
                    &reg_bb.ops[v.0],
                    v.0,
                    bb,
                    reg_bb,
                    state_regs,
                    buffer_instances,
                    input_vars,
                    operand_cache,
                    bb_idx,
                    reg_op.stage
                ))
                .unwrap();

                (var!((&expr).eq(k)), v)
            });
            let value = pairs
                .into_iter()
                .fold(None, |acc, (cond, value)| match acc {
                    Some(acc) => Some(cond.mux(value, acc)),
                    None => Some(value),
                })
                .unwrap();
            Some(var!(value))
        }
        Op::IfElse(op_idx, op_idx1, op_idx2) => {
            let cond = generate!(translate_operation(
                backend,
                &reg_bb.ops[op_idx.0],
                op_idx.0,
                bb,
                reg_bb,
                state_regs,
                buffer_instances,
                input_vars,
                operand_cache,
                bb_idx,
                reg_op.stage
            ))
            .unwrap();
            let then_val = generate!(translate_operation(
                backend,
                &reg_bb.ops[op_idx1.0],
                op_idx1.0,
                bb,
                reg_bb,
                state_regs,
                buffer_instances,
                input_vars,
                operand_cache,
                bb_idx,
                reg_op.stage
            ))
            .unwrap();
            let else_val = generate!(translate_operation(
                backend,
                &reg_bb.ops[op_idx2.0],
                op_idx2.0,
                bb,
                reg_bb,
                state_regs,
                buffer_instances,
                input_vars,
                operand_cache,
                bb_idx,
                reg_op.stage
            ))
            .unwrap();
            Some(cond.mux(then_val, else_val))
        }
        Op::RegReadReq(op_idx) => todo!(),
        Op::RegReadResp(op_idx) => todo!(),
        Op::RegWriteReq(op_idx, op_idx1) => todo!(),
        Op::MemReadReq(op_idx) => {
            let addr = generate!(translate_operation(
                backend,
                &reg_bb.ops[op_idx.0],
                op_idx.0,
                bb,
                reg_bb,
                state_regs,
                buffer_instances,
                input_vars,
                operand_cache,
                bb_idx,
                reg_op.stage
            ))
            .unwrap();
            backend.read_memory_nonblocking(&addr);
            None
        }
        Op::MemReadResp(op_idx) => Some(backend.fetch_read_memory_response()),
        Op::MemWriteReq(op_idx, op_idx1, op_idx2) => {
            let addr = generate!(translate_operation(
                backend,
                &reg_bb.ops[op_idx.0],
                op_idx.0,
                bb,
                reg_bb,
                state_regs,
                buffer_instances,
                input_vars,
                operand_cache,
                bb_idx,
                reg_op.stage
            ))
            .unwrap();
            let data = generate!(translate_operation(
                backend,
                &reg_bb.ops[op_idx1.0],
                op_idx1.0,
                bb,
                reg_bb,
                state_regs,
                buffer_instances,
                input_vars,
                operand_cache,
                bb_idx,
                reg_op.stage
            ))
            .unwrap();
            let mask = generate!(translate_operation(
                backend,
                &reg_bb.ops[op_idx2.0],
                op_idx2.0,
                bb,
                reg_bb,
                state_regs,
                buffer_instances,
                input_vars,
                operand_cache,
                bb_idx,
                reg_op.stage
            ))
            .unwrap();
            backend.write_memory_nonblocking(&addr, &data, &mask);
            None
        }
        Op::StateRead(s) => {
            if let Some(reg) = state_regs.get(s) {
                Some(reg.read())
            } else {
                panic!("State register {} not found in RoCC backend", s);
            }
        }
        Op::StateWrite(s, op_idx) => {
            let val = generate!(translate_operation(
                backend,
                &reg_bb.ops[op_idx.0],
                op_idx.0,
                bb,
                reg_bb,
                state_regs,
                buffer_instances,
                input_vars,
                operand_cache,
                bb_idx,
                reg_op.stage
            ))
            .unwrap();
            if &s[..] == "rd" {
                backend.write_gpr(1, &val);
            } else {
                if let Some(reg) = state_regs.get(s) {
                    reg.write(val.clone());
                }
            }
            None
        }
        Op::PcUpdate(op_idx, op_idx1) => todo!(),
    };
    operand_cache.insert(reg_op_idx, var.clone());

    return var;
}

/// Generate all stage rules for all basic blocks
#[gen_fn]
fn gen_all_stage_rules(
    backend: &RoCCBackend,
    cdfg: &CDFG,
    reg_bbs: &[RegularizedBB],
    state_regs: &HashMap<String, stl::Reg>,
    buffer_instances: &HashMap<(usize, usize, usize), Rc<dyn std::any::Any>>,
    success_map: &HashMap<(usize, usize), stl::Wire>,
    enable_map: &HashMap<(usize, usize), stl::Reg>,
    cond_wires: &HashMap<(usize, usize), stl::Wire>,
) {
    for reg_bb in reg_bbs {
        let bb = &cdfg.blocks[reg_bb.bb_idx];
        let bb_idx = reg_bb.bb_idx;

        let n_stages = reg_bb.get_n_stages();
        let schedule = match &reg_bb.schedule {
            Schedule::Sequential(seq) => &seq.schedule,
            Schedule::Modulo(modulo) => &modulo.schedule,
        };

        let mut stages_ops: HashMap<usize, Vec<(usize, &RegularizedOp)>> = HashMap::new();
        for (op_idx, reg_op) in reg_bb.ops.iter().enumerate() {
            stages_ops
                .entry(reg_op.stage)
                .or_insert_with(Vec::new)
                .push((op_idx, reg_op));
        }

        let mut operand_cache = HashMap::new();
        // OpIdx => Var

        let mut buffers = HashMap::new();

        let mut rules = vec![];
        for stage in 0..n_stages {
            let ops = stages_ops.get(&stage).map(|v| v.as_slice()).unwrap_or(&[]);
            let success_wire = &success_map[&(bb_idx, stage)];
            let enable_reg = &enable_map[&(bb_idx, stage)];

            let rule_name = format!("bb_{}_stage_{}", bb_idx, stage);
            let rule = if stage == 0 && bb_idx == 0 {
                let rule = named_always! {
                  rule_name;
                  () {
                    // Handle BB entry condition if present
                    let mut input_vars = HashMap::new();
                      let inst = &backend.request_instruction(0b0001011);
                      let cmd = backend.decode_instruction(inst);
                      let rs = [backend.read_gpr(1), backend.read_gpr(2)];

                      for (i, input) in bb.inputs.iter().enumerate() {
                        let v = match &input[..] {
                          "rs1" => rs[0].clone(),
                          "rs2" => rs[1].clone(),
                          "funct7" => cmd.funct7.clone(),
                          "funct3" => cmd.funct3.clone(),
                          "opcode" => cmd.opcode.clone(),
                          _ => panic!("Unknown input {}", input),
                        };
                        input_vars.insert(input.clone(), v);
                      }

                    for (k,v) in buffer_instances.iter() {
                      if k.0 == bb_idx && k.2 == stage {
                        if let Some(buffer_var) = v.downcast_ref::<Buffer>() {
                          let v = buffer_var.pop();
                          buffers.insert(k.clone(), v.clone());
                          if buffer_instances.contains_key(&(bb_idx, k.1, stage +1)) {
                              let next_buffer_any = buffer_instances
                                  .get(&(bb_idx, k.1, stage +1))
                                  .unwrap();
                              if let Some(next_buffer_var) = next_buffer_any.downcast_ref::<Buffer>() {
                                println!("push {} {} {}", bb_idx, k.1, stage+1);
                                  next_buffer_var.push(v.clone());
                              }
                          }
                        }
                        if let Some(reg_var) = v.downcast_ref::<stl::Reg>() {
                          buffers.insert(k.clone(), var!(reg_var.read()));
                        }
                      }
                    }

                    // Translate all operations in this stage
                    for (op_idx, reg_op) in ops {
                        generate!(translate_operation(
                            backend,
                            reg_op,
                            *op_idx,
                            bb,
                            reg_bb,
                            state_regs,
                            &buffers,
                            &input_vars,
                            &mut operand_cache,
                            bb_idx,
                            stage
                        ));

                        match reg_op.buffer {
                            BufferAlloc::StageBuffer(_, _, _) => {
                                println!("push2 {} {} {}", bb_idx, *op_idx, stage+1);
                              buffer_instances[&(bb_idx, *op_idx, stage+1)].downcast_ref::<Buffer>().unwrap().push(
                                operand_cache.get(op_idx).unwrap().as_ref().unwrap().clone()
                              );
                            },
                            BufferAlloc::Register(_, _) => {
                              buffer_instances[&(bb_idx, *op_idx, stage+1)].downcast_ref::<stl::Reg>().unwrap().write(
                                operand_cache.get(op_idx).unwrap().as_ref().unwrap().clone()
                              );
                            }
                            BufferAlloc::Inline => {}
                        }
                    }

                    if bb.cond.is_some() {
                      if let Some(cond_wire) = cond_wires.get(&(bb_idx, stage)) {
                        // cond_wire.write(true);
                        let var = generate!(translate_operation(
                            backend,
                            &reg_bb.ops[bb.cond.as_ref().unwrap().0],
                            bb.cond.as_ref().unwrap().0,
                            bb,
                            reg_bb,
                            state_regs,
                            &buffers,
                            &input_vars,
                            &mut operand_cache,
                            bb_idx,
                            stage
                        ));
                        cond_wire.write(
                          var.unwrap()
                        );
                      }
                    }


                    if stage == n_stages - 1 && bb_idx == reg_bbs.len() - 1 {
                      // last
                      if !*backend.gpr_written.borrow() {
                        backend.write_gpr(1, &0.uint(32));
                      }
                    }
                    success_wire.write(true);
                  }
                };
                rule
            } else {
                let rule = named_always! {
                  rule_name;
                  [enable_reg.read()] () {
                    // Handle BB entry condition if present
                    let input_vars = HashMap::new();

                    for (k,v) in buffer_instances.iter() {
                      if k.0 == bb_idx && k.2 == stage {
                        if let Some(buffer_var) = v.downcast_ref::<Buffer>() {
                          let staged = buffer_var.pop();

                          buffers.insert(k.clone(), staged.clone());
                          if buffer_instances.contains_key(&(bb_idx, k.1, stage +1)) {
                              let next_buffer_any = buffer_instances
                                  .get(&(bb_idx, k.1, stage +1))
                                  .unwrap();
                              if let Some(next_buffer_var) = next_buffer_any.downcast_ref::<Buffer>() {
                                  next_buffer_var.push(staged);
                              }
                          }
                        }
                        if let Some(reg_var) = v.downcast_ref::<stl::Reg>() {
                          buffers.insert(k.clone(), var!(reg_var.read()));
                        }
                      }
                    }

                    // Translate all operations in this stage
                    for (op_idx, reg_op) in ops {
                        generate!(translate_operation(
                            backend,
                            reg_op,
                            *op_idx,
                            bb,
                            reg_bb,
                            state_regs,
                            &buffers,
                            &input_vars,
                            &mut operand_cache,
                            bb_idx,
                            stage
                        ));

                        match reg_op.buffer {
                            BufferAlloc::StageBuffer(_, _, _) => {
                              buffer_instances[&(bb_idx, *op_idx, stage+1)].downcast_ref::<Buffer>().unwrap().push(
                                operand_cache.get(op_idx).unwrap().as_ref().unwrap().clone()
                              );
                            },
                            BufferAlloc::Register(_, _) => {
                              buffer_instances[&(bb_idx, *op_idx, stage+1)].downcast_ref::<stl::Reg>().unwrap().write(
                                operand_cache.get(op_idx).unwrap().as_ref().unwrap().clone()
                              );
                            }
                            BufferAlloc::Inline => {}
                        }
                    }

                    if bb.cond.is_some() {
                      if let Some(cond_wire) = cond_wires.get(&(bb_idx, stage)) {
                        // cond_wire.write(true);
                        let var = generate!(translate_operation(
                            backend,
                            &reg_bb.ops[bb.cond.as_ref().unwrap().0],
                            bb.cond.as_ref().unwrap().0,
                            bb,
                            reg_bb,
                            state_regs,
                            &buffers,
                            &input_vars,
                            &mut operand_cache,
                            bb_idx,
                            stage
                        ));
                        cond_wire.write(var.unwrap());
                      }
                    }


                    if stage == n_stages - 1 && bb_idx == reg_bbs.len() - 1 {
                      // last
                      if !*backend.gpr_written.borrow() {
                        backend.write_gpr(1, &0.uint(32));
                      }
                    }
                    success_wire.write(true);
                  }
                };
                rule
            };

            rules.push(rule);
        }

        if matches!(&reg_bb.schedule, Schedule::Modulo(_)) {
            let rules: Vec<_> = rules.into_iter().rev().collect();
            schedule_raw!(&rules[..]);
        }
    }
}

/// Generate the update_valids rule
#[gen_fn]
fn gen_update_valids_rule_impl(
    cdfg: &CDFG,
    reg_bbs: &[RegularizedBB],
    success_map: &HashMap<(usize, usize), stl::Wire>,
    enable_map: &HashMap<(usize, usize), stl::Reg>,
    cond_wires: &HashMap<(usize, usize), stl::Wire>,
) {
    let _ = named_always! {
        format!("update_valids");
        () {
          let mut valids = HashMap::new();

          for (idx, reg_bb) in reg_bbs.iter().enumerate() {
              let bb_idx = reg_bb.bb_idx;
              let n_stages = reg_bb.get_n_stages();

              for stage in 0..n_stages {
                let enable_reg = enable_map.get(&(bb_idx, stage)).unwrap();
                let success_wire = success_map.get(&(bb_idx, stage)).unwrap();

                if bb_idx != 0 || stage != 0 {
                  let valid_expr = enable_reg.read() & !success_wire.read();
                  let k = (bb_idx, stage);
                  match valids.get(&k) {
                    Some(current_expr) => {
                      valids.insert(k, valid_expr | current_expr);
                    },
                    None => {
                      valids.insert(k, valid_expr);
                    }
                  }
                  continue;
                }

                if stage + 1 < n_stages {
                  let k = (bb_idx, stage + 1);
                  match valids.get(&k) {
                    Some(current_expr) => {
                      valids.insert(k, success_wire.read() | current_expr);
                    },
                    None => {
                      valids.insert(k, success_wire.read());
                    }
                  };
                }
              }

              // if reg_bb.
              // check cond
              // let cond_opt = cdfg.blocks[bb_idx].cond.as_ref();
              let bb = &cdfg.blocks[bb_idx];
              if let Some(cond_op_idx) = bb.cond.as_ref() {
                let last_stage = n_stages -1;
                let cond_wire = cond_wires.get(&(bb_idx, last_stage)).unwrap();
                let success_wire = success_map.get(&(bb_idx, last_stage)).unwrap();
                // On success of last stage, set cond_wire
                // let k = (bb_, last_stage);

                let true_bb = bb.true_branch.unwrap();
                let false_bb = bb.false_branch.unwrap();

                let k = (true_bb.0, 0);
                match valids.get(&k) {
                  Some(current_expr) => {
                    valids.insert(k, (cond_wire.read() & success_wire.read()) | current_expr);
                  },
                  None => {
                    valids.insert(k, cond_wire.read() & success_wire.read());
                  }
                }

                let k = (false_bb.0, 0);
                match valids.get(&k) {
                  Some(current_expr) => {
                    valids.insert(k, (!cond_wire.read() & success_wire.read()) | current_expr);
                  }
                  None => {
                    valids.insert(k, !cond_wire.read() & success_wire.read());
                  }
                }

                if let Schedule::Modulo(schedule) = &reg_bb.schedule {
                  if schedule.II != n_stages {
                    // Handle backedge
                    let k = (bb_idx, 0);

                    let cond_wire = cond_wires.get(&(bb_idx, schedule.II-1)).unwrap();
                    let success_wire = success_map.get(&(bb_idx, schedule.II-1)).unwrap();

                    match valids.get(&k) {
                      Some(current_expr) => {
                        valids.insert(k, (!cond_wire.read() & success_wire.read()) | current_expr);
                      }
                      None => {
                        valids.insert(k, !cond_wire.read() & success_wire.read());
                      }
                    }
                  }
                }
              }
          }

          for ((bb_idx, stage), expr) in valids.iter() {
            let enable_reg = enable_map.get(&(*bb_idx, *stage)).unwrap();
            // valids
            if *bb_idx == 0 && *stage == 0 {
              continue;
            }
            enable_reg.write(expr);
          }
        }
    };
}

#[module]
pub fn make_rocc_test_module0() -> RoCCModule {
    let io = io! {};
    set_name!("clay_top_rocc".to_string());

    let backend = Box::new(generate!(RoCCBackend::new(vec![0b0001011])));

    // translate BBs

    let n = instance!(stl::reg(&Type::UInt(32)));
    let i = instance!(stl::reg(&Type::UInt(32)));
    let d = instance!(stl::reg(&Type::UInt(32)));
    let s = instance!(stl::reg(&Type::UInt(32)));

    let s0 = instance!(wire_default(&Type::UInt(1), false));
    let s1 = instance!(wire_default(&Type::UInt(1), false));
    let s2 = instance!(wire_default(&Type::UInt(1), false));
    let s3 = instance!(wire_default(&Type::UInt(1), false));
    let s4 = instance!(wire_default(&Type::UInt(1), false));

    let v1 = instance!(stl::reg_init(&Type::UInt(1), false));
    let v2 = instance!(stl::reg_init(&Type::UInt(1), false));
    let v3 = instance!(stl::reg_init(&Type::UInt(1), false));
    let v4 = instance!(stl::reg_init(&Type::UInt(1), false));

    let c0 = instance!(wire_default(&Type::UInt(1), false));
    let c1 = instance!(wire_default(&Type::UInt(1), false));

    let t0 = instance!(mk_stage_buffer(&Type::UInt(1)));
    let t1 = instance!(mk_stage_buffer(&Type::UInt(32)));
    let t2 = instance!(mk_stage_buffer(&Type::UInt(32)));
    let t3 = instance!(mk_stage_buffer(&Type::UInt(32)));
    let t4 = instance!(mk_stage_buffer(&Type::UInt(32)));
    let t5 = instance!(mk_stage_buffer(&Type::UInt(1)));

    let _ = named_always! {
        format!("init_{}", 0b0001011);
        () {
            let inst = &backend.request_instruction(0b0001011);
            let cmd = backend.decode_instruction(inst);
            let rs = [backend.read_gpr(1), backend.read_gpr(2)];

            s.write(rs[0].clone());
            d.write(rs[1].clone());
            i.write(0u32);
            n.write(cmd.funct7.clone());

            c0.write(0.uint(32).lt(cmd.funct7));

            s0.write(true);
        }

    };

    let l1 = always! {
      [v1.read()] () {
        backend.read_memory_nonblocking(&(s.read()+4.uint(32)));

        d.write(d.read() + 4.uint(32));
        s.write(s.read() + 8.uint(32));
        i.write(i.read() + 1.uint(32));

        t0.push((i.read()+1.uint(32)).lt(n.read()));
        t1.push(s.read());
        t3.push(d.read());

        s1.write(true);
      }
    };

    let l2 = always! {
      [v2.read()] () {
        let resp = backend.fetch_read_memory_response();
        backend.read_memory_nonblocking(&(t1.pop()));
        t2.push(resp);
        t4.push(t3.pop());
        t5.push(t0.pop());

        s2.write(true);
      }
    };

    let l3 = always! {
      [v3.read()] () {
        let sum = t2.pop() + backend.fetch_read_memory_response();
        backend.write_memory_nonblocking(&t4.pop(), &sum, &0xf.uint(4));

        c1.write(t5.pop());

        s3.write(true);
      }
    };

    schedule!(l3, l2, l1);

    let e = always! {
      [v4.read()] () {
        backend.write_gpr(1, &0.uint(32));
        s4.write(true);
      }
    };

    let _ = named_always! {
      format!("update_valids");
      () {
        v1.write((s0.read() & c0.read()) | (s3.read() & c1.read()) | (v1.read() & !s1.read()));
        v2.write(s1.read() | (v2.read() & !s2.read()));
        v3.write(s2.read() | (v3.read() & !s3.read()));
        v4.write((s3.read() & !c1.read()) | (v4.read() & !s4.read()));
      }
    };
}
