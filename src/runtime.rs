use std::time::Duration;

use color_eyre::Result;

use crate::{
    app::{DataSource, RefreshSource},
    archive,
    config::{ConfigPaths, UserConfig},
    currency::CurrencyTable,
    ingest,
};

pub struct RuntimeState {
    pub source: DataSource,
    pub status: Option<String>,
    pub settings: UserConfig,
    pub paths: ConfigPaths,
    pub currency_table: CurrencyTable,
    pub initial_refresh_delay: Duration,
    pub refresh_source: RefreshSource,
}

pub fn load_startup() -> Result<RuntimeState> {
    let paths = ConfigPaths::default();
    let mut status_messages = Vec::new();
    let settings = match UserConfig::load_or_create(&paths) {
        Ok(settings) => settings,
        Err(e) => {
            status_messages.push(format!("config failed · defaults ({e})"));
            UserConfig::default()
        }
    };
    let currency_table = match CurrencyTable::load(&paths) {
        Ok(table) => table,
        Err(e) => {
            status_messages.push(format!("currency rates failed · embedded ({e})"));
            CurrencyTable::embedded().expect("embedded currency rates must be valid JSON")
        }
    };

    let (source, ingest_status, initial_refresh_delay, refresh_source) =
        match archive::load_startup(&paths) {
            Ok(startup) => {
                let mut parts = Vec::new();
                if startup.legacy_records_imported > 0 {
                    parts.push(format!(
                        "legacy cache imported · {} records",
                        startup.legacy_records_imported
                    ));
                }
                if let Some(stats) = startup.sync_stats {
                    if stats.calls_inserted > 0 || stats.limits_inserted > 0 {
                        parts.push(format!(
                            "archive synced · {} calls · {} limits",
                            stats.calls_inserted, stats.limits_inserted
                        ));
                    }
                }

                let source = if startup.ingested.is_empty() {
                    if parts.is_empty() {
                        parts.push("no local sessions found · sample data".into());
                    }
                    DataSource::Sample
                } else {
                    DataSource::Live(startup.ingested)
                };
                let delay = if startup.loaded_existing_archive {
                    Duration::from_secs(0)
                } else {
                    archive::SYNC_INTERVAL
                };
                (
                    source,
                    if parts.is_empty() {
                        None
                    } else {
                        Some(parts.join(" · "))
                    },
                    delay,
                    RefreshSource::Archive(paths.clone()),
                )
            }
            Err(archive_err) => match ingest::load() {
                Ok(ingested) if !ingested.is_empty() => (
                    DataSource::Live(ingested),
                    Some(format!("archive failed · raw ingest ({archive_err})")),
                    archive::SYNC_INTERVAL,
                    RefreshSource::RawIngest,
                ),
                Ok(_) => (
                    DataSource::Sample,
                    Some(format!(
                        "archive failed · raw ingest ({archive_err}) · no local sessions found · sample data"
                    )),
                    archive::SYNC_INTERVAL,
                    RefreshSource::RawIngest,
                ),
                Err(e) => (
                    DataSource::Sample,
                    Some(format!(
                        "archive failed · raw ingest ({archive_err}) · ingest failed · sample data ({e})"
                    )),
                    archive::SYNC_INTERVAL,
                    RefreshSource::RawIngest,
                ),
            },
        };
    if let Some(status) = ingest_status {
        status_messages.push(status);
    }
    let status = if status_messages.is_empty() {
        None
    } else {
        Some(status_messages.join(" · "))
    };

    Ok(RuntimeState {
        source,
        status,
        settings,
        paths,
        currency_table,
        initial_refresh_delay,
        refresh_source,
    })
}
