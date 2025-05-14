use std::borrow::Cow;

use move_binary_format::file_format::{ AbilitySet, DatatypeTyParameter };
use move_core_types::account_address::AccountAddress;
use serde::{ Deserialize, Serialize };
use sui_package_resolver::{ DataDef, DatatypeRef, MoveData, OpenSignatureBody, VariantDef };

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OpenSignatureBodyWapper {
  Address,
  Bool,
  U8,
  U16,
  U32,
  U64,
  U128,
  U256,
  Vector(Box<OpenSignatureBodyWapper>),
  Datatype(DatatypeKeyWrapper, Vec<OpenSignatureBodyWapper>),
  TypeParameter(u16),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatatypeRefWrapper<'m, 'n> {
  pub package: AccountAddress,
  pub module: Cow<'m, str>,
  pub name: Cow<'n, str>,
}

/// A `StructRef` that owns its strings.
pub type DatatypeKeyWrapper = DatatypeRefWrapper<'static, 'static>;

impl<'a, 'b> From<DatatypeRefWrapper<'a, 'b>> for DatatypeRef<'a, 'b> {
  fn from(value: DatatypeRefWrapper<'a, 'b>) -> Self {
    let data: DatatypeRef<'a, 'b> = DatatypeRef {
      package: value.package,
      module: value.module,
      name: value.name,
    };

    data
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MoveDataWrapper {
  /// Serialized representation of fields (names and deserialized signatures). Signatures refer to
  /// packages at their runtime IDs (not their storage ID or defining ID).
  Struct(Vec<(String, OpenSignatureBodyWapper)>),

  /// Serialized representation of variants (names and deserialized signatures).
  Enum(Vec<VariantDefWrapper>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantDefWrapper {
  /// The name of the enum variant
  pub name: String,

  /// The serialized representation of the variant's signature. Signatures refer to packages at
  /// their runtime IDs (not their storage ID or defining ID).
  pub signatures: Vec<(String, OpenSignatureBodyWapper)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatatypeTyParameterWrapper {
  /// The type parameter constraints.
  pub constraints: AbilitySet,
  /// Whether the parameter is declared as phantom.
  pub is_phantom: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataDefWrapper {
  pub defining_id: AccountAddress,
  pub abilities: AbilitySet,
  pub type_params: Vec<DatatypeTyParameterWrapper>,
  pub data: MoveDataWrapper,
}

impl From<VariantDefWrapper> for VariantDef {
  fn from(value: VariantDefWrapper) -> Self {
    VariantDef {
      name: value.name,
      signatures: value.signatures
        .iter()
        .map(|v| (v.0.clone(), v.1.clone().into()))
        .collect(),
    }
  }
}

impl From<OpenSignatureBodyWapper> for OpenSignatureBody {
  fn from(value: OpenSignatureBodyWapper) -> Self {
    match value {
      OpenSignatureBodyWapper::Address => OpenSignatureBody::Address,
      OpenSignatureBodyWapper::Bool => OpenSignatureBody::Bool,
      OpenSignatureBodyWapper::U256 => OpenSignatureBody::U256,
      OpenSignatureBodyWapper::U128 => OpenSignatureBody::U128,
      OpenSignatureBodyWapper::U64 => OpenSignatureBody::U64,
      OpenSignatureBodyWapper::U32 => OpenSignatureBody::U32,
      OpenSignatureBodyWapper::U16 => OpenSignatureBody::U16,
      OpenSignatureBodyWapper::U8 => OpenSignatureBody::U8,
      OpenSignatureBodyWapper::Datatype(v1, v2) =>
        OpenSignatureBody::Datatype(
          v1.into(),
          v2
            .iter()
            .map(|v| v.clone().into())
            .collect()
        ),
      OpenSignatureBodyWapper::TypeParameter(v) => OpenSignatureBody::TypeParameter(v),
      OpenSignatureBodyWapper::Vector(v) => {
        OpenSignatureBody::Vector(Box::new(v.as_ref().clone().into()))
      }
    }
  }
}

impl From<OpenSignatureBody> for OpenSignatureBodyWapper {
  fn from(value: OpenSignatureBody) -> Self {
    match value {
      OpenSignatureBody::Address => OpenSignatureBodyWapper::Address,
      OpenSignatureBody::Bool => OpenSignatureBodyWapper::Bool,
      OpenSignatureBody::U256 => OpenSignatureBodyWapper::U256,
      OpenSignatureBody::U128 => OpenSignatureBodyWapper::U128,
      OpenSignatureBody::U64 => OpenSignatureBodyWapper::U64,
      OpenSignatureBody::U32 => OpenSignatureBodyWapper::U32,
      OpenSignatureBody::U16 => OpenSignatureBodyWapper::U16,
      OpenSignatureBody::U8 => OpenSignatureBodyWapper::U8,
      OpenSignatureBody::Datatype(v1, v2) =>
        OpenSignatureBodyWapper::Datatype(
          DatatypeRefWrapper { package: v1.package, module: v1.module, name: v1.name },
          v2
            .iter()
            .map(|v| v.clone().into())
            .collect()
        ),
      OpenSignatureBody::TypeParameter(v) => OpenSignatureBodyWapper::TypeParameter(v),
      OpenSignatureBody::Vector(v) => {
        OpenSignatureBodyWapper::Vector(Box::new(v.as_ref().clone().into()))
      }
    }
  }
}

impl From<MoveDataWrapper> for MoveData {
  fn from(value: MoveDataWrapper) -> Self {
    match value {
      MoveDataWrapper::Enum(v) =>
        Self::Enum(
          v
            .iter()
            .map(|v| v.clone().into())
            .collect()
        ),
      MoveDataWrapper::Struct(s) =>
        Self::Struct(
          s
            .iter()
            .map(|v| (v.0.clone(), v.1.clone().into()))
            .collect()
        ),
    }
  }
}

impl From<MoveData> for MoveDataWrapper {
  fn from(value: MoveData) -> Self {
    match value {
      MoveData::Enum(v) =>
        Self::Enum(
          v
            .iter()
            .map(|v| VariantDefWrapper {
              name: v.name.clone(),
              signatures: v.signatures
                .iter()
                .map(|v| (v.0.clone(), v.1.clone().into()))
                .collect(),
            })
            .collect()
        ),
      MoveData::Struct(s) =>
        Self::Struct(
          s
            .iter()
            .map(|v| (v.0.clone(), v.1.clone().into()))
            .collect()
        ),
    }
  }
}

impl From<&MoveData> for MoveDataWrapper {
  fn from(value: &MoveData) -> Self {
    match value {
      MoveData::Enum(v) =>
        Self::Enum(
          v
            .iter()
            .map(|v| VariantDefWrapper {
              name: v.name.clone(),
              signatures: v.signatures
                .iter()
                .map(|v| (v.0.clone(), v.1.clone().into()))
                .collect(),
            })
            .collect()
        ),
      MoveData::Struct(s) =>
        Self::Struct(
          s
            .iter()
            .map(|v| (v.0.clone(), v.1.clone().into()))
            .collect()
        ),
    }
  }
}

impl From<DatatypeTyParameterWrapper> for DatatypeTyParameter {
  fn from(value: DatatypeTyParameterWrapper) -> Self {
    Self { constraints: value.constraints, is_phantom: value.is_phantom }
  }
}

impl From<DataDefWrapper> for DataDef {
  fn from(value: DataDefWrapper) -> Self {
    DataDef {
      abilities: value.abilities,
      data: value.data.into(),
      defining_id: value.defining_id,
      type_params: value.type_params
        .iter()
        .map(|v| v.clone().into())
        .collect(),
    }
  }
}

impl From<&DataDefWrapper> for DataDef {
  fn from(value: &DataDefWrapper) -> Self {
    DataDef {
      abilities: value.abilities,
      data: value.data.clone().into(),
      defining_id: value.defining_id,
      type_params: value.type_params
        .iter()
        .map(|v| v.clone().into())
        .collect(),
    }
  }
}

impl From<DataDef> for DataDefWrapper {
  fn from(value: DataDef) -> Self {
    Self {
      defining_id: value.defining_id,
      abilities: value.abilities,
      type_params: value.type_params
        .iter()
        .map(|v| DatatypeTyParameterWrapper {
          constraints: v.constraints,
          is_phantom: v.is_phantom,
        })
        .collect(),
      data: value.data.into(),
    }
  }
}

impl From<&DataDef> for DataDefWrapper {
  fn from(value: &DataDef) -> Self {
    Self {
      defining_id: value.defining_id,
      abilities: value.abilities,
      type_params: value.type_params
        .iter()
        .map(|v| DatatypeTyParameterWrapper {
          constraints: v.constraints,
          is_phantom: v.is_phantom,
        })
        .collect(),
      data: MoveDataWrapper::from(&value.data),
    }
  }
}