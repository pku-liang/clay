module clay_rocc_wrapper (
    input           clock,
    input           reset,

    //////////// ROCC COMMAND /////////////
    output          rocc_cmd_ready,
    input           rocc_cmd_valid,
    input [ 6: 0]   rocc_cmd_bits_inst_funct,
    input [ 4: 0]   rocc_cmd_bits_inst_rs2,
    input [ 4: 0]   rocc_cmd_bits_inst_rs1,
    input           rocc_cmd_bits_inst_xd,
    input           rocc_cmd_bits_inst_xs1,
    input           rocc_cmd_bits_inst_xs2,
    input [ 4: 0]   rocc_cmd_bits_inst_rd,
    input [ 6: 0]   rocc_cmd_bits_inst_opcode,
    input [31: 0]   rocc_cmd_bits_rs1,
    input [31: 0]   rocc_cmd_bits_rs2,
    input           rocc_cmd_bits_status_debug,
    input           rocc_cmd_bits_status_cease,
    input           rocc_cmd_bits_status_wfi,
    input [31: 0]   rocc_cmd_bits_status_isa,
    input [ 1: 0]   rocc_cmd_bits_status_dprv,
    input           rocc_cmd_bits_status_dv,
    input [ 1: 0]   rocc_cmd_bits_status_prv,
    input           rocc_cmd_bits_status_v,
    input           rocc_cmd_bits_status_sd,
    input [22: 0]   rocc_cmd_bits_status_zero2,
    input           rocc_cmd_bits_status_mpv,
    input           rocc_cmd_bits_status_gva,
    input           rocc_cmd_bits_status_mbe,
    input           rocc_cmd_bits_status_sbe,
    input [ 1: 0]   rocc_cmd_bits_status_sxl,
    input [ 1: 0]   rocc_cmd_bits_status_uxl,
    input           rocc_cmd_bits_status_sd_rv32,
    input [ 7: 0]   rocc_cmd_bits_status_zero1,
    input           rocc_cmd_bits_status_tsr,
    input           rocc_cmd_bits_status_tw,
    input           rocc_cmd_bits_status_tvm,
    input           rocc_cmd_bits_status_mxr,
    input           rocc_cmd_bits_status_sum,
    input           rocc_cmd_bits_status_mprv,
    input [ 1: 0]   rocc_cmd_bits_status_xs,
    input [ 1: 0]   rocc_cmd_bits_status_fs,
    input [ 1: 0]   rocc_cmd_bits_status_vs,
    input [ 1: 0]   rocc_cmd_bits_status_mpp,
    input           rocc_cmd_bits_status_spp,
    input           rocc_cmd_bits_status_mpie,
    input           rocc_cmd_bits_status_ube,
    input           rocc_cmd_bits_status_spie,
    input           rocc_cmd_bits_status_upie,
    input           rocc_cmd_bits_status_mie,
    input           rocc_cmd_bits_status_hie,
    input           rocc_cmd_bits_status_sie,
    input           rocc_cmd_bits_status_uie,
    /////////////////// ROCC RESPONSE ///////////////
    input           rocc_resp_ready,
    output          rocc_resp_valid,
    output [ 4: 0]  rocc_resp_bits_rd,
    output [31: 0]  rocc_resp_bits_data,
    /////////////////// ROCC MEM REQ ////////////////
    input           rocc_mem_req_ready,
    output          rocc_mem_req_valid,
    output [31: 0]  rocc_mem_req_bits_addr,
    output [ 7: 0]  rocc_mem_req_bits_tag,
    output [ 4: 0]  rocc_mem_req_bits_cmd,
    output [ 1: 0]  rocc_mem_req_bits_size,
    output          rocc_mem_req_bits_signed,
    output          rocc_mem_req_bits_phys,
    output          rocc_mem_req_bits_no_alloc,
    output          rocc_mem_req_bits_no_xcpt,
    output          rocc_mem_req_bits_no_resp,
    output [ 1: 0]  rocc_mem_req_bits_dprv,
    output          rocc_mem_req_bits_dv,
    output [31: 0]  rocc_mem_req_bits_data,
    output [ 3: 0]  rocc_mem_req_bits_mask,
    output          rocc_mem_s1_kill,
    output [31: 0]  rocc_mem_s1_data_data,
    output [ 3: 0]  rocc_mem_s1_data_mask,
    input           rocc_mem_s2_nack,
    input           rocc_mem_s2_nack_cause_raw,
    output          rocc_mem_s2_kill,
    input           rocc_mem_s2_uncached,
    input [31: 0]   rocc_mem_s2_paddr,
    input [31: 0]   rocc_mem_s2_gpa,
    input           rocc_mem_s2_gpa_is_pte,
    input           rocc_mem_resp_valid,
    input [31: 0]   rocc_mem_resp_bits_addr,
    input [ 7: 0]   rocc_mem_resp_bits_tag,
    input [ 4: 0]   rocc_mem_resp_bits_cmd,
    input [ 1: 0]   rocc_mem_resp_bits_size,
    input           rocc_mem_resp_bits_signed,
    input [31: 0]   rocc_mem_resp_bits_data,
    input [ 3: 0]   rocc_mem_resp_bits_mask,
    input           rocc_mem_resp_bits_replay,
    input           rocc_mem_resp_bits_has_data,
    input [31: 0]   rocc_mem_resp_bits_data_word_bypass,
    input [31: 0]   rocc_mem_resp_bits_data_raw,
    input [31: 0]   rocc_mem_resp_bits_store_data,
    input [ 1: 0]   rocc_mem_resp_bits_dprv,
    input           rocc_mem_resp_bits_dv,
    input           rocc_mem_replay_next,
    input           rocc_mem_s2_xcpt_ma_ld,
    input           rocc_mem_s2_xcpt_ma_st,
    input           rocc_mem_s2_xcpt_pf_ld,
    input           rocc_mem_s2_xcpt_pf_st,
    input           rocc_mem_s2_xcpt_gf_ld,
    input           rocc_mem_s2_xcpt_gf_st,
    input           rocc_mem_s2_xcpt_ae_ld,
    input           rocc_mem_s2_xcpt_ae_st,
    input           rocc_mem_ordered,
    input           rocc_mem_store_pending,
    input           rocc_mem_perf_acquire,
    input           rocc_mem_perf_release,
    input           rocc_mem_perf_grant,
    input           rocc_mem_perf_tlbMiss,
    input           rocc_mem_perf_blocked,
    input           rocc_mem_perf_canAcceptStoreThenLoad,
    input           rocc_mem_perf_canAcceptStoreThenRMW,
    input           rocc_mem_perf_canAcceptLoadThenLoad,
    input           rocc_mem_perf_storeBufferEmptyAfterLoad,
    input           rocc_mem_perf_storeBufferEmptyAfterStore,
    output          rocc_mem_keep_clock_enabled,
    input           rocc_mem_clock_enabled,

    //////////////////// ROCC AUX ////////////////////////////
    output          rocc_busy
  );

  clay_module u_clay_module (
    .clock(clock),
    .reset(reset),
    .rocc_cmd_ready(rocc_cmd_ready),
    .rocc_cmd_valid(rocc_cmd_valid),
    .rocc_cmd_bits_inst_funct(rocc_cmd_bits_inst_funct),
    .rocc_cmd_bits_inst_rs2(rocc_cmd_bits_inst_rs2),
    .rocc_cmd_bits_inst_rs1(rocc_cmd_bits_inst_rs1),
    .rocc_cmd_bits_inst_xd(rocc_cmd_bits_inst_xd),
    .rocc_cmd_bits_inst_xs1(rocc_cmd_bits_inst_xs1),
    .rocc_cmd_bits_inst_xs2(rocc_cmd_bits_inst_xs2),
    .rocc_cmd_bits_inst_rd(rocc_cmd_bits_inst_rd),
    .rocc_cmd_bits_inst_opcode(rocc_cmd_bits_inst_opcode),
    .rocc_cmd_bits_rs1(rocc_cmd_bits_rs1),
    .rocc_cmd_bits_rs2(rocc_cmd_bits_rs2),
    .rocc_resp_ready(rocc_resp_ready),
    .rocc_resp_valid(rocc_resp_valid),
    .rocc_resp_bits_rd(rocc_resp_bits_rd),
    .rocc_resp_bits_data(rocc_resp_bits_data),
    .rocc_mem_req_ready(rocc_mem_req_ready),
    .rocc_mem_req_valid(rocc_mem_req_valid),
    .rocc_mem_req_bits_addr(rocc_mem_req_bits_addr),
    .rocc_mem_req_bits_tag(rocc_mem_req_bits_tag),
    .rocc_mem_req_bits_cmd(rocc_mem_req_bits_cmd),
    .rocc_mem_req_bits_size(rocc_mem_req_bits_size),
    .rocc_mem_req_bits_signed(rocc_mem_req_bits_signed),
    .rocc_mem_req_bits_phys(rocc_mem_req_bits_phys),
    .rocc_mem_req_bits_data(rocc_mem_req_bits_data),
    .rocc_mem_req_bits_mask(rocc_mem_req_bits_mask),
    .rocc_mem_resp_valid(rocc_mem_resp_valid),
    .rocc_mem_resp_bits_addr(rocc_mem_resp_bits_addr),
    .rocc_mem_resp_bits_tag(rocc_mem_resp_bits_tag),
    .rocc_mem_resp_bits_cmd(rocc_mem_resp_bits_cmd),
    .rocc_mem_resp_bits_size(rocc_mem_resp_bits_size),
    .rocc_mem_resp_bits_signed(rocc_mem_resp_bits_signed),
    .rocc_mem_resp_bits_data(rocc_mem_resp_bits_data),
    .rocc_mem_resp_bits_mask(rocc_mem_resp_bits_mask),
    .rocc_busy(rocc_busy)
  );


  assign rocc_mem_req_bits_no_alloc = 'd0;
  assign rocc_mem_req_bits_no_xcpt = 'd0;
  assign rocc_mem_req_bits_no_resp = 'd0;
  assign rocc_mem_req_bits_dprv = rocc_cmd_bits_status_dprv;
  assign rocc_mem_req_bits_dv = rocc_cmd_bits_status_dv;
  assign rocc_mem_s1_kill = 'd0;
  assign rocc_mem_s1_data_data = 'd0;
  assign rocc_mem_s1_data_mask = 'd0;
  assign rocc_mem_s2_kill = 'd0;
  assign rocc_mem_keep_clock_enabled = 'd1;

