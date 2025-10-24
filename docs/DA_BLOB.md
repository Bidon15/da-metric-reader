Package

```toml
[package]
name = "celestia-client"
version = "0.2.0"
```

Example use:

```rust
use std::env;

use celestia_client::{Client, Result};
use celestia_client::tx::TxConfig;
use celestia_client::types::nmt::Namespace;
use celestia_client::types::{AppVersion, Blob};

#[tokio::main]
async fn main() -> Result<()> {
let client = Client::builder()
.rpc_url("ws://localhost:26658")
.grpc_url("http://localhost:9090")
.private_key_hex("393fdb5def075819de55756b45c9e2c8531a8c78dd6eede483d3440e9457d839")
.build()
.await?;

    // Create the blob
    let ns = Namespace::new_v0(b"mydata")?;
    let blob = Blob::new(
        ns,
        b"some data to store".to_vec(),
        Some(client.address()?),
        AppVersion::V5,
    )?;

    // This is the hash of the blob which is needed later on for retrieving
    // it form chain.
    let commitment = blob.commitment.clone();

    // Submit the blob
    let tx_info = client.blob().submit(&[blob], TxConfig::default()).await?;

    // Retrieve the blob. Blob is validated within the `get` method, so
    // we don't need to do anything else.
    let received_blob = client
        .blob()
        .get(tx_info.height.value(), ns, commitment)
        .await?;

    println!("Data: {:?}", str::from_utf8(&received_blob.data).unwrap());

    Ok(())

}

```
