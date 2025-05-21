use std::{ collections::VecDeque, str::FromStr, sync::Arc };

use crate::{
  library::{
    blacklist_resolver::BlacklistResolver,
    display::StoredDisplay,
    object::{ get_object_content_bytes, insert_objects, is_valid_object },
    package_resolve::PackageResolver,
    struct_resolver::StructResolver,
  },
  scan_worker::AppProvider,
  transaction::{ SuiObject, TransactionObject },
  utils::error::OBJECT_NOT_FOUND_LOCAL,
};

use super::{
  base_job::{ BaseJob, MessageContent, Task },
  parse_object_fields_job::{ ParseObjectsFieldsJob, ParseObjectsFieldsJobPayload },
};
use anyhow::{ Error, Result };
use async_trait::async_trait;
use move_core_types::{ annotated_value::MoveStruct, language_storage::StructTag };
use serde::{ Deserialize, Serialize };
use sui_sdk::{ json::MoveTypeLayout, rpc_types::{ SuiMoveStruct, SuiParsedMoveObject } };
use sui_types::{
  digests::TransactionDigest,
  move_package::MovePackage,
  object::{ Data, MoveObject, Object, Owner },
  TypeTag,
};
use tokio::sync::RwLock;

pub struct SaveObjectsJob;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveObjectsJobPayload {
  pub objects: String,
}

#[async_trait]
impl BaseJob<SaveObjectsJobPayload> for SaveObjectsJob {
  async fn handle(provider: &AppProvider, job: SaveObjectsJobPayload) -> anyhow::Result<()> {
    let tx_objects: Vec<TransactionObject> = serde_json::from_str(&job.objects)?;
    let mut objects = Vec::<SuiObject>::new();
    let mut objects_failed = Vec::<Object>::new();
    let mut packages = Vec::<(MovePackage, Option<TransactionDigest>)>::new();
    let mut displays = Vec::<(StoredDisplay, Option<TransactionDigest>)>::new();

    tx_objects.iter().for_each(|tx_object| {
      match tx_object {
        TransactionObject::Package(package, detail) => {
          packages.push((package.clone(), Some(detail.digest)));
        }
        TransactionObject::Display(display, detail) => {
          displays.push((display.clone(), Some(detail.digest)));
        }
        _ => {}
      }
    });

    PackageResolver::save_packages(provider, packages).await?;
    PackageResolver::save_displays(provider, displays).await?;

    for tx_object in tx_objects {
      match tx_object {
        TransactionObject::Struct(object, detail) => {
          if let Data::Move(move_object) = object.data.clone() {
            let object_type = object.type_().map(|v| v.to_string());

            if let Some(ref type_str) = object_type {
              if
                !is_valid_object(type_str.clone()) ||
                BlacklistResolver::is_blacklist(provider, type_str).await
              {
                continue;
              }
            }
            let move_struct_type = &move_object.type_().to_string();
            let struct_tag_res: Result<StructTag, _> = StructTag::from_str(&move_struct_type);
            let mut content = None;
            let mut display = None;
            let content_bytes = get_object_content_bytes(&object.data);

            if let Ok(struct_tag) = struct_tag_res {
              let res = StructResolver::resolve_type_layout(
                provider,
                &TypeTag::Struct(Box::new(struct_tag)),
                true,
                20
              ).await;

              if let Ok(data) = res {
                if let MoveTypeLayout::Struct(layout) = data.0 {
                  if let Some(move_object) = object.data.try_as_move() {
                    let data = move_object.to_move_struct(layout.as_ref());
                    match data {
                      Ok(layout) => {
                        let content_parsed = move_struct_to_content(move_object, layout.clone());
                        content = Some(serde_json::to_string(&content_parsed.fields).unwrap());

                        if
                          let Ok(display_template) = PackageResolver::get_display(
                            provider,
                            &move_struct_type,
                            &layout
                          ).await
                        {
                          display = Some(display_template);
                        }
                      }
                      _ => {}
                    }
                  }
                }
              } else {
                let error = res.unwrap_err();

                if error.to_string().contains(OBJECT_NOT_FOUND_LOCAL) {
                  objects_failed.push(object.clone());
                  println!("[SaveObjectsJob]: add object to [ParseObjectsFieldsJob]");
                } else {
                  eprintln!(
                    "[SaveObjectsJob]: error ================> {:?} -> {:?} -> {:?}",
                    object.id(),
                    object.type_().map(|t| t.to_string()),
                    error
                  );
                }
              }
            }

            if
              object.id().to_string() ==
              "0x15a9b5d245548e4495aad55383414c9247145aa1ac266f271ed703e4bbc8f51d"
            {
              println!("==========================> {:?} {:?}", content, display);
            }

            if (content.is_none() && display.is_none()) || (content.is_some() && display.is_some()) {
              objects.push(SuiObject {
                id: object.id().to_string(),
                owner: get_object_owner_address(&object),
                object_type,
                status: detail.status,
                updated_at: detail.confirmed_timestamp,
                content_bytes,
                content,
                display,
                version: object.version().to_string(),
              });
            }
          }
        }
        _ => {}
      }
    }

    insert_objects(provider, objects).await?;
    if objects_failed.len() > 0 {
      ParseObjectsFieldsJob::dispatch(
        provider,
        MessageContent::ParseObjectsFields(ParseObjectsFieldsJobPayload {
          objects: serde_json::to_string(&objects_failed)?,
        })
      ).await?;
    }

    Ok(())
  }

  async fn dispatch(provider: &AppProvider, payload: MessageContent) -> Result<()> {
    let mut connection = provider.worker_client.get_multiplexed_tokio_connection().await?;
    let client = Arc::clone(&provider.http_client);
    match payload {
      MessageContent::SaveObjects(_) => {
        SaveObjectsJob::send(&mut connection, client, payload).await?;
      }
      _ => {
        return Err(Error::msg("[SaveObjectJob]: Invalid payload"));
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
      MessageContent::SaveObjects(_) => {
        SaveObjectsJob::send_current(&mut connection, all_tasks, payload).await?;
      }
      _ => {
        return Err(Error::msg("[SaveObjectJob]: Invalid payload"));
      }
    }

    Ok(())
  }
}

fn get_object_owner_address(object: &Object) -> String {
  match object.owner() {
    Owner::AddressOwner(addr) => addr.to_string(),
    Owner::ObjectOwner(addr) => addr.to_string(),
    Owner::Immutable => "Immutable".to_string(),
    Owner::Shared { initial_shared_version } => format!("Shared({})", initial_shared_version),
    Owner::ConsensusV2 { start_version: _, authenticator } => {
      authenticator.as_single_owner().to_string()
    }
  }
}

pub fn move_struct_to_content(object: &MoveObject, move_struct: MoveStruct) -> SuiParsedMoveObject {
  let sui_move_struct = move_struct.into();
  if let SuiMoveStruct::WithTypes { type_, fields } = sui_move_struct {
    SuiParsedMoveObject {
      type_,
      has_public_transfer: object.has_public_transfer(),
      fields: SuiMoveStruct::WithFields(fields),
    }
  } else {
    SuiParsedMoveObject {
      type_: object.type_().clone().into(),
      has_public_transfer: object.has_public_transfer(),
      fields: sui_move_struct,
    }
  }
}
