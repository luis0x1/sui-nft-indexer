use std::{ future::Future, sync::Arc, thread, time::Duration };

use anyhow::Result;
use sui_sdk::{ SuiClient, SuiClientBuilder };

pub struct SuiClientProvider {
  clients: Arc<Vec<SuiClient>>,
}

const CLIENT_URLS: [&'static str; 4] = [
  "https://wallet-rpc.mainnet.sui.io",
  "https://fullnode.mainnet.sui.io",
  "https://internal.suivision.xyz/mainnet/api",
  "https://mainnet.suiet.app",
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
      clients: Arc::new(clients),
    })
  }

  pub async fn open_client<T, R, S>(&self, callback: T) -> Result<S>
    where T: Fn(SuiClient) -> R, R: Future<Output = Result<S>>
  {
    let mut index = 0;
    loop {
      if index >= self.clients.len() {
        index = 0;
      }

      let sui_client = self.clients[index].clone();
      let callback_res = callback(sui_client).await;

      let value = match callback_res {
        Ok(res) => Ok(res),
        Err(err) => {
          let error_str = err.to_string();
          if
            error_str.contains("Request rejected `429`") ||
            error_str.contains("Can't assign requested address (os error 49)")
          {
            println!("call blockchain error try next rpc: {index} -> {:?}", err);
            index += 1;
            thread::sleep(Duration::from_secs(index as u64));
            continue;
          }

          Err(err)
        }
      };

      return value;
    }
  }

  pub async fn open_client_with_params<T, R, S, Params>(
    &self,
    callback: T,
    params: Params
  )
    -> Result<S>
    where T: Fn(SuiClient, Arc<Params>) -> R, R: Future<Output = Result<S>>
  {
    let mut index = 0;
    let p = Arc::new(params);
    loop {
      println!("index -> {:?}", index);
      if index >= self.clients.len() {
        index = 0;
      }

      let sui_client = self.clients[index].clone();
      let callback_res = callback(sui_client, p.clone()).await;

      let value = match callback_res {
        Ok(res) => Ok(res),
        Err(err) => {
          if err.to_string().contains("Request rejected `429`") {
            println!("call blockchain error try next rpc: {index} -> {:?}", err);
            index += 1;
            continue;
          }

          Err(err)
        }
      };

      return value;
    }
  }
}
