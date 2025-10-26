# Clay: High-level ASIP Framework for Flexible Microarchitecture-Aware Instruction Customization

<div align="center">

[![Paper](https://img.shields.io/badge/ICCAD'25-Accepted-blue)](https://iccad.com/)
[![Platform](https://img.shields.io/badge/Platform-Linux-lightgrey.svg)](https://www.linux.org/)

</div>

## Citation

If you use Clay in your research, please cite our ICCAD'25 paper:

```bibtex
@inproceedings{peng2025clay,
  title={Clay: High-level ASIP Framework for Flexible Microarchitecture-Aware Instruction Customization},
  author={Peng, Weijie and Xiao, Youwei and Zou, Yuyang and Luo, Zizhang and Liang, Yun},
  booktitle={2025 IEEE/ACM International Conference on Computer-Aided Design (ICCAD)},
  year={2025},
  organization={IEEE}
}
```

## Usage

1. Install [pixi](https://pixi.sh/latest/installation/) package manager

```bash
curl -fsSL https://pixi.sh/install.sh | sh
```

2. Run init script to prepare chipyard environment

```bash
pixi s
# in the new shell
bash scripts/init.sh
```

3. Test custom instructions

```bash
cargo run --package clay -- --input-file samples/stream_add.cadl
pushd chipyard/tests
cmake -B build -S .
cmake --build build
popd
cd chipyard/sims/verilator
make CONFIG=ClayRocketConfig BINARY=../../tests/stream_add.riscv  run-binary-debug LOADMEM=1
```

4. See `samples` for more examples!

---

Also check out **APS (Agile Processor Synthesis)**! -- an end-to-end open-source framework for rapid hardware-software co-design of domain-specific RISC-V processors.
APS enables researchers and developers to design, synthesize, compile, simulate, and physically implement custom instruction set extensions with minimal effort.

Visit [pku-liang/aps](https://github.com/pku-liang/aps) to learn more and get started.
