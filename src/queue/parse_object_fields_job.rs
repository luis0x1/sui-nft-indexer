use std::{ collections::VecDeque, str::FromStr, sync::Arc };

use crate::{
  library::{ package_resolve::PackageResolver, struct_resolver::StructResolver },
  scan_worker::AppProvider,
  utils::object::{ is_valid_object, update_objects_fields, UpdateObjectArgs },
};

use super::base_job::{ BaseJob, MessageContent, Task };
use anyhow::{ Error, Result };
use async_trait::async_trait;
use move_core_types::language_storage::StructTag;
use serde::{ Deserialize, Serialize };
use sui_sdk::json::MoveTypeLayout;
use sui_types::{ object::{ Data, Object }, TypeTag };
use tokio::sync::RwLock;

pub struct ParseObjectsFieldsJob;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseObjectsFieldsJobPayload {
  pub objects: String,
}

#[async_trait]
impl BaseJob<ParseObjectsFieldsJobPayload> for ParseObjectsFieldsJob {
  async fn handle(provider: &AppProvider, job: ParseObjectsFieldsJobPayload) -> anyhow::Result<()> {
    let tx_objects: Vec<Object> = serde_json::from_str(&job.objects)?;
    let mut objects = Vec::<UpdateObjectArgs>::new();

    for object in tx_objects {
      if let Data::Move(move_object) = object.data.clone() {
        let object_type = object.type_().map(|v| v.to_string());

        if let Some(ref type_str) = object_type {
          if !is_valid_object(type_str.clone()) {
            continue;
          }
        }
        let move_struct_type = &move_object.type_().to_string();
        let struct_tag_res: Result<StructTag, _> = StructTag::from_str(&move_struct_type);
        let mut content = None;
        let mut display = None;

        if let Ok(struct_tag) = struct_tag_res {
          let res = StructResolver::resolve_type_layout(
            provider,
            &TypeTag::Struct(Box::new(struct_tag)),
            false,
            20
          ).await;

          if let Ok(data) = res {
            if let MoveTypeLayout::Struct(layout) = data.0 {
              let data = object.data.try_as_move().unwrap().to_move_struct(layout.as_ref());
              match data {
                Ok(d) => {
                  content = Some(serde_json::to_string(&d).unwrap());

                  if
                    let Ok(display_template) = PackageResolver::get_display(
                      provider,
                      &move_struct_type,
                      &d
                    ).await
                  {
                    display = Some(display_template);
                  }
                }
                _ => {}
              }
            }
          } else {
            eprintln!(
              "[ParseObjectsFieldsJob]: error ================> {:?} -> {:?} -> {:?}",
              object.id(),
              object.type_().map(|t| t.to_string()),
              res.unwrap_err()
            );
          }
        }

        objects.push(UpdateObjectArgs {
          id: object.id().to_string(),
          version: object.version().value(),
          display: display.unwrap_or("{}".to_string()),
          fields: content.unwrap_or("{}".to_string()),
        });
      }
    }

    update_objects_fields(provider, objects).await?;

    Ok(())
  }

  async fn dispatch(provider: &AppProvider, payload: MessageContent) -> Result<()> {
    let mut connection = provider.worker_client.get_multiplexed_tokio_connection().await?;
    let client = Arc::clone(&provider.http_client);
    match payload {
      MessageContent::ParseObjectsFields(_) => {
        ParseObjectsFieldsJob::send(&mut connection, client, payload).await?;
      }
      _ => {
        return Err(Error::msg("[ParseObjectsFieldsJob]: Invalid payload"));
      }
    }

    Ok(())
  }

  async fn dispatch_current(
    provider: &AppProvider,
    all_tasks: Arc<RwLock<VecDeque<Task>>>,
    payload: MessageContent
  ) -> Result<()> {
    let mut connection = provider.worker_client.get_multiplexed_tokio_connection().await?;

    match payload {
      MessageContent::ParseObjectsFields(_) => {
        ParseObjectsFieldsJob::send_current(&mut connection, all_tasks, payload).await?;
      }
      _ => {
        return Err(Error::msg("[ParseObjectsFieldsJob]: Invalid payload"));
      }
    }

    Ok(())
  }
}

// fn get_object_owner_address(object: &Object) -> String {
//   match object.owner() {
//     Owner::AddressOwner(addr) => addr.to_string(),
//     Owner::ObjectOwner(addr) => addr.to_string(),
//     Owner::Immutable => "Immutable".to_string(),
//     Owner::Shared { initial_shared_version } => format!("Shared({})", initial_shared_version),
//     Owner::ConsensusV2 { start_version: _, authenticator } => {
//       authenticator.as_single_owner().to_string()
//     }
//   }
// }
