#!/bin/bash
set -x
export PATH="/home/runner/.local/share/solana/install/active_release/bin:$PATH"
source .env

for i in $versions_for_test
do
  echo "Test on version $i"
  solana-install init $i
  timeout 10 solana-test-validator --geyser-plugin-config config/geyser-plugin-config.json &
  sleep 5
  RES=$(curl -s http://127.0.0.1:8899 -X POST -H "Content-Type: application/json" -d '{"jsonrpc": "2.0", "id": 1, "method": "getIdentity"}' | jq .result.identity)
  if [[ -n $RES ]]
  then
    echo "PASSED $RES"
  else 
    echo "NOT PASSED $RES"
    exit 1
  fi
  sleep 7
done



