use std::future::Future;

use anyhow::Result;
use sui_sdk::{ error::SuiRpcResult, SuiClient, SuiClientBuilder };

pub struct SuiClientProvider {
  clients: Vec<SuiClient>,
}

const CLIENT_URLS: [&'static str; 4] = [
  "https://wallet-rpc.mainnet.sui.io",
  "https://mainnet.suiet.app",
  "https://internal.suivision.xyz/mainnet/api",
  "https://fullnode.mainnet.sui.io",
];

impl SuiClientProvider {
  pub async fn init() -> Result<Self> {
    let mut clients = Vec::new();
    println!("start init");
    let mut index = 0;
    for url in CLIENT_URLS {
      let client = SuiClientBuilder::default().build(&url).await?;
      println!("initing {:?}", index);
      index += 1;
      clients.push(client);
    }
    println!("end init");

    Ok(Self {
      clients,
    })
  }

  pub async fn open_client<T, R, S>(&self, callback: T) -> SuiRpcResult<S>
    where T: Fn(SuiClient) -> R, R: Future<Output = SuiRpcResult<S>>
  {
    let mut index = 0;
    loop {
      if index >= self.clients.len() {
        index = 0;
      }

      let sui_client = self.clients[index].clone();
      let callback_res = callback(sui_client).await;

      return match callback_res {
        Ok(res) => Ok(res),
        Err(err) => Err(err),
      };
    }
  }
}
