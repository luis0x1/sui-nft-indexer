use anyhow::Result;

use crate::scan_worker::AppProvider;

pub struct BlacklistResolver;

impl BlacklistResolver {
  pub async fn is_blacklist(provider: &AppProvider, object_type: &str) -> bool {
    if let Some(is_blacklist) = provider.is_blacklist(object_type).await {
      return is_blacklist;
    }
    let pg_client = provider.pg_client();
    let exists_res = pg_client.query(
      "SELECT EXISTS(SELECT id FROM blacklist_struct WHERE object_type = $1)",
      &[&object_type]
    ).await;

    if let Ok(exists) = exists_res {
      if exists.len() > 0 {
        let struct_exists: bool = exists[0].get("exists");
        provider.set_blacklist(object_type, struct_exists).await;

        return struct_exists;
      }
    }

    false
  }

  pub async fn set_blacklist(
    provider: &AppProvider,
    object_type: &str,
    is_blacklist: bool
  ) -> Result<()> {
    let res = provider.set_blacklist(object_type, is_blacklist).await;

    if res.is_some() {
      return Ok(());
    }

    if is_blacklist {
      let pg_client = provider.pg_client();
      let _ = pg_client.query(
        "INSERT INTO blacklist_struct(object_type)
          VALUES($1) ON CONFLICT(object_type)
          DO NOTHING",
        &[&object_type]
      ).await;
    }

    Ok(())
  }
}
