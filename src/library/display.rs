use std::str::FromStr;

use anyhow::{ anyhow, Result };
use move_core_types::{annotated_value::{ MoveStruct, MoveValue }, language_storage::StructTag};
use serde::{ Deserialize, Serialize };
use sui_sdk::rpc_types::{ DisplayFieldsResponse, SuiMoveStruct, SuiMoveValue, SuiMoveVariant };
use sui_types::{
  base_types::ObjectID,
  collection_types::VecMap,
  display::DisplayVersionUpdatedEvent,
  error::SuiObjectResponseError,
};

use super::package_resolve::DisplayObject;
const MAX_DISPLAY_NESTED_LEVEL: usize = 20;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StoredDisplay {
  pub object_type: String,
  pub id: ObjectID,
  pub version: i16,
  pub bcs: Vec<u8>,
  pub fields: VecMap<String, String>,
}

impl StoredDisplay {
  pub fn try_from_event(event: &sui_types::event::Event) -> Option<Self> {
    let (ty, display_event) = DisplayVersionUpdatedEvent::try_from_event(event)?;

    Some(Self {
      object_type: ty.to_canonical_string(/* with_prefix */ true),
      id: display_event.id.bytes,
      version: display_event.version as i16,
      bcs: [0u8; 0].to_vec(),
      fields: display_event.fields,
    })
  }

  pub fn try_from_display_object(struct_type: &str, object: &DisplayObject) -> Option<Self> {
    let Ok(struct_tag) = StructTag::from_str(struct_type) else {
      return None;
    };

    let Some(object_type) = DisplayVersionUpdatedEvent::inner_type(&struct_tag) else {
      return None;
    };

    Some(Self {
      object_type: object_type.to_string(),
      id: object.id.object_id().clone(),
      version: object.version as i16,
      bcs: [0u8; 0].to_vec(),
      fields: object.fields.clone(),
    })
  }

  pub fn get_rendered_fields(
    fields: &VecMap<String, String>,
    move_struct: &MoveStruct
  ) -> Result<DisplayFieldsResponse> {
    let sui_move_value: SuiMoveValue = MoveValue::Struct(move_struct.clone()).into();
    if let SuiMoveValue::Struct(move_struct) = sui_move_value {
      let fields = fields.contents.iter().map(|entry| {
        match parse_template(&entry.value, &move_struct) {
          Ok(value) => Ok((entry.key.clone(), value)),
          Err(e) => Err(e),
        }
      });
      let (oks, errs): (Vec<_>, Vec<_>) = fields.partition(Result::is_ok);
      let success = oks.into_iter().filter_map(Result::ok).collect();
      let errors: Vec<_> = errs.into_iter().filter_map(Result::err).collect();
      let error_string = errors
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<String>>()
        .join("; ");

      let error = if !error_string.is_empty() {
        Some(SuiObjectResponseError::DisplayError {
          error: anyhow!("{error_string}").to_string(),
        })
      } else {
        None
      };

      return Ok(DisplayFieldsResponse {
        data: Some(success),
        error,
      });
    }
    Err(anyhow!("NotMoveStruct"))?
  }

  // pub fn to_display_update_event(&self) -> Result<DisplayVersionUpdatedEvent, bcs::Error> {
  //   bcs::from_bytes(&self.bcs)
  // }
}

fn parse_template(template: &str, move_struct: &SuiMoveStruct) -> Result<String> {
  let mut output = template.to_string();
  let mut var_name = String::new();
  let mut in_braces = false;
  let mut escaped = false;

  for ch in template.chars() {
    match ch {
      '\\' => {
        escaped = true;
        continue;
      }
      '{' if !escaped => {
        in_braces = true;
        var_name.clear();
      }
      '}' if !escaped => {
        in_braces = false;
        let value = get_value_from_move_struct(move_struct, &var_name)?;
        output = output.replace(&format!("{{{}}}", var_name), &value.to_string());
      }
      _ if !escaped => {
        if in_braces {
          var_name.push(ch);
        }
      }
      _ => {}
    }
    escaped = false;
  }

  Ok(output.replace('\\', ""))
}

fn get_value_from_move_struct(move_struct: &SuiMoveStruct, var_name: &str) -> Result<String> {
  let parts: Vec<&str> = var_name.split('.').collect();
  if parts.is_empty() {
    Err(anyhow!("Display template value cannot be empty"))?;
  }
  if parts.len() > MAX_DISPLAY_NESTED_LEVEL {
    Err(anyhow!("Display template value nested depth cannot exist {}", MAX_DISPLAY_NESTED_LEVEL))?;
  }
  let mut current_value = &SuiMoveValue::Struct(move_struct.clone());
  // iterate over the parts and try to access the corresponding field
  for part in parts {
    match current_value {
      SuiMoveValue::Struct(move_struct) => {
        if
          let SuiMoveStruct::WithTypes { type_: _, fields } | SuiMoveStruct::WithFields(fields) =
            move_struct
        {
          if let Some(value) = fields.get(part) {
            current_value = value;
          } else {
            Err(anyhow!("Field value {} cannot be found in struct", var_name))?;
          }
        } else {
          Err(anyhow!("Unexpected move struct type for field {}", var_name))?;
        }
      }
      SuiMoveValue::Variant(SuiMoveVariant { fields, variant, .. }) => {
        if let Some(value) = fields.get(part) {
          current_value = value;
        } else {
          Err(anyhow!("Field value {var_name} cannot be found in variant {variant}"))?;
        }
      }
      _ => {
        return Err(anyhow!("Unexpected move value type for field {}", var_name))?;
      }
    }
  }

  match current_value {
    SuiMoveValue::Option(move_option) =>
      match move_option.as_ref() {
        Some(move_value) => Ok(move_value.to_string()),
        None => Ok("".to_string()),
      }
    SuiMoveValue::Vector(_) =>
      Err(anyhow!("Vector is not supported as a Display value {}", var_name))?,

    _ => Ok(current_value.to_string()),
  }
}
