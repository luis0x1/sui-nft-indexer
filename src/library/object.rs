use anyhow::Result;
use serde::{ Deserialize, Serialize };
use sui_types::{ base_types::SuiAddress, object::Data };

use crate::{ constants::{ is_in_blocklist }, scan_worker::AppProvider, transaction::SuiObject };

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
  if is_in_blocklist(&object_type) {
    return false;
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

pub async fn insert_objects(provider: &AppProvider, objects: Vec<SuiObject>) -> Result<()> {
  // Process objects in batches to avoid huge SQL queries
  const BATCH_SIZE: usize = 1000;

  let pg_client = provider.pg_client();

  for chunk in objects.chunks(BATCH_SIZE) {
    let values = chunk
      .iter()
      .map(|object| {
        format!(
          r#"('{}', '{}', {}, '{}'::"ObjectStatus", '{}', $${}$$::jsonb, $${}$$::jsonb, '{}'::bigint, '{}'::timestamp, '{}'::timestamp)"#,
          object.id(),
          object.owner(),
          parse_object_type_to_sql(object),
          object.status().to_string(),
          "[]",
          parse_object_content_to_sql(object),
          parse_object_display_to_sql(object),
          object.version(),
          object.updated_at(),
          object.updated_at()
        )
      })
      .collect::<Vec<String>>()
      .join(",");

    let query = format!(
      r#"
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
      "#,
      values,
      "{}",
      "{}"
    );

    pg_client.query(query.as_str(), &[]).await?;
  }

  Ok(())
}

pub struct UpdateObjectArgs {
  pub id: String,
  pub version: u64,
  pub fields: String,
  pub display: String,
  pub is_remove: bool,
}

pub async fn update_object_fields(provider: &AppProvider, args: UpdateObjectArgs) -> Result<()> {
  let pg_client = &provider.pg_client;

  if args.is_remove {
    let query = r#"DELETE FROM objects
        WHERE id = $1"#;
    pg_client.query(query, &[&args.id]).await?;
  } else {
    let query = format!(
      r#"UPDATE objects
      SET fields = $${}$$::jsonb,
          display = $${}$$::jsonb
      WHERE id = $1 AND version = $2::bigint"#,
      &args.fields,
      &args.display
    );
    pg_client.query(query.as_str(), &[&args.id, &args.version.to_string()]).await?;
  }

  Ok(())
}

pub async fn update_objects_fields(
  provider: &AppProvider,
  args: Vec<UpdateObjectArgs>
) -> Result<()> {
  const BATCH_SIZE: usize = 1000;
  let pg_client = &provider.pg_client;

  for chunk in args.chunks(BATCH_SIZE) {
    let update_objects = chunk.iter().filter(|o| !o.is_remove);
    let remove_objects = chunk.iter().filter(|o| o.is_remove);

    let _updating_object = {
      let values = update_objects
        .map(|object| {
          format!(
            r#"('{}', '{}'::bigint, $${}$$::jsonb, $${}$$::jsonb)"#,
            object.id,
            object.version,
            object.fields,
            object.display
          )
        })
        .collect::<Vec<String>>();

      if values.len() > 0 {
        let query = format!(
          r#"UPDATE objects
          SET fields = v.fields,
              display = v.display
          FROM (VALUES {}) v(id, version, fields, display)
          WHERE objects.id = v.id AND objects.version = v.version"#,
          values.join(",")
        );

        pg_client.query(query.as_str(), &[]).await?;
      }
    };

    let _removing_object = {
      let values = remove_objects.map(|object| object.id.as_str()).collect::<Vec<&str>>();

      if values.len() > 0 {
        pg_client.query(r#"DELETE FROM objects
        WHERE id = ANY($1)"#, &[&values]).await?;
      }
    };
  }

  Ok(())
}
