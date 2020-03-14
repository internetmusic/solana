#!/usr/bin/env bash

here=$(dirname "$0")
# shellcheck source=multinode-demo/common.sh
source "$here"/common.sh

set -e

rm -rf "$SOLANA_CONFIG_DIR"/bootstrap-validator
mkdir -p "$SOLANA_CONFIG_DIR"/bootstrap-validator

# Create genesis ledger
if [[ -r $FAUCET_KEYPAIR ]]; then
  cp -f "$FAUCET_KEYPAIR" "$SOLANA_CONFIG_DIR"/faucet.json
else
  $solana_keygen new --no-passphrase -fso "$SOLANA_CONFIG_DIR"/faucet.json
fi

if [[ -f $BOOTSTRAP_VALIDATOR_IDENTITY_KEYPAIR ]]; then
  cp -f "$BOOTSTRAP_VALIDATOR_IDENTITY_KEYPAIR" "$SOLANA_CONFIG_DIR"/bootstrap-validator/identity.json
else
  $solana_keygen new --no-passphrase -so "$SOLANA_CONFIG_DIR"/bootstrap-validator/identity.json
fi

$solana_keygen new --no-passphrase -so "$SOLANA_CONFIG_DIR"/bootstrap-validator/vote-account.json
$solana_keygen new --no-passphrase -so "$SOLANA_CONFIG_DIR"/bootstrap-validator/stake-account.json

args=(
  "$@"
  --enable-warmup-epochs
  --bootstrap-validator "$SOLANA_CONFIG_DIR"/bootstrap-validator/identity.json
                        "$SOLANA_CONFIG_DIR"/bootstrap-validator/vote-account.json
                        "$SOLANA_CONFIG_DIR"/bootstrap-validator/stake-account.json
)
default_arg --ledger "$SOLANA_CONFIG_DIR"/bootstrap-validator
default_arg --faucet-pubkey "$SOLANA_CONFIG_DIR"/faucet.json
default_arg --faucet-lamports 500000000000000000
default_arg --hashes-per-tick auto
default_arg --operating-mode development
$solana_genesis "${args[@]}"
