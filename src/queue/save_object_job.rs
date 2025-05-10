use std::{str::FromStr, sync::Arc};

use crate::{
  library::struct_resolver::StructResolver, scan_worker::AppProvider, transaction::{SuiObject, TransactionObject}, utils::object::insert_objects
};

use super::base_job::{ BaseJob, MessageContent };
use anyhow::{ Error, Result };
use async_trait::async_trait;
use move_core_types::language_storage::StructTag;
use serde::{ de::DeserializeOwned, Deserialize, Serialize };
use sui_sdk::json::MoveTypeLayout;
use sui_types::{object::Data, TypeTag};

pub struct SaveObjectsJob;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveObjectsJobPayload {
  pub objects: String,
}

#[async_trait]
impl BaseJob<SaveObjectsJobPayload> for SaveObjectsJob {
  async fn handle(provider: &AppProvider, job: SaveObjectsJobPayload) -> anyhow::Result<()> {
    let tx_objects: Vec<TransactionObject> = serde_json::from_str(&job.objects)?;
    let objects = Vec::<SuiObject>::new();
    for tx_object in tx_objects {
      match tx_object {
        TransactionObject::Struct(object, detail) => {
          if let Data::Move(move_object) = object.data.clone() {
            let move_struct_type = &move_object.type_().to_string();
            let struct_tag_res: Result<StructTag, _> = StructTag::from_str(&move_struct_type);
            let mut content = None;
            if let Ok(struct_tag) = struct_tag_res {
              let res = StructResolver::resolve_type_layout(
                provider,
                &TypeTag::Struct(Box::new(struct_tag)),
                20
              ).await;

              match res {
                Ok(data) => {
                  match data.0 {
                    MoveTypeLayout::Struct(layout) => {
                      let data = object.data.try_as_move().unwrap().to_move_struct(layout.as_ref());
                      match data {
                        Ok(d) => {
                          content = Some(serde_json::to_string(&d).unwrap());
                        }
                        _ => {}
                      }
                    }
                    _ => {}
                  };
                }
                Err(e) => {
                  eprintln!(
                    "error ================> {:?} -> {:?} -> {:?}",
                    object.id(),
                    object.type_().map(|t| t.to_string()),
                    e
                  );
                }
              }
            }
          }
        }
        TransactionObject::Package(object, detail) => {}
        TransactionObject::Display(object, detail) => {}
      }
    }

    insert_objects(provider, objects).await?;
    Ok(())
  }

  async fn dispatch(provider: &AppProvider, payload: MessageContent) -> Result<()>  {
    let mut connection = provider.redis_client().get_multiplexed_tokio_connection().await?;
    let client = Arc::clone(&provider.http_client);
    match payload {
      MessageContent::SaveObjects(_) => {
        SaveObjectsJob::send(&mut connection, client, payload).await?;
      }
      _ => {
        return Err(Error::msg("Invalid SaveObjectJob payload"));
      }
    }

    Ok(())
  }
}