endmodule

module clay_module (
    input                                clock,
    input                                reset,

    //////////// ROCC COMMAND /////////////
    output          rocc_cmd_ready,
    input           rocc_cmd_valid,
    input [ 6: 0]   rocc_cmd_bits_inst_funct,
    input [ 4: 0]   rocc_cmd_bits_inst_rs2,
    input [ 4: 0]   rocc_cmd_bits_inst_rs1,
    input           rocc_cmd_bits_inst_xd,
    input           rocc_cmd_bits_inst_xs1,
    input           rocc_cmd_bits_inst_xs2,
    input [ 4: 0]   rocc_cmd_bits_inst_rd,
    input [ 6: 0]   rocc_cmd_bits_inst_opcode,
    input [31: 0]   rocc_cmd_bits_rs1,
    input [31: 0]   rocc_cmd_bits_rs2,
    /////////////////// ROCC RESPONSE ///////////////
    input           rocc_resp_ready,
    output          rocc_resp_valid,
    output [ 4: 0]  rocc_resp_bits_rd,
    output [31: 0]  rocc_resp_bits_data,
    /////////////////// ROCC MEM REQ ////////////////
    input           rocc_mem_req_ready,
    output          rocc_mem_req_valid,
    output [31: 0]  rocc_mem_req_bits_addr,
    output [ 7: 0]  rocc_mem_req_bits_tag,
    output [ 4: 0]  rocc_mem_req_bits_cmd,
    output [ 1: 0]  rocc_mem_req_bits_size,
    output          rocc_mem_req_bits_signed,
    output          rocc_mem_req_bits_phys,
    output [31: 0]  rocc_mem_req_bits_data,
    output [ 3: 0]  rocc_mem_req_bits_mask,
    /////////////////// ROCC MEM RESP /////////////////////
    input           rocc_mem_resp_valid,
    input [31: 0]   rocc_mem_resp_bits_addr,
    input [ 7: 0]   rocc_mem_resp_bits_tag,
    input [ 4: 0]   rocc_mem_resp_bits_cmd,
    input [ 1: 0]   rocc_mem_resp_bits_size,
    input           rocc_mem_resp_bits_signed,
    input [31: 0]   rocc_mem_resp_bits_data,
    input [ 3: 0]   rocc_mem_resp_bits_mask,
    //////////////////// ROCC AUX ////////////////////////////
    output          rocc_busy
);

  clay_top_rocc u_clay_top_rocc(
    .clk                      ( clock                         ),
    .rst                      ( reset                         ),
    .rocc_cmd_funct           ( rocc_cmd_bits_inst_funct      ),
    .rocc_cmd_opcode          ( rocc_cmd_bits_inst_opcode     ),
    .rocc_cmd_rd              ( rocc_cmd_bits_inst_rd         ),
    .rocc_cmd_rs1             ( rocc_cmd_bits_inst_rs1        ),
    .rocc_cmd_rs1data         ( rocc_cmd_bits_rs1             ),
    .rocc_cmd_rs2             ( rocc_cmd_bits_inst_rs2        ),
    .rocc_cmd_rs2data         ( rocc_cmd_bits_rs2             ),
    .rocc_cmd_xd              ( rocc_cmd_bits_inst_xd         ),
    .rocc_cmd_xs1             ( rocc_cmd_bits_inst_xs1        ),
    .rocc_cmd_xs2             ( rocc_cmd_bits_inst_xs2        ),
    .rocc_cmd_enable          ( rocc_cmd_valid                ),
    .rocc_cmd_ready           ( rocc_cmd_ready                ),

    .rocc_resp_bus_rd     	  ( rocc_resp_bits_rd             ),
    .rocc_resp_bus_rddata 	  ( rocc_resp_bits_data           ),
    .rocc_resp_to_bus_enable  ( rocc_resp_valid               ),
    .rocc_resp_to_bus_ready   ( rocc_resp_ready               ),

    .hella_cmd_bus_addr       ( rocc_mem_req_bits_addr        ),
    .hella_cmd_bus_cmd        ( rocc_mem_req_bits_cmd         ),
    .hella_cmd_bus_data       ( rocc_mem_req_bits_data        ),
    .hella_cmd_bus_mask       ( rocc_mem_req_bits_mask        ),
    .hella_cmd_bus_phys       ( rocc_mem_req_bits_phys        ),
    .hella_cmd_bus_signed     ( rocc_mem_req_bits_signed      ),
    .hella_cmd_bus_size       ( rocc_mem_req_bits_size        ),
    .hella_cmd_bus_tag        ( rocc_mem_req_bits_tag         ),
    .hella_cmd_to_bus_enable  ( rocc_mem_req_valid            ),
    .hella_cmd_to_bus_ready   ( rocc_mem_req_ready            ),
    
    .hella_resp_addr          ( rocc_mem_resp_bits_addr       ),
    .hella_resp_cmd           ( rocc_mem_resp_bits_cmd        ),
    .hella_resp_data          ( rocc_mem_resp_bits_data       ),
    .hella_resp_mask          ( rocc_mem_resp_bits_mask       ),
    .hella_resp_signed        ( rocc_mem_resp_bits_signed     ),
    .hella_resp_size          ( rocc_mem_resp_bits_size       ),
    .hella_resp_tag           ( rocc_mem_resp_bits_tag        ),
    .hella_resp_enable        ( rocc_mem_resp_valid           ),
    .hella_resp_ready         ( /* unused */                  )
  );

  reg rocc_busy_reg;
  always @(posedge clock) begin
    if (reset) begin
      rocc_busy_reg <= 1'b0;
    end
    else begin
      if (rocc_cmd_valid && rocc_cmd_ready) begin
        rocc_busy_reg <= 1'b1;
      end
      else if (rocc_resp_valid && rocc_resp_ready) begin
        rocc_busy_reg <= 1'b0;
      end
    end
  end
  assign rocc_busy = rocc_busy_reg;

endmodule
