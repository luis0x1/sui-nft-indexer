use anyhow::Result;
use serde::{ Deserialize, Serialize };
use sui_types::{ base_types::SuiAddress, object::Data };

use crate::{ env::get_env, scan_worker::AppProvider, transaction::SuiObject };

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

fn parse_object_display_to_sql(object: &SuiObject) -> String {
  let default_value = "{}".to_string();
  match object.display() {
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
) -> Result<()> {
  // Process objects in batches to avoid huge SQL queries
  const BATCH_SIZE: usize = 1000;

  let pg_client = provider.pg_client();

  for chunk in objects.chunks(BATCH_SIZE) {
    let values = chunk
      .iter()
      .map(|object| {
        format!(
          r#"('{}', '{}', {}, '{}'::"ObjectStatus", '{}', '{}'::jsonb, '{}'::jsonb, '{}'::bigint, '{}'::timestamp, '{}'::timestamp)"#,
          object.id(),
          object.owner(),
          parse_object_type_to_sql(object),
          object.status().to_string(),
          parse_vector_to_sql(object.content_bytes()),
          parse_object_content_to_sql(object),
          parse_object_display_to_sql(object),
          object.version(),
          object.updated_at(),
          object.updated_at()
        )
      })
      .collect::<Vec<String>>()
      .join(",");

    let query =
      format!(r#"
      INSERT INTO objects (id, owner, type, status, field_raw, fields, display, version, updated_at, created_at)
      VALUES {}
      ON CONFLICT (id)
      DO UPDATE
      SET owner = EXCLUDED.owner,
          status = EXCLUDED.status,
          field_raw = EXCLUDED.field_raw,
          fields = CASE WHEN EXCLUDED.fields <> '{}'::jsonb THEN EXCLUDED.fields ELSE objects.fields END,
          display = CASE WHEN EXCLUDED.display <> '{}'::jsonb THEN EXCLUDED.display ELSE objects.display END,
          version = EXCLUDED.version,
          updated_at = EXCLUDED.updated_at
      WHERE objects.updated_at < EXCLUDED.updated_at
      "#, values, "{}", "{}");

    pg_client.query(query.as_str(), &[]).await?;
  }

  Ok(())
}

pub struct UpdateObjectArgs {
  pub id: String,
  pub version: u64,
  pub fields: String,
  pub display: String,
}

pub async fn update_object_fields(
  provider: &AppProvider,
  args: UpdateObjectArgs
) -> Result<()> {
  let pg_client = &provider.pg_client;
  let query =
    r#"UPDATE objects
      SET fields = $3::jsonb,
          display = $4::jsonb
      WHERE id = $1 AND version = $2"#;

  pg_client.query(
    query,
    &[&args.id, &(args.version as i64), &args.fields, &args.display]
  ).await?;

  Ok(())
}

pub async fn update_objects_fields(
  provider: &AppProvider,
  args: Vec<UpdateObjectArgs>
) -> Result<()> {
  const BATCH_SIZE: usize = 1000;
  let pg_client = &provider.pg_client;

  for chunk in args.chunks(BATCH_SIZE) {
    let values = chunk
      .iter()
      .map(|object| {
        format!(
          r#"('{}', '{}'::bigint, '{}'::jsonb, '{}'::jsonb)"#,
          object.id,
          object.version,
          object.fields,
          object.display
        )
      })
      .collect::<Vec<String>>()
      .join(",");

    let query = format!(r#"UPDATE objects
          SET fields = v.fields,
              display = v.display
          FROM (VALUES {}) v(id, version, fields, display)
          WHERE objects.id = v.id AND objects.version = v.version"#, values);

    pg_client.query(query.as_str(), &[]).await?;
  }

  Ok(())
}
