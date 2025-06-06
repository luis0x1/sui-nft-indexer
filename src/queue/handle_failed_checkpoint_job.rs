use std::{ collections::{ HashMap, VecDeque }, sync::Arc };

use crate::{
  library::sui_client::SuiClientProvider,
  scan_worker::AppProvider,
  string,
  transaction::{ SuiObject, SuiObjectStatus },
  utils::datetime::millis_to_iso,
};

use super::{ base_job::{ BaseJob, MessageContent, Task } };
use anyhow::Result;
use async_trait::async_trait;
use serde::{ Deserialize, Serialize };
use sui_sdk::rpc_types::{
  CheckpointId,
  ObjectChange,
  SuiGetPastObjectRequest,
  SuiObjectData,
  SuiObjectDataOptions,
  SuiParsedData,
  SuiTransactionBlockResponseOptions,
};
use sui_types::{ base_types::{ ObjectID, SequenceNumber }, object::Owner };
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandleFailedCheckpointJob(pub u64);

const BATCH_SIZE: usize = 50;

#[async_trait]
impl BaseJob for HandleFailedCheckpointJob {
  async fn handle(provider: &AppProvider, job: Self) -> anyhow::Result<()> {
    let checkpoint_num = job.0;
    let sui_client = provider.sui_client.clone();
    let checkpoint = sui_client.open_client(|client| async move {
      Ok(client.read_api().get_checkpoint(CheckpointId::SequenceNumber(checkpoint_num)).await?)
    }).await?;
    let mut objects: Vec<SuiObject> = Vec::new();
    let timestamp = millis_to_iso(checkpoint.timestamp_ms);

    for transaction_digests in checkpoint.transactions.chunks(BATCH_SIZE) {
      let digests = &Arc::new(transaction_digests.to_vec());
      let transactions_data = sui_client.open_client(|client| async move {
        Ok(
          client
            .read_api()
            .multi_get_transactions_with_options(
              digests.as_ref().clone(),
              SuiTransactionBlockResponseOptions::default()
                .with_object_changes()
                .with_effects()
                .with_events()
            ).await?
        )
      }).await?;

      let objects_change: Vec<(ObjectChange, String)> = transactions_data
        .iter()
        .flat_map(|tx| {
          let Some(object_change) = tx.object_changes.clone() else {
            return vec![];
          };

          object_change
            .iter()
            .map(|obj| (obj.clone(), timestamp.clone()))
            .collect()
        })
        .fold(HashMap::<ObjectID, (ObjectChange, String)>::new(), |mut prev, (obj, timestamp)| {
          let object_id = obj.object_id();
          if prev.contains_key(&object_id) {
            let (exist_value, _) = &prev[&object_id];
            let old_version = object_change_to_version(&exist_value);
            let new_version = object_change_to_version(&obj);

            if new_version > old_version {
              prev.insert(object_id, (obj, timestamp));
            }
          } else {
            prev.insert(object_id, (obj, timestamp));
          }
          prev
        })
        .values()
        .cloned()
        .collect();

      let all_objects = get_transaction_objects(&sui_client, &objects_change).await?;
      objects.extend(all_objects);
    }

    objects
      .iter()
      .rev()
      .for_each(|o| {
        println!("object: {:?}", o);
      });
    Ok(())
  }

  async fn dispatch(self, provider: &AppProvider) -> Result<()> {
    let mut connection = provider.worker_client.get_multiplexed_tokio_connection().await?;
    let client = Arc::clone(&provider.http_client);
    HandleFailedCheckpointJob::send(
      &mut connection,
      client,
      MessageContent::HandleFailedCheckpoint(self)
    ).await?;

    Ok(())
  }

  async fn dispatch_current(
    self,
    provider: &AppProvider,
    all_tasks: Arc<RwLock<VecDeque<Task>>>
  ) -> Result<()> {
    let mut connection = provider.worker_client.get_multiplexed_tokio_connection().await?;

    HandleFailedCheckpointJob::send_current(
      &mut connection,
      all_tasks,
      MessageContent::HandleFailedCheckpoint(self)
    ).await?;

    Ok(())
  }
}

fn move_struct_to_content(
  object_data: &SuiObjectData
) -> (Option<String>, Option<String>, Option<String>, String) {
  let content = get_object_content(object_data).ok();

  let display = get_object_display(object_data).ok();

  let struct_type = object_data
    .object_type()
    .as_ref()
    .map(|t| t.to_string())
    .ok();

  let owner = object_data.owner
    .as_ref()
    .map(|owner| get_object_owner_address(owner))
    .unwrap_or("Shared".to_string());

  (content, display, struct_type, owner)
}

