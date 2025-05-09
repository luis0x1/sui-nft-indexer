use serde::{ Deserialize, Serialize };
use sui_types::{ base_types::SuiAddress, object::Data };

use crate::{ env::get_env, transaction::SuiObject, scan_worker::AppProvider };

use super::error::{ unwrap, AppError };

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum ObjectType {
  WormNFT(WormNftContent),
  BirdNFT(BirdNftContent),
  Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BirdNftContent {
  id: SuiAddress,
  xid: u64,
  object_type: u16,
  sub_type: u8,
  gen_id: u16,
  mating_left: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WormNftContent {
  id: SuiAddress,
  xid: u128,
  object_type: u16,
  sub_type: u16,
  rare: u16,
  value: u64,
}

pub fn is_valid_object(object_type: String) -> bool {
  let worm_nft_type = get_env().worm_nft_type;
  if object_type == worm_nft_type {
    return true;
  }

  true
}

pub fn get_object_content_bytes(data: &Data) -> Vec<u8> {
  match data {
    Data::Move(move_object) => move_object.contents().to_vec(),
    Data::Package(_) => { Vec::new() }
  }
}

fn parse_object_type_to_sql(object: &SuiObject) -> String {
  match object.object_type() {
    Some(v) => "'".to_string() + v.as_str() + "'",
    None => "NULL".to_string(),
  }
}

fn parse_object_content_to_sql(object: &SuiObject) -> String {
  let default_value = "{}".to_string();
  match object.content() {
    Some(v) => serde_json::to_string(&v).unwrap_or(default_value),
    None => default_value,
  }
}

fn parse_vector_to_sql(data: &Vec<u8>) -> String {
  format!("{:?}", data)
}

pub async fn insert_objects(
  provider: &AppProvider,
  objects: Vec<SuiObject>
) -> Result<(), AppError> {
  // Process objects in batches to avoid huge SQL queries
  const BATCH_SIZE: usize = 1000;

  let pg_client = provider.pg_client();

  for chunk in objects.chunks(BATCH_SIZE) {
    let values = chunk
      .iter()
      .map(|object| {
        format!(
          r#"('{}', '{}', {}, '{}'::"ObjectStatus", '{}', '{}'::jsonb, '{}'::bigint, '{}'::timestamp, '{}'::timestamp)"#,
          object.id(),
          object.owner(),
          parse_object_type_to_sql(object),
          object.status().to_string(),
          parse_vector_to_sql(object.content_bytes()),
          parse_object_content_to_sql(object),
          object.version(),
          object.updated_at(),
          object.updated_at()
        )
      })
      .collect::<Vec<String>>()
      .join(",");

    let query =
      format!(r#"
      INSERT INTO objects (id, owner, type, status, field_raw, fields, version, updated_at, created_at)
      VALUES {}
      ON CONFLICT (id)
      DO UPDATE
      SET owner = EXCLUDED.owner,
          status = EXCLUDED.status,
          field_raw = EXCLUDED.field_raw,
          fields = EXCLUDED.fields,
          version = EXCLUDED.version,
          updated_at = EXCLUDED.updated_at
      WHERE objects.updated_at < EXCLUDED.updated_at
      "#, values);

    let res = pg_client.query(query.as_str(), &[]).await;
    unwrap(res)?;
  }

  Ok(())
}
