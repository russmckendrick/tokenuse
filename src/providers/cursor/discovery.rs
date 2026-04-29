use color_eyre::Result;

use crate::providers::SessionSource;

use super::config;

pub fn discover() -> Result<Vec<SessionSource>> {
    let Some(db) = config::state_db_path() else {
        return Ok(Vec::new());
    };
    if !db.exists() {
        return Ok(Vec::new());
    }
    Ok(vec![SessionSource {
        path: db,
        project: "cursor-workspace".into(),
        provider: config::PROVIDER_ID,
    }])
}
