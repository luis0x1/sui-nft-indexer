use crate::{
  library::display::StoredDisplay,
  utils::{ datetime::millis_to_iso, object::is_valid_object },
  scan_worker::AppProvider,
};
use serde::{ Deserialize, Serialize };
use sui_package_resolver::Package;
use std::{collections::HashSet, fmt::Display};
use sui_types::{
  digests::TransactionDigest,
  display::{ DISPLAY_MODULE_NAME, DISPLAY_VERSION_UPDATED_EVENT_NAME },
  full_checkpoint_content::CheckpointTransaction,
  move_package::MovePackage,
  object::{ Data, Object, Owner },
  SUI_FRAMEWORK_ADDRESS,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SuiObjectStatus {
  #[serde(rename = "created")]
  Created,
  #[serde(rename = "mutated")]
  Mutated,
  #[serde(rename = "deleted")]
  Deleted,
}

impl Display for SuiObjectStatus {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let str = match self {
      Self::Created => "created".to_string(),
      Self::Mutated => "mutated".to_string(),
      Self::Deleted => "deleted".to_string(),
    };
    write!(f, "{}", str)
  }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SuiObject {
  id: String,
  owner: String,
  object_type: Option<String>,
  status: SuiObjectStatus,
  updated_at: String,
  content_bytes: Vec<u8>,
  content: Option<String>,
  version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransactionDetail {
  pub digest: TransactionDigest,
  pub confirmed_timestamp: String,
  pub status: SuiObjectStatus,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TransactionObject {
  Struct(Object, Option<TransactionDetail>),
  Package(MovePackage, Option<TransactionDetail>),
  Display(StoredDisplay, Option<TransactionDetail>),
}

impl SuiObject {
  pub fn id(&self) -> &str {
    &self.id
  }

  pub fn owner(&self) -> &str {
    &self.owner
  }

  pub fn status(&self) -> &SuiObjectStatus {
    &self.status
  }

  pub fn object_type(&self) -> &Option<String> {
    &self.object_type
  }

  pub fn updated_at(&self) -> &str {
    &self.updated_at
  }

  pub fn content(&self) -> &Option<String> {
    &self.content
  }

  pub fn content_bytes(&self) -> &Vec<u8> {
    &self.content_bytes
  }

  pub fn version(&self) -> &str {
    &self.version
  }
}

async fn process_object(
  (objects, object_map): (&mut Vec<TransactionObject>, &mut HashSet<String>),
  object: &Object,
  status: SuiObjectStatus,
  _latest_digest: &TransactionDigest,
  confirmed_timestamp: &str
) {
  let object_type = object.type_().map(|v| v.to_string());

  if let Some(ref type_str) = object_type {
    if !is_valid_object(type_str.clone()) {
      return;
    }
  } /* else {
  return;
} */

  let object_id = object.id().to_string();
  if !object_map.insert(object_id.clone()) {
    return; // Object already processed
  }

  if object_id == "0x8fd6526ba0fdfc85f94b0027ca2a00d942f91d194fe7261aab233b994cfbd5e2" {
    if let Data::Move(move_object) = object.data.clone() {
      println!("CONTENTS: {:?}", move_object.contents());
    }
  }

  if object_id == "0xdb5069875ed5eab109b16b20c599b1a8293fd6f3be4d9e9fdf59394a076c7a16" {
    if let Data::Move(move_object) = object.data.clone() {
      println!("NFT CONTENTS: {:?}", move_object.contents());
    }
    let tag = object.type_().unwrap();
    println!(
      "QUERY: {:?} {:?} {:?} {:?}",
      SUI_FRAMEWORK_ADDRESS,
      DISPLAY_VERSION_UPDATED_EVENT_NAME,
      DISPLAY_MODULE_NAME,
      tag.clone().type_params()
    );
  }

  if object_id == "0xbe1ca29fd5c79b061a387aa0cfffa7905722ac0e9895946dd5e20c0fad48ba8b" {
    if let Data::Package(package) = object.data.clone() {
      // println!("CONTRACT: {:?}", serde_json::to_string(&bird_package));
      let package_ = Package::read_from_package(&package);

      if let Ok(package) = package_ {
        if let Ok(module) = package.module("birds_nft") {
          println!("module: {:?}", module);
          // if let Ok(struct__) = module.struct_def("Config") {
          //   if let Some(struct_) = struct__ {
          //     if let MoveData::Struct(data) = struct_.data {
          //       for (name_field, type_) in data {
          //         println!("UserArchive -> {:?}: {:?}", name_field, type_);
          //       }
          //     }
          //   }
          // }
        }
      }
    }
  }

  let transaction_detail = TransactionDetail {
    digest: object.previous_transaction,
    confirmed_timestamp: confirmed_timestamp.to_string(),
    status,
  };

  match object.data.clone() {
    Data::Move(_) => {
      objects.push(TransactionObject::Struct(object.clone(), Some(transaction_detail)));
    }
    Data::Package(package_data) => {
      objects.push(TransactionObject::Package(package_data, Some(transaction_detail)));
    }
  }
}

pub async fn get_all_object_checkpoint(
  _provider: &AppProvider,
  transactions: Vec<CheckpointTransaction>,
  confirmed_at: u64
) -> Vec<TransactionObject> {
  let confirmed_timestamp = millis_to_iso(confirmed_at);
  let mut object_map = HashSet::new();
  let mut objects: Vec<TransactionObject> = Vec::with_capacity(transactions.len() * 2); // Pre-allocate with estimated capacity
  // Helper closure to process an object

  for transaction in transactions.iter().rev() {
    transaction.events.iter().for_each(|events| {
      let displays: Vec<TransactionObject> = events.data
        .iter()
        .flat_map(StoredDisplay::try_from_event)
        .map(|display|
          TransactionObject::Display(
            display,
            Some(TransactionDetail {
              digest: transaction.transaction.digest().clone(),
              confirmed_timestamp: confirmed_timestamp.to_string(),
              status: SuiObjectStatus::Mutated,
            })
          )
        )
        .collect();

      objects.extend(displays);
    });

    for obj in transaction.removed_objects_pre_version() {
      process_object(
        (&mut objects, &mut object_map),
        obj,
        SuiObjectStatus::Deleted,
        transaction.transaction.digest(),
        &confirmed_timestamp
      ).await;
    }

    for (obj, _) in transaction.changed_objects() {
      process_object(
        (&mut objects, &mut object_map),
        obj,
        SuiObjectStatus::Mutated,
        transaction.transaction.digest(),
        &confirmed_timestamp
      ).await;
    }

    for obj in transaction.created_objects() {
      process_object(
        (&mut objects, &mut object_map),
        obj,
        SuiObjectStatus::Created,
        transaction.transaction.digest(),
        &confirmed_timestamp
      ).await;
    }
  }

  objects
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
