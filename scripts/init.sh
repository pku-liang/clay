#!/bin/bash

set -euo pipefail

RDIR=$(git rev-parse --show-toplevel)

cd $RDIR
git submodule update --init chipyard cmt2

pushd $RDIR/chipyard
cat <<EOF > clay_config.json
{
  "arch": "rv32",
  "backend": "rocc",
  "core_count": 1,
  "top_module": "clay_rocc_wrapper",
  "name": "clay_rocc_module",
  "vsrc": [
      "${RDIR}/rocc_top.sv",
      "${RDIR}/crates/clayrs/res/clay_rocc_wrapper.sv"
  ]
}
EOF


bash ./scripts/init-submodules-no-riscv-tools-nolog.sh
bash ./scripts/build-rv32-toolchain-extra.sh -p $RISCV
popd
