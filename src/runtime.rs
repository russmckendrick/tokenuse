use std::time::Duration;

use color_eyre::Result;

use crate::{
    app::{AppStatus, DataSource, RefreshSource, StatusTone},
    archive,
    config::{ConfigPaths, UserConfig},
    copy::{self, copy},
    currency::CurrencyTable,
    ingest,
};

pub struct RuntimeState {
    pub source: DataSource,
    pub status: Option<AppStatus>,
    pub settings: UserConfig,
    pub paths: ConfigPaths,
    pub currency_table: CurrencyTable,
    pub initial_refresh_delay: Duration,
    pub refresh_source: RefreshSource,
}

pub fn load_startup() -> Result<RuntimeState> {
    let paths = ConfigPaths::default();
    let mut status_messages: Vec<AppStatus> = Vec::new();
    let settings = match UserConfig::load_or_create(&paths) {
        Ok(settings) => settings,
        Err(e) => {
            status_messages.push(AppStatus::new(
                copy::template(
                    &copy().status.config_failed_defaults,
                    &[("error", e.to_string())],
                ),
                StatusTone::Warning,
            ));
            UserConfig::default()
        }
    };
    let currency_table = match CurrencyTable::load(&paths) {
        Ok(table) => table,
        Err(e) => {
            status_messages.push(AppStatus::new(
                copy::template(
                    &copy().status.currency_rates_failed_embedded,
                    &[("error", e.to_string())],
                ),
                StatusTone::Warning,
            ));
            CurrencyTable::embedded().expect("embedded currency rates must be valid JSON")
        }
    };

    let (source, ingest_status, initial_refresh_delay, refresh_source) =
        match archive::load_startup(&paths) {
            Ok(startup) => {
                let mut parts = Vec::new();
                if startup.legacy_records_imported > 0 {
                    parts.push(copy::template(
                        &copy().status.legacy_cache_imported_records,
                        &[("records", startup.legacy_records_imported.to_string())],
                    ));
                }
                if let Some(stats) = startup.sync_stats {
                    if stats.calls_inserted > 0 || stats.limits_inserted > 0 {
                        parts.push(copy::template(
                            &copy().status.archive_synced_counts,
                            &[
                                ("calls", stats.calls_inserted.to_string()),
                                ("limits", stats.limits_inserted.to_string()),
                            ],
                        ));
                    }
                }

                let source = if startup.ingested.is_empty() {
                    if parts.is_empty() {
                        parts.push(copy().status.no_local_sessions_sample_data.clone());
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
                    parts_to_status(parts, StatusTone::Info),
                    delay,
                    RefreshSource::Archive(paths.clone()),
                )
            }
            Err(archive_err) => match ingest::load() {
                Ok(ingested) if !ingested.is_empty() => (
                    DataSource::Live(ingested),
                    Some(AppStatus::new(
                        copy::template(
                            &copy().status.archive_failed_raw_ingest,
                            &[("error", archive_err.to_string())],
                        ),
                        StatusTone::Warning,
                    )),
                    archive::SYNC_INTERVAL,
                    RefreshSource::RawIngest,
                ),
                Ok(_) => (
                    DataSource::Sample,
                    Some(AppStatus::new(
                        copy::template(
                            &copy()
                                .status
                                .archive_failed_raw_ingest_no_sessions_sample_data,
                            &[("archive_error", archive_err.to_string())],
                        ),
                        StatusTone::Warning,
                    )),
                    archive::SYNC_INTERVAL,
                    RefreshSource::RawIngest,
                ),
                Err(e) => (
                    DataSource::Sample,
                    Some(AppStatus::new(
                        copy::template(
                            &copy()
                                .status
                                .archive_failed_raw_ingest_ingest_failed_sample_data,
                            &[
                                ("archive_error", archive_err.to_string()),
                                ("error", e.to_string()),
                            ],
                        ),
                        StatusTone::Error,
                    )),
                    archive::SYNC_INTERVAL,
                    RefreshSource::RawIngest,
                ),
            },
        };
    if let Some(status) = ingest_status {
        status_messages.push(status);
    }
    let status = combine_statuses(status_messages);

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

fn parts_to_status(parts: Vec<String>, tone: StatusTone) -> Option<AppStatus> {
    (!parts.is_empty()).then(|| AppStatus::new(parts.join(" · "), tone))
}

fn combine_statuses(statuses: Vec<AppStatus>) -> Option<AppStatus> {
    if statuses.is_empty() {
        return None;
    }
    let tone = statuses
        .iter()
        .map(|status| status.tone)
        .max_by_key(|tone| match tone {
            StatusTone::Info => 0,
            StatusTone::Success => 1,
            StatusTone::Busy => 2,
            StatusTone::Warning => 3,
            StatusTone::Error => 4,
        })
        .unwrap_or(StatusTone::Info);
    Some(AppStatus::new(
        statuses
            .into_iter()
            .map(|status| status.text)
            .collect::<Vec<_>>()
            .join(" · "),
        tone,
    ))
}
