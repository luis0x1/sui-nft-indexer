use serde::{ Deserialize, Serialize };
use sui_types::display::DisplayVersionUpdatedEvent;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ObjectDisplayField {
  key: String,
  value: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StoredDisplay {
  pub object_type: String,
  pub id: Vec<u8>,
  pub version: i16,
  pub bcs: Vec<u8>,
  pub fields: Vec<ObjectDisplayField>,
}

impl StoredDisplay {
  pub fn try_from_event(event: &sui_types::event::Event) -> Option<Self> {
    let (ty, display_event) = DisplayVersionUpdatedEvent::try_from_event(event)?;
    let fields: Vec<ObjectDisplayField> = display_event.fields.contents
      .iter()
      .map(|content| ObjectDisplayField {
        key: content.key.clone(),
        value: content.value.clone(),
      })
      .collect();

    Some(Self {
      object_type: ty.to_canonical_string(/* with_prefix */ true),
      id: display_event.id.bytes.to_vec(),
      version: display_event.version as i16,
      bcs: event.contents.clone(),
      fields,
    })
  }

  // pub fn to_display_update_event(&self) -> Result<DisplayVersionUpdatedEvent, bcs::Error> {
  //   bcs::from_bytes(&self.bcs)
  // }
}
