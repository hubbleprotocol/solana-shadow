# Solana Shadow

The Solana Shadow crate adds shadows to solana on-chain accounts for off-chain processing. This create synchronises all accounts and their data related to a program in real time and allows off-chain bots to act upon changes to those accounts.

[![Apache 2.0 licensed](https://img.shields.io/badge/license-Apache--2.0-blue)](./LICENSE)

## Usage

Add this in your `Cargo.toml`:

```toml
[dependencies]
solana-shadow = "*"
```

Take a look at the `examples/` directory for usage examples.