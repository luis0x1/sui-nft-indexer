use std::sync::Arc;

use crate::{ scan_worker::AppProvider, utils::object::insert_objects };

use super::base_job::{ BaseJob, MessageContent };
use anyhow::{ Error, Result };
use async_trait::async_trait;
use serde::{ de::DeserializeOwned, Deserialize, Serialize };
use sui_types::object::{ Data, Object };

pub struct SaveObjectsJob;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveObjectsJobPayload {
  pub objects: Vec<String>,
}

#[async_trait]
impl BaseJob<SaveObjectsJobPayload> for SaveObjectsJob {
  async fn handle(provider: &AppProvider, job: SaveObjectsJobPayload) -> anyhow::Result<()> {
    let objects: Vec<Object> = job.objects
      .iter()
      .filter_map(|object_raw| {
        let object_res = serde_json::from_str(&object_raw);
        object_res.ok()
      })
      .collect();

    for object in objects {
      if let Data::Move(move_object) = object.data.clone() {
        let move_struct_type = &move_object.type_().to_string();
        let struct_tag_res: Result<StructTag, _> = StructTag::from_str(&move_struct_type);

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

    insert_objects(provider, objects).await?;
    Ok(())
  }

  async fn dispatch<R>(provider: &AppProvider, payload: &R) -> Result<()> where R: DeserializeOwned {
    let mut connection = provider.redis_client().get_multiplexed_tokio_connection().await?;
    let client = Arc::clone(&provider.http_client);
    match payload {
      MessageContent::SaveObjects(_) => {
        SaveObjectsJob::send(&mut connection, client, serde_json::to_string(payload)).await?;
      }
      _ => {
        return Err(Error::msg("Invalid SaveObjectJob payload"));
      }
    }

    Ok(())
  }
}
