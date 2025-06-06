use anyhow::{ Result, Error };
use redis::{ aio::MultiplexedConnection, AsyncCommands, Client as RedisClient };
use tokio::{ sync::RwLock, time::Instant };
use axum::{ routing::{ get, post }, Extension, Json, Router, http::StatusCode };
use std::{ sync::Arc, thread, time::Duration };
use async_scoped::TokioScope;

use crate::{
  env::get_env,
  queue::{
    base_job::{ BaseJob, Message, MessageContent, PubSubMessage, Task },
    handle_failed_checkpoint_job::HandleFailedCheckpointJob,
    parse_object_fields_job::ParseObjectsFieldsJob,
    save_object_job::SaveObjectsJob,
  },
  scan_worker::AppProvider,
  utils::async_vec::AsyncVecDeque,
};

pub async fn setup_worker_flow(concurrency: u8) -> Result<()> {
  let provider = Arc::new(AppProvider::init().await?);
  let start = Instant::now();
  let all_task = Arc::new(init_task(&provider.worker_client).await?);
  let stop = Instant::now();
  println!("take tasks cost: {:?} with length: {:?}", stop - start, all_task.len().await);

  // Subscribe vào channel
  let task_subscriber = Arc::clone(&all_task);
  {
    let client_clone = Arc::clone(&provider.worker_client);
    tokio::spawn(async move {
      let processed = subscriber(&client_clone, &task_subscriber).await;

      if processed.is_err() {
        println!("process message error {:?}", processed.unwrap_err())
      }
    });
  }

  TokioScope::scope_and_block(|scope| {
    for _ in 0..concurrency {
      let all_task_clone = Arc::clone(&all_task);
      let client_clone = Arc::clone(&provider.worker_client);
      let provider_clone = Arc::clone(&provider);

      scope.spawn(async move {
        match client_clone.get_multiplexed_tokio_connection().await {
          Ok(ref mut connection) => {
            loop {
              let task_taked = all_task_clone.take().await;
              process_task(
                &provider_clone,
                connection,
                Arc::clone(&all_task_clone),
                Arc::new(task_taked)
              ).await;
              thread::sleep(Duration::from_millis(10));
            }
          }
          Err(_) => {
            eprintln!("Cannot get connection");
          }
        }
      });
    }
  });

  Ok(())
}

async fn init_task(redis_client: &Arc<RedisClient>) -> Result<AsyncVecDeque<Task>> {
  let tasks = AsyncVecDeque::<Task>::new();
  let mut cursor = "0".to_string();
  let mut conn = redis_client.get_multiplexed_tokio_connection().await?;

  loop {
    let messages_res: (String, Vec<String>) = redis
      ::cmd("SCAN")
      .arg(&cursor)
      .arg("MATCH")
      .arg(format!("{}:message:*", get_env().channel_id))
      .arg("COUNT")
      .arg(1000)
      .query_async(&mut conn).await?;

    cursor = messages_res.0;
    let messages = messages_res.1;

    for message_key in messages {
      let items: Vec<String> = conn.get(&message_key).await?;
      if items.len() > 0 {
        let message: Result<Message, _> = serde_json::from_str(&items[0]);
        if message.is_ok() {
          tasks.push_back(Task {
            key: message_key,
            payload: message.unwrap(),
            retry_count: 0,
          }).await;
        } else {
          println!("messages error: {:?}", message.unwrap_err());
        }
      }
    }

    if cursor == "0" {
      break;
    }
  }
  println!("messages: {:?}", tasks.len().await);

  Ok(tasks)
}

struct AppState {
  redis_conn: Arc<RwLock<MultiplexedConnection>>,
  all_task: Arc<AsyncVecDeque<Task>>,
}

async fn subscriber(client: &Arc<RedisClient>, all_task: &Arc<AsyncVecDeque<Task>>) -> Result<()> {
  let conn = client.get_multiplexed_tokio_connection().await?;

  println!("> Subscribed to '{}'", get_env().channel_id);

  let app = Router::new()
    .route(
      "/",
      get(|| async { "Hello, World!" })
    )
    .route("/message", post(on_message))
    .layer(
      Extension(
        Arc::new(AppState {
          all_task: Arc::clone(all_task),
          redis_conn: Arc::new(RwLock::new(conn)),
        })
      )
    );

  let listener = tokio::net::TcpListener::bind(get_env().worker_url).await?;
  println!("Worker listen on port 2811");
  axum::serve(listener, app).await?;

  Ok(())
}

async fn on_message(
  Extension(state): Extension<Arc<AppState>>,
  Json(message): Json<PubSubMessage>
) -> Result<(), (StatusCode, String)> {
  let mut conn = state.redis_conn.write().await;
  let message_key = message.key;
  let task_res: Result<String, _> = (*conn).get(&message_key).await;
  drop(conn);
  tokio::spawn(async move {
    println!("-> Received from api {:?}", message_key);

    if let Ok(task) = task_res {
      let task_data: Result<Message, _> = serde_json::from_str(&task);
      if let Ok(message) = task_data {
        println!("-> Received from api {:?} successfully", message_key);

        state.all_task.push_back(message.into()).await;
      }
    } else {
      eprintln!("Error key: {:?} -> {:?}", message_key, task_res.unwrap_err());
    }
  });

  Ok(())
}

async fn process_task(
  provider: &Arc<AppProvider>,
  redis_connection: &mut MultiplexedConnection,
  all_tasks: Arc<AsyncVecDeque<Task>>,
  task: Arc<Task>
) {
  let res = _process_task(provider, redis_connection, all_tasks, &task).await;

  match res {
    Ok(task_type) =>
      println!(
        "process task [{}] {} successfully in thread: {:?}",
        task_type,
        task.key,
        thread::current().id()
      ),
    Err(error) => eprintln!("process task {:?} failed: {:?}", task.key, error),
  }
}

async fn _process_task(
  provider: &Arc<AppProvider>,
  redis_connection: &mut MultiplexedConnection,
  all_tasks: Arc<AsyncVecDeque<Task>>,
  task: &Task
) -> Result<String> {
  let task_type: String;

  let res = match task.payload.content.clone() {
    MessageContent::SaveObjects(job) => {
      task_type = "SaveObjectsJob".to_string();
      SaveObjectsJob::handle(provider, job).await
    }
    MessageContent::ParseObjectsFields(job) => {
      task_type = "ParseObjectsFields".to_string();
      ParseObjectsFieldsJob::handle(provider, job).await
    }
    MessageContent::HandleFailedCheckpoint(job) => {
      task_type = "HandleFailedCheckpoint".to_string();
      HandleFailedCheckpointJob::handle(provider, job).await
    }
    _ => {
      return Err(Error::msg("Job is not support"));
    }
  };

  if let Err(error) = res {
    if let Some(new_task) = task.next_retry() {
      all_tasks.push_back(new_task).await;
    } else {
      let _: () = redis_connection.del(&task.key).await?;
      return Err(error);
    }
  }

  let _: () = redis_connection.del(&task.key).await?;

  Ok(task_type.to_string())
}
