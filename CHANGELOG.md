# 0.2.1
- tracking of change on accounts for reconnects
- panic! on too low reconnect values, to avoid unstable behaviour
- instrumentation, tracing_futures RUST_LOG=solana_shadow=debug

# 0.2.0

- API change, `add_program`, `add_accounts` and `add_account` now return accounts
- blocking rpc calls now executed in thread
- changed "rpc call -> subscribe" into: "subsribe -> confirmation -> rpc call -> oneshot return"
- added timeout on build_async 

# 0.1.8

- Setting commitment level on RpcClient calls.

# 0.1.7

- Changing commitment config default to "finalized"
- Updated dependencies

# 0.1.6

- Commitment Config Option

# 0.1.5

- `for_each_account` accepts `FnMut` to allow capturing mut variables
- changed to edition = 2018 to be more compatible

# 0.1.4

- Auto-reconnect on connection drop