async fn get_transaction_objects(
  sui_client: &SuiClientProvider,
  objects: &Vec<(ObjectChange, String)>
) -> Result<Vec<SuiObject>> {
  let object_ids: Vec<SuiGetPastObjectRequest> = objects
    .iter()
    .map(|(o, _)| SuiGetPastObjectRequest {
      object_id: o.object_id(),
      version: object_change_to_version(o),
    })
    .collect();
  let mut object_map: HashMap<ObjectID, SuiObjectData> = HashMap::new();

  for chunk in object_ids.chunks(BATCH_SIZE) {
    let objects_res = sui_client.open_client(|client| async move {
      Ok(
        client
          .read_api()
          .try_multi_get_parsed_past_object(
            chunk.to_vec(),
            SuiObjectDataOptions::new().with_display().with_content().with_type().with_owner()
          ).await?
      )
    }).await?;

    object_map.extend(
      objects_res.iter().flat_map(|obj| {
        let object_res = obj.object();
        let Ok(object_data) = object_res else {
          return None;
        };

        return Some((object_data.object_id, object_data.clone()));
      })
    );
  }

  Ok(
    objects
      .iter()
      .flat_map(|(object_change, confirmed_at)| {
        let Some(object_data) = object_map.get(&object_change.object_id()) else {
          return Some(SuiObject {
            id: string!(object_change.object_id()),
            content_bytes: vec![],
            content: Some(string!("{}")),
            display: Some(string!("{}")),
            object_type: object_change_to_struct_tag(object_change),
            owner: object_change_to_owner(object_change),
            status: SuiObjectStatus::Deleted,
            updated_at: confirmed_at.clone(),
            version: object_change_to_version(object_change).to_string(),
          });
        };

        let (content, display, object_type, owner) = move_struct_to_content(&object_data);

        Some(SuiObject {
          id: object_data.object_id.to_string(),
          content_bytes: vec![],
          content,
          display,
          object_type,
          owner,
          status: object_change_to_status(&object_change),
          updated_at: confirmed_at.clone(),
          version: object_data.version.to_string(),
        })
      })
      .collect()
  )
}

fn object_change_to_status(obj: &ObjectChange) -> SuiObjectStatus {
  match obj {
    ObjectChange::Created { .. } => SuiObjectStatus::Created,
    ObjectChange::Mutated { .. } => SuiObjectStatus::Mutated,
    ObjectChange::Deleted { .. } => SuiObjectStatus::Deleted,
    ObjectChange::Transferred { .. } => SuiObjectStatus::Mutated,
    ObjectChange::Wrapped { .. } => SuiObjectStatus::Mutated,
    ObjectChange::Published { .. } => SuiObjectStatus::Mutated,
  }
}

fn object_change_to_struct_tag(obj: &ObjectChange) -> Option<String> {
  match obj {
    ObjectChange::Created { object_type, .. } => Some(string!(object_type)),
    ObjectChange::Mutated { object_type, .. } => Some(string!(object_type)),
    ObjectChange::Deleted { object_type, .. } => Some(string!(object_type)),
    ObjectChange::Transferred { object_type, .. } => Some(string!(object_type)),
    ObjectChange::Wrapped { object_type, .. } => Some(string!(object_type)),
    ObjectChange::Published { .. } => None,
  }
}

fn object_change_to_owner(obj: &ObjectChange) -> String {
  match obj {
    ObjectChange::Created { owner, .. } => string!(owner),
    ObjectChange::Mutated { owner, .. } => string!(owner),
    ObjectChange::Deleted { sender, .. } => string!(sender),
    ObjectChange::Transferred { recipient, .. } => string!(recipient),
    ObjectChange::Wrapped { .. } => string!("Wrapped"),
    ObjectChange::Published { .. } => string!("Immutable"),
  }
}

fn object_change_to_version(obj: &ObjectChange) -> SequenceNumber {
  match obj {
    ObjectChange::Created { version, .. } => version.clone(),
    ObjectChange::Mutated { version, .. } => version.clone(),
    ObjectChange::Deleted { version, .. } => version.clone(),
    ObjectChange::Transferred { version, .. } => version.clone(),
    ObjectChange::Wrapped { version, .. } => version.clone(),
    ObjectChange::Published { version, .. } => version.clone(),
  }
}

fn get_object_owner_address(owner: &Owner) -> String {
  match owner {
    Owner::AddressOwner(addr) => addr.to_string(),
    Owner::ObjectOwner(addr) => addr.to_string(),
    Owner::Immutable => "Immutable".to_string(),
    Owner::Shared { initial_shared_version } => format!("Shared({})", initial_shared_version),
    Owner::ConsensusAddressOwner { start_version: _, owner } => { owner.to_string() }
  }
}

fn get_object_content(object: &SuiObjectData) -> Result<String> {
  let Some(content) = object.content.as_ref() else {
    return Ok(string!("{}"));
  };

  Ok(match content {
    SuiParsedData::MoveObject(object) => serde_json::to_string(&object.fields)?,
    _ => string!("{}"),
  })
}

fn get_object_display(object: &SuiObjectData) -> Result<String> {
  let Some(display) = object.display.as_ref() else {
    return Ok(string!("{}"));
  };

  Ok(match display.data.as_ref() {
    Some(object) => serde_json::to_string(&object)?,
    _ => string!("{}"),
  })
}
