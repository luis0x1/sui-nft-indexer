use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use redis::{ aio::MultiplexedConnection, AsyncCommands, ToRedisArgs };
use serde::{ de::DeserializeOwned, Deserialize, Serialize };
use uuid::Uuid;
use reqwest::Client as HttpClient;
use crate::{ env::get_env, scan_worker::AppProvider };

use super::save_object_job::SaveObjectsJobPayload;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
  I32(i32),
  I128(i128),
  String(String),
  SaveObjects(SaveObjectsJobPayload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
  pub id: String,
  pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubSubMessage {
  pub id: String,
  pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
  pub key: String,
  pub payload: Message,
}

impl From<Message> for Task {
  fn from(message: Message) -> Self {
    Self { key: format!("{}:message:{}", get_env().channel_id, message.id), payload: message }
  }
}

impl ToRedisArgs for Message {
  fn write_redis_args<W>(&self, out: &mut W) where W: ?Sized + redis::RedisWrite {
    out.write_arg(serde_json::to_string(self).unwrap().as_bytes());
  }
}

impl ToRedisArgs for PubSubMessage {
  fn write_redis_args<W>(&self, out: &mut W) where W: ?Sized + redis::RedisWrite {
    out.write_arg(serde_json::to_string(self).unwrap().as_bytes());
  }
}

#[async_trait]
pub trait BaseJob<T> {
  #[allow(unused)]
  async fn handle(provider: &AppProvider, job: T) -> Result<()>;
  #[allow(unused)]
  async fn dispatch(provider: &AppProvider, payload: MessageContent) -> Result<()>;
  async fn send(
    redis_connection: &mut MultiplexedConnection,
    http_client: Arc<HttpClient>,
    payload: MessageContent
  ) -> Result<()> {
    let id = Uuid::new_v4().to_string();
    let message = Message {
      id: id.clone(),
      content: payload.clone(),
    };
    let channel_id = get_env().channel_id;
    let task_key = format!("{}:message:{}", channel_id, id);

    let pubsub_message = PubSubMessage {
      id: id.clone(),
      key: task_key.clone(),
    };

    let _: () = redis_connection.set(task_key, &message).await?;

    let send_notification = http_client
      .post("http://127.0.0.1:2811/message")
      .json(&pubsub_message)
      .send().await;

    match send_notification {
      Err(e) => eprintln!("Cannot send notification to worker: {:?}", e),
      Ok(_) => eprintln!("Send notification to worker successfully"),
    }

    Ok(())
  }
}
