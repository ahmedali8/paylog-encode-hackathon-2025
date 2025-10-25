#!/usr/bin/env bash
set -euo pipefail

echo "🚀 Starting evaluation and contract update process..."

# Step 1: Run the TypeScript evaluation script
echo "📊 Running evaluation script..."
cd agent
npx tsx src/contract-intgeration.ts
cd ..

# Step 2: Update contract with the new hash
echo "📝 Updating contract with new work hash..."
printf "y\n\n" | concordium-client contract update worklog_inst_2 \
  --entrypoint requestRelease \
  --parameter-json smart-contract/schema/request.json \
  --schema smart-contract/dist/worklog.schema.bin \
  --sender oracle \
  --energy 1000000 \
  --grpc-ip 127.0.0.1 \
  --grpc-port 20100

echo "✅ Process completed successfully!"