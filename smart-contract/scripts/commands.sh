#!/usr/bin/env bash
set -euo pipefail

# 0) Import accounts
concordium-client config account import ./path/to/client.export --name client
concordium-client config account import ./path/to/freelancer.export --name freelancer
concordium-client config account import ./path/to/oracle.export --name oracle

# 1) Build WASM + schema
cargo concordium build --out dist/worklog.wasm.v1 --schema-out dist/worklog.schema.bin

# 2) Deploy module
concordium-client module deploy dist/worklog.wasm.v1 \
  --sender client \
  --name worklog_mod_2 \
  --grpc-ip 127.0.0.1 \
  --grpc-port 20100

# 3) Init contract instance
concordium-client contract init worklog_mod_2 \
  --contract worklog \
  --parameter-json schema/init.json \
  --schema dist/worklog.schema.bin \
  --sender client \
  --name worklog_inst_2 \
  --energy 1000000 \
  --grpc-ip 127.0.0.1 \
  --grpc-port 20100

# 4) Oracle requests release
concordium-client contract update worklog_inst_2 \
  --entrypoint requestRelease \
  --parameter-json schema/request.json \
  --schema dist/worklog.schema.bin \
  --sender oracle \
  --energy 1000000 \
  --grpc-ip 127.0.0.1 \
  --grpc-port 20100

# 5) Client sends PLT to freelancer (off-chain token-holder op)
concordium-client transaction plt send \
  --sender client \
  --receiver freelancer \
  --amount 10000000 \               # minor units (e.g., 100 with 8 decimals)
  --tokenId PAYLOGPLT \
  --grpc-ip 127.0.0.1 \
  --grpc-port 20100

# 6) Client confirms payment (final attestation)
concordium-client contract update worklog_inst_2 \
  --entrypoint confirmPayment \
  --parameter-json schema/confirm.json \
  --schema dist/worklog.schema.bin \
  --sender client \
  --energy 1000000 \
  --grpc-ip 127.0.0.1 \
  --grpc-port 20100

# 7) Read back state
concordium-client contract invoke worklog_inst_2 \
  --entrypoint viewMilestone \
  --parameter-json schema/view.json \
  --schema dist/worklog.schema.bin \
  --grpc-ip 127.0.0.1 \
  --grpc-port 20100
