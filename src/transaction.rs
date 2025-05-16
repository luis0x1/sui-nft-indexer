use crate::{
  library::display::StoredDisplay,
  utils::{ datetime::millis_to_iso, object::is_valid_object },
  scan_worker::AppProvider,
};
use serde::{ Deserialize, Serialize };
use std::{ collections::HashSet, fmt::Display };
use sui_types::{
  digests::TransactionDigest,
  full_checkpoint_content::CheckpointTransaction,
  move_package::MovePackage,
  object::{ Data, Object },
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
  pub id: String,
  pub owner: String,
  pub object_type: Option<String>,
  pub status: SuiObjectStatus,
  pub updated_at: String,
  pub content_bytes: Vec<u8>,
  pub content: Option<String>,
  pub display: Option<String>,
  pub version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransactionDetail {
  pub digest: TransactionDigest,
  pub confirmed_timestamp: String,
  pub status: SuiObjectStatus,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TransactionObject {
  Struct(Object, TransactionDetail),
  Package(MovePackage, TransactionDetail),
  Display(StoredDisplay, TransactionDetail),
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

  pub fn display(&self) -> &Option<String> {
    &self.display
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

  let transaction_detail = TransactionDetail {
    digest: object.previous_transaction,
    confirmed_timestamp: confirmed_timestamp.to_string(),
    status,
  };

  match object.data.clone() {
    Data::Move(_) => {
      objects.push(TransactionObject::Struct(object.clone(), transaction_detail));
    }
    Data::Package(package_data) => {
      objects.push(TransactionObject::Package(package_data, transaction_detail));
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
          TransactionObject::Display(display, TransactionDetail {
            digest: transaction.transaction.digest().clone(),
            confirmed_timestamp: confirmed_timestamp.to_string(),
            status: SuiObjectStatus::Mutated,
          })
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
