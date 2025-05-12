use std::{ collections::BTreeMap, str::FromStr, sync::Arc };
use redis::AsyncCommands;
use anyhow::{ Error, Result };
use move_core_types::{
  account_address::AccountAddress,
  annotated_value::{ MoveEnumLayout, MoveFieldLayout, MoveStructLayout, MoveTypeLayout },
  language_storage::StructTag,
};
use sui_package_resolver::{ DataDef, MoveData, OpenSignatureBody };
use sui_types::{ Identifier, TypeTag };

use crate::{ utils::error::AppError, scan_worker::AppProvider };

use super::{ object_wrapper::DataDefWrapper, package_resolve::PackageResolver };

pub struct StructResolver {}

impl StructResolver {
  async fn get_datatype(provider: &AppProvider, key: &StructTag) -> Result<Arc<DataDef>> {
    let key_str = format!(
      "{}::{}::{}",
      key.address.to_canonical_string(true),
      key.module,
      key.name
    );
    let data_def_opt = provider.get_dataref(&key_str).await;

    if let Some(data_def) = data_def_opt {
      return Ok(Arc::new(data_def.into()));
    }

    let (package, _) = match
      PackageResolver::get_package(provider, &key.address.to_canonical_string(true)).await
    {
      Ok(value) => value,
      Err(e) => {
        return Err(
          Error::from_boxed(Box::new(AppError::new(&format!("Cannot find package: {:?}", e))))
        );
      }
    };

    if
      key.address.to_canonical_string(true) ==
      "0x4979543452f609ff272d504755966ff5e70122256101c17adbe8bd27d35116c3"
    {
      let modules = package
        .modules()
        .iter()
        .map(|m| m.0.clone())
        .collect::<Vec<String>>();

      println!(
        "package: {:?} + {:?} -> {:?}",
        modules,
        key.module.to_string(),
        package.module(&key.module.to_string()).is_ok()
      );
    }

    let Ok(module) = package.module(&key.module.to_string()) else {
      return Err(
        Error::from_boxed(
          Box::new(
            AppError::new(
              &format!(
                "cannot find module: {:?}::{:?}::{:?}",
                &key.address.to_canonical_string(true),
                &key.module.to_string(),
                &key.name.to_string()
              )
            )
          )
        )
      );
    };

    let Ok(data_opt) = module.data_def(key.name.as_str()) else {
      return Err(Error::msg("Cannot find struct(85)"));
    };

    let Some(data) = data_opt else {
      return Err(Error::msg("Cannot find struct(89)"));
    };

    let value = DataDefWrapper::from(&data);

    match provider.set_dataref(&key_str, value).await {
      Err(e) => {
        eprintln!("Failed to save struct to db: {:?}", e);
      }
      Ok(_) => {}
    }

    Ok(Arc::new(data))
  }

  #[allow(unused)]
  async fn save_datatype(
    provider: &AppProvider,
    key: &str,
    data_type: &DataDefWrapper,
    version: u64
  ) -> Result<()> {
    let pg_client = provider.pg_client();
    let redis_client = provider.redis_client();
    let mut connection = redis_client.get_multiplexed_tokio_connection().await?;

    let seried_value = serde_json::to_string(&data_type)?;

    let inserted_res = pg_client.query_one(
      r#"INSERT INTO object_types (id, fields, version, updated_at, created_at)
                VALUES ($1, $2, $3, NOW(), NOW())
                ON CONFLICT(id)
                DO UPDATE SET fields = $2,
                              version = $3,
                              updated_at = NOW()
                  WHERE version < $3 RETURNING id"#,
      &[&key, &seried_value, &(version as i64)]
    ).await;

    let inserted = match inserted_res {
      Ok(value) => value.len() > 0,
      Err(_) => true,
    };

    if inserted {
      let _: () = connection.set(key, seried_value).await?;
    }

    Ok(())
  }

  pub async fn resolve_type_layout(
    provider: &AppProvider,
    tag: &TypeTag,
    max_depth: usize
  ) -> Result<(MoveTypeLayout, usize)> {
    use MoveTypeLayout as L;
    use TypeTag as T;

    if max_depth == 0 {
      return Err(Error::msg("MAX_DEPTH"));
    }

    Ok(match tag {
      T::Signer => {
        return Err(Error::msg("CANNOT_DECODE_SIGNER_TYPE"));
      }

      T::Address => (L::Address, 1),
      T::Bool => (L::Bool, 1),
      T::U8 => (L::U8, 1),
      T::U16 => (L::U16, 1),
      T::U32 => (L::U32, 1),
      T::U64 => (L::U64, 1),
      T::U128 => (L::U128, 1),
      T::U256 => (L::U256, 1),

      T::Vector(tag) => {
        let (layout, depth) = Box::pin(
          StructResolver::resolve_type_layout(provider, tag, max_depth - 1)
        ).await?;
        (L::Vector(Box::new(layout)), depth + 1)
      }

      T::Struct(s) => {
        let mut param_layouts: Vec<(MoveTypeLayout, usize)> = Vec::new();

        for tag in s.type_params.clone() {
          let tag_layout = Box::pin(
            StructResolver::resolve_type_layout(provider, &tag, max_depth - 1)
          ).await?;
          param_layouts.push(tag_layout);
        }

        let mut type_params: Vec<TypeTag> = Vec::new();
        for l in param_layouts.clone() {
          let move_type = &l.0;
          type_params.push(TypeTag::from(move_type));
        }

        // SAFETY: `add_type_tag` ensures `datatyps` has an element with this key.
        let key = s.as_ref();

        let def = match StructResolver::get_datatype(provider, key).await {
          Err(e) => {
            let message = &format!("CANNOT FIND DATADEF: {:?}", e);
            return Err(Error::from_boxed(Box::new(AppError::new(&message))));
          }
          Ok(v) => v,
        };

        let type_ = StructTag {
          address: def.defining_id,
          module: s.module.clone(),
          name: s.name.clone(),
          type_params,
        };

        Box::pin(
          StructResolver::resolve_datatype_signature(
            provider,
            &def,
            type_,
            param_layouts,
            max_depth
          )
        ).await?
      }
    })
  }

