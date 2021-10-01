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

# Mirroring a program id and all its owned accounts:

```rust
// this is the prog id that owns all pyth oracles on mainnet
let prog = "FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH".parse()?;
let network = Network::Mainnet;
let local = BlockchainShadow::new_for_program(&prog, network).await?;

loop {
  local.for_each_account(|pubkey, account| {
    println!(" - [{}]: {:?}", pubkey, account);
  });

  sleep(Duration::from_secs(3)).await;
}

local.worker().await?;
```

# Mirroring few random accounts

```rust
  // https://pyth.network/developers/accounts/
  let ethusd = "JBu1AL4obBcCMqKBBxhpWCNUt136ijcuMZLFvTP7iWdB".parse()?;
  let btcusd = "GVXRSBjFk6e6J3NbVPXohDJetcTjaeeuykUpbQF8UoMU".parse()?;

  let local = BlockchainShadow::new_for_accounts(&vec![ethusd, btcusd], Network::Mainnet).await?;

  loop {
    let ethacc = shadow.get_account(&ethusd).unwrap();
    let ethprice = cast::<Price>(&ethacc.data).agg.price;

    let btcacc = shadow.get_account(&btcusd).unwrap();
    let btcprice = cast::<Price>(&btcacc.data).agg.price;

    println!("ETH/USD: {}", ethprice);
    println!("BTC/USD: {}", btcprice);

    sleep(Duration::from_secs(3)).await;
  }

  local.worker().await?;
```


# Listening on changes to accounts: 

```rust

  // https://pyth.network/developers/accounts/
  let ethusd = "JBu1AL4obBcCMqKBBxhpWCNUt136ijcuMZLFvTP7iWdB".parse()?;
  let btcusd = "GVXRSBjFk6e6J3NbVPXohDJetcTjaeeuykUpbQF8UoMU".parse()?;
  let solusd = "H6ARHf6YXhGYeQfUzQNGk6rDNnLBQKrenN712K4AQJEG".parse()?;

  // create an offline shadow of the on-chain data.
  // whenever the data change on-chain those changes
  // will be reflected immediately in this type.
  let shadow = BlockchainShadow::new_for_accounts(
    &vec![ethusd, btcusd, solusd],
    Network::Mainnet,
  )
  .await?;

  tokio::spawn(async move {
    // start printing updates only after 5 seconds
    tokio::time::sleep(Duration::from_secs(5)).await;

    // now everytime an account changes, its pubkey will be
    // broadcasted to all receivers that are waiting on updates.
    while let Ok((pubkey, account)) = updates_channel.recv().await {
      let price = cast::<Price>(&account.data).agg.price;
      println!("account updated: {}: {}", &pubkey, price);
    }
  });

  shadow.worker().await?;

```