// Cycle-Accurate Testbench for Bit Accelerator v2
// Verifies exact cycle-level timing and reset behavior

module tb_bit_accelerator_v2 ();

  localparam int ADDR_WIDTH = 64;
  localparam int WORD_WIDTH = 64;
  localparam int WORD_BYTES = WORD_WIDTH / 8;

  // Clock and Reset
  logic clk;
  logic reset;

  // DUT signals
  logic        op_valid;
  logic        op_ready;
  logic [63:0] base_address;
  logic [63:0] bit_offset;
  logic [2:0]  operation;

  logic        result_valid;
  logic        result_bit;
  logic        error;

  logic                   mem_valid;
  logic                   mem_write;
  logic [ADDR_WIDTH-1:0]  mem_addr;
  logic [WORD_WIDTH-1:0]  mem_wdata;
  logic [WORD_BYTES-1:0]  mem_wstrb;
  logic                   mem_ready;
  logic                   mem_rvalid;
  logic [WORD_WIDTH-1:0]  mem_rdata;
  logic                   mem_fault;

  // DUT
  bit_accelerator_v2 dut (
    .clk         (clk),
    .reset       (reset),
    .op_valid    (op_valid),
    .op_ready    (op_ready),
    .base_address(base_address),
    .bit_offset  (bit_offset),
    .operation   (operation),
    .result_valid(result_valid),
    .result_bit  (result_bit),
    .error       (error),
    .mem_valid   (mem_valid),
    .mem_write   (mem_write),
    .mem_addr    (mem_addr),
    .mem_wdata   (mem_wdata),
    .mem_wstrb   (mem_wstrb),
    .mem_ready   (mem_ready),
    .mem_rvalid  (mem_rvalid),
    .mem_rdata   (mem_rdata),
    .mem_fault   (mem_fault)
  );

  // Simple Memory Model
  logic [63:0] memory [0:1023];

  // Clock generation
  initial begin
    clk = 1'b0;
    forever #5 clk = ~clk;
  end

  // Memory simulation (1-cycle latency for simplicity)
  logic mem_read_pending;
  logic [ADDR_WIDTH-1:0] pending_read_addr;

  initial begin
    mem_ready = 1'b1;
    mem_rvalid = 1'b0;
    mem_rdata = 64'h0;
    mem_read_pending = 1'b0;
  end

  always_ff @(posedge clk) begin
    mem_rvalid <= 1'b0;

    if (mem_valid && mem_ready) begin
      if (mem_write) begin
        // Write operation
        logic [ADDR_WIDTH-1:0] word_addr = mem_addr >> 3;
        if (word_addr < 1024) begin
          for (int i = 0; i < WORD_BYTES; i++) begin
            if (mem_wstrb[i]) begin
              memory[word_addr][i*8 +: 8] <= mem_wdata[i*8 +: 8];
            end
          end
        end
      end else begin
        // Read operation - will respond next cycle
        mem_read_pending <= 1'b1;
        pending_read_addr <= mem_addr;
      end
    end

    if (mem_read_pending) begin
      logic [ADDR_WIDTH-1:0] word_addr = pending_read_addr >> 3;
      if (word_addr < 1024) begin
        mem_rdata <= memory[word_addr];
      end else begin
        mem_rdata <= 64'hDEADBEEFDEADBEEF;
      end
      mem_rvalid <= 1'b1;
      mem_read_pending <= 1'b0;
    end
  end

  // Test counter and reporting
  int cycle_count;
  int test_count = 0;
  int pass_count = 0;
  int fail_count = 0;

  task automatic clock_edge();
    @(posedge clk);
    cycle_count++;
  endtask

  task automatic init_memory();
    for (int i = 0; i < 1024; i++) begin
      memory[i] = {32'hABCD_EF00 | i[15:0], 32'h1234_5670 | i[15:0]};
    end
  endtask

  task automatic reset_dut();
    reset = 1'b1;
    op_valid = 1'b0;
    base_address = 64'h0;
    bit_offset = 64'h0;
    operation = 3'h0;
    clock_edge();
    reset = 1'b0;
    clock_edge();
  endtask

  task automatic test_bit_get_timing();
    // Test: BIT_GET with 1-cycle memory latency
    // Expected timeline:
    // Cycle N:   OP_ACCEPT
    // Cycle N+1: READ_REQUEST
    // Cycle N+2: READ_RESPONSE
    // Cycle N+3: RESULT_VALID

    $display("\n=== TEST: BIT_GET Cycle Timing ===");
    test_count++;

    int start_cycle = cycle_count;

    // Cycle N: Issue operation
    op_valid = 1'b1;
    base_address = 64'h0;
    bit_offset = 64'h0;
    operation = 3'b000;  // BIT_GET

    clock_edge();
    if (!op_ready) begin
      $display("FAIL: op_ready not asserted on acceptance");
      fail_count++;
      return;
    end

    op_valid = 1'b0;

    // Cycle N+1: Expect READ_REQUEST
    clock_edge();
    if (!mem_valid || mem_write) begin
      $display("FAIL: mem_valid should be 1, mem_write should be 0 at N+1");
      fail_count++;
      return;
    end

    // Cycle N+2: READ_RESPONSE arrives
    clock_edge();
    if (!mem_rvalid) begin
      $display("FAIL: mem_rvalid should be 1 at N+2");
      fail_count++;
      return;
    end

    // Cycle N+3: RESULT_VALID should be 1
    clock_edge();
    if (!result_valid) begin
      $display("FAIL: result_valid should be 1 at N+3");
      fail_count++;
      return;
    end

    // Cycle N+4: RESULT_VALID should be 0
    clock_edge();
    if (result_valid) begin
      $display("FAIL: result_valid should be 0 at N+4 (one-cycle pulse)");
      fail_count++;
      return;
    end

    $display("PASS: BIT_GET timing correct (4 cycles total)");
    pass_count++;
  endtask

  task automatic test_bit_set_timing();
    // Test: BIT_SET with 1-cycle memory latency
    // Expected timeline:
    // Cycle N:   OP_ACCEPT
    // Cycle N+1: READ_REQUEST
    // Cycle N+2: READ_RESPONSE
    // Cycle N+3: MODIFY (combinational, completes in 1 cycle)
    // Cycle N+4: WRITE_REQUEST
    // Cycle N+5: WRITE_ACCEPT
    // Cycle N+6: RESULT_VALID

    $display("\n=== TEST: BIT_SET Cycle Timing ===");
    test_count++;

    // Cycle N: Issue operation
    op_valid = 1'b1;
    base_address = 64'h0;
    bit_offset = 64'h0;
    operation = 3'b010;  // BIT_SET

    clock_edge();
    if (!op_ready) begin
      $display("FAIL: op_ready not asserted");
      fail_count++;
      return;
    end

    op_valid = 1'b0;

    // Cycle N+1: READ_REQUEST
    clock_edge();
    if (!mem_valid || mem_write) begin
      $display("FAIL: Should issue READ_REQUEST at N+1");
      fail_count++;
      return;
    end

    // Cycle N+2: READ_RESPONSE
    clock_edge();
    if (!mem_rvalid) begin
      $display("FAIL: mem_rvalid should be 1 at N+2");
      fail_count++;
      return;
    end

    // Cycle N+3: MODIFY state (internal)
    clock_edge();

    // Cycle N+4: WRITE_REQUEST
    if (!mem_valid || !mem_write) begin
      $display("FAIL: Should issue WRITE_REQUEST at N+4");
      fail_count++;
      return;
    end
    clock_edge();

    // Cycle N+5: WRITE accepted
    if (!mem_valid || !mem_write) begin
      $display("FAIL: WRITE_REQUEST should still be valid at N+5");
      fail_count++;
      return;
    end
    clock_edge();

    // Cycle N+6: RESULT_VALID
    if (!result_valid) begin
      $display("FAIL: result_valid should be 1 at N+6");
      fail_count++;
      return;
    end

    $display("PASS: BIT_SET timing correct (6 cycles total)");
    pass_count++;
  endtask

  task automatic test_write_stall();
    // Test: Memory stalls write with mem_ready = 0
    $display("\n=== TEST: Write Stall (mem_ready = 0) ===");
    test_count++;

    // Start a SET operation
    op_valid = 1'b1;
    base_address = 64'h0;
    bit_offset = 64'h0;
    operation = 3'b010;
    clock_edge();
    op_valid = 1'b0;

    // Progress through read
    clock_edge();  // N+1: READ_REQUEST
    clock_edge();  // N+2: READ_RESPONSE
    clock_edge();  // N+3: MODIFY
    clock_edge();  // N+4: WRITE_REQUEST

    // Now stall the write
    mem_ready = 1'b0;
    clock_edge();  // N+5: Write should remain asserted

    if (!mem_valid || !mem_write) begin
      $display("FAIL: Write signals should remain asserted during stall");
      fail_count++;
      mem_ready = 1'b1;
      return;
    end

    clock_edge();  // N+6: Still stalled
    if (!mem_valid || !mem_write) begin
      $display("FAIL: Write signals should remain asserted during stall");
      fail_count++;
      mem_ready = 1'b1;
      return;
    end

    // Release stall
    mem_ready = 1'b1;
    clock_edge();  // N+7: Write accepted

    clock_edge();  // N+8: RESULT_VALID
    if (!result_valid) begin
      $display("FAIL: result_valid not asserted after write stall");
      fail_count++;
      return;
    end

    $display("PASS: Write stall handling correct");
    pass_count++;
  endtask

  task automatic test_reset_cancellation();
    // Test: Reset cancels in-flight operation
    $display("\n=== TEST: Reset Cancellation ===");
    test_count++;

    // Start a GET operation
    op_valid = 1'b1;
    base_address = 64'h0;
    bit_offset = 64'h0;
    operation = 3'b000;
    clock_edge();
    op_valid = 1'b0;

    clock_edge();  // N+1: READ_REQUEST

    // Assert reset
    reset = 1'b1;
    clock_edge();
    reset = 1'b0;

    // Verify reset state
    clock_edge();

    if (op_ready != 1'b1) begin
      $display("FAIL: op_ready should be 1 after reset");
      fail_count++;
      return;
    end

    // Verify result_valid is not asserted for cancelled operation
    if (result_valid) begin
      $display("FAIL: result_valid should not pulse for cancelled operation");
      fail_count++;
      return;
    end

    // Verify mem_valid is cleared
    if (mem_valid) begin
      $display("FAIL: mem_valid should be 0 after reset");
      fail_count++;
      return;
    end

    $display("PASS: Reset cancellation correct");
    pass_count++;
  endtask

  // Main Testbench
  initial begin
    $display("\n========================================");
    $display("    CYCLE-ACCURATE VERIFICATION        ");
    $display("========================================\n");

    cycle_count = 0;
    init_memory();
    reset_dut();

    test_bit_get_timing();
    reset_dut();

    test_bit_set_timing();
    reset_dut();

    test_write_stall();
    reset_dut();

    test_reset_cancellation();

    $display("\n========================================");
    $display("    SUMMARY                            ");
    $display("========================================");
    $display("Tests:  %0d", test_count);
    $display("Passed: %0d", pass_count);
    $display("Failed: %0d", fail_count);

    if (fail_count == 0) begin
      $display("\n✓ All tests PASSED!");
    end else begin
      $display("\n✗ %0d tests FAILED!", fail_count);
    end
    $display("========================================\n");

    #100;
    $finish;
  end

endmodule : tb_bit_accelerator_v2