  async fn resolve_datatype_signature(
    provider: &AppProvider,
    data_def: &Arc<DataDef>,
    type_: StructTag,
    param_layouts: Vec<(MoveTypeLayout, usize)>,
    max_depth: usize
  ) -> Result<(MoveTypeLayout, usize)> {
    Ok(match &data_def.data {
      MoveData::Struct(fields) => {
        let mut resolved_fields = Vec::with_capacity(fields.len());
        let mut field_depth = 0;

        for (name, sig) in fields {
          let (layout, depth) = StructResolver::resolve_signature_layout(
            provider,
            sig,
            &param_layouts,
            max_depth - 1
          ).await?;

          field_depth = field_depth.max(depth);
          resolved_fields.push(MoveFieldLayout {
            name: Identifier::from_str(name.as_str())?,
            layout,
          });
        }

        (
          MoveTypeLayout::Struct(
            Box::new(MoveStructLayout {
              type_,
              fields: resolved_fields,
            })
          ),
          field_depth + 1,
        )
      }
      MoveData::Enum(variants) => {
        let mut field_depth = 0;
        let mut resolved_variants = BTreeMap::new();

        for (tag, variant) in variants.iter().enumerate() {
          let mut fields = Vec::with_capacity(variant.signatures.len());
          for (name, sig) in &variant.signatures {
            // Note: We decrement the depth here because we're already under the variant
            let (layout, depth) = StructResolver::resolve_signature_layout(
              provider,
              sig,
              &param_layouts,
              max_depth - 1
            ).await?;

            field_depth = field_depth.max(depth);
            fields.push(MoveFieldLayout {
              name: Identifier::from_str(name.as_str())?,
              layout,
            });
          }

          resolved_variants.insert(
            (Identifier::from_str(variant.name.as_str())?, tag as u16),
            fields
          );
        }

        (
          MoveTypeLayout::Enum(
            Box::new(MoveEnumLayout {
              type_,
              variants: resolved_variants,
            })
          ),
          field_depth + 1,
        )
      }
    })
  }

  async fn resolve_signature_layout(
    provider: &AppProvider,
    sig: &OpenSignatureBody,
    param_layouts: &[(MoveTypeLayout, usize)],
    max_depth: usize
  ) -> Result<(MoveTypeLayout, usize)> {
    use MoveTypeLayout as L;
    use OpenSignatureBody as O;

    if max_depth == 0 {
      return Err(Error::msg("LAYOUT_TOO_DEPTH"));
    }

    Ok(match sig {
      O::Address => (L::Address, 1),
      O::Bool => (L::Bool, 1),
      O::U8 => (L::U8, 1),
      O::U16 => (L::U16, 1),
      O::U32 => (L::U32, 1),
      O::U64 => (L::U64, 1),
      O::U128 => (L::U128, 1),
      O::U256 => (L::U256, 1),

      O::TypeParameter(ix) => {
        let (layout, depth) = param_layouts
          .get(*ix as usize)
          .ok_or_else(|| Error::msg("TypeParamOOB"))
          .cloned()?;

        // We need to re-check the type parameter before we use it because it might have
        // been fine when it was created, but result in too deep a layout when we use it at
        // this position.
        if depth > max_depth {
          return Err(Error::msg("LAYOUT_TOO_DEPTH"));
        }

        (layout, depth)
      }

      O::Vector(sig) => {
        let (layout, depth) = Box::pin(
          StructResolver::resolve_signature_layout(
            provider,
            sig.as_ref(),
            param_layouts,
            max_depth - 1
          )
        ).await?;

        (L::Vector(Box::new(layout)), depth + 1)
      }

      O::Datatype(key, params) => {
        // SAFETY: `add_signature` ensures `datatypes` has an element with this key.
        let def = match
          StructResolver::get_datatype(
            provider,
            &(StructTag {
              address: AccountAddress::from_str(&key.package.to_canonical_string(true)).unwrap(),
              module: Identifier::from_str(&key.module.to_string()).unwrap(),
              name: Identifier::from_str(&key.name.to_string()).unwrap(),
              type_params: vec![],
            })
          ).await
        {
          Err(e) => {
            let message = &format!("CANNOT FIND DATADEF: {:?}", e);
            return Err(Error::from_boxed(Box::new(AppError::new(&message))));
          }
          Ok(v) => v,
        };

        let mut p_layouts: Vec<(MoveTypeLayout, usize)> = Vec::new();

        for sig in params {
          p_layouts.push(
            Box::pin(
              StructResolver::resolve_signature_layout(provider, sig, param_layouts, max_depth - 1)
            ).await?
          );
        }

        // SAFETY: `param_layouts` contains `MoveTypeLayout`-s that are generated by this
        // `ResolutionContext`, which guarantees that struct layouts come with types, which
        // is necessary to avoid errors when converting layouts into type tags.
        let type_params: Vec<TypeTag> = p_layouts
          .iter()
          .map(|l| TypeTag::from(&l.0))
          .collect();

        let type_ = StructTag {
          address: def.defining_id,
          module: Identifier::from_str(&key.module)?,
          name: Identifier::from_str(&key.name)?,
          type_params,
        };

        Box::pin(
          StructResolver::resolve_datatype_signature(provider, &def, type_, p_layouts, max_depth)
        ).await?
      }
    })
  }
}
