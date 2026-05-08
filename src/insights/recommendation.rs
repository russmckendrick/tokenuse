use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warn,
    Risk,
}

impl Severity {
    pub fn rank(self) -> u8 {
        match self {
            Self::Risk => 2,
            Self::Warn => 1,
            Self::Info => 0,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Risk => "risk",
            Self::Warn => "warn",
            Self::Info => "info",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    ModelRightsizing,
    Cache,
    Anomalies,
    Quota,
}

impl Category {
    pub fn id(self) -> &'static str {
        match self {
            Self::ModelRightsizing => "model_rightsizing",
            Self::Cache => "cache",
            Self::Anomalies => "anomalies",
            Self::Quota => "quota",
        }
    }

    pub const ALL: [Category; 4] = [
        Category::ModelRightsizing,
        Category::Cache,
        Category::Anomalies,
        Category::Quota,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleId {
    ShortOutputSonnet,
    FastModeOpusExcess,
    ReasoningHeavyOSeries,
    CacheHitTrendDrop,
    CacheWriteOverhead,
    LowHitProjectOutlier,
    CacheToolSilenced,
    OutlierSessionCost,
    DayOverDaySpendZscore,
    ProjectMomGrowth,
    ClaudeWeeklyForecast,
    CopilotPremiumPacing,
    LimitRecentlyHit,
}

impl RuleId {
    pub fn key(self) -> &'static str {
        match self {
            Self::ShortOutputSonnet => "short_output_sonnet",
            Self::FastModeOpusExcess => "fast_mode_opus_excess",
            Self::ReasoningHeavyOSeries => "reasoning_heavy_o_series",
            Self::CacheHitTrendDrop => "cache_hit_trend_drop",
            Self::CacheWriteOverhead => "cache_write_overhead",
            Self::LowHitProjectOutlier => "low_hit_project_outlier",
            Self::CacheToolSilenced => "cache_tool_silenced",
            Self::OutlierSessionCost => "outlier_session_cost",
            Self::DayOverDaySpendZscore => "day_over_day_spend_zscore",
            Self::ProjectMomGrowth => "project_mom_growth",
            Self::ClaudeWeeklyForecast => "claude_weekly_forecast",
            Self::CopilotPremiumPacing => "copilot_premium_pacing",
            Self::LimitRecentlyHit => "limit_recently_hit",
        }
    }

    pub fn category(self) -> Category {
        match self {
            Self::ShortOutputSonnet | Self::FastModeOpusExcess | Self::ReasoningHeavyOSeries => {
                Category::ModelRightsizing
            }
            Self::CacheHitTrendDrop
            | Self::CacheWriteOverhead
            | Self::LowHitProjectOutlier
            | Self::CacheToolSilenced => Category::Cache,
            Self::OutlierSessionCost | Self::DayOverDaySpendZscore | Self::ProjectMomGrowth => {
                Category::Anomalies
            }
            Self::ClaudeWeeklyForecast | Self::CopilotPremiumPacing | Self::LimitRecentlyHit => {
                Category::Quota
            }
        }
    }

    pub const ALL: [RuleId; 13] = [
        RuleId::ShortOutputSonnet,
        RuleId::FastModeOpusExcess,
        RuleId::ReasoningHeavyOSeries,
        RuleId::CacheHitTrendDrop,
        RuleId::CacheWriteOverhead,
        RuleId::LowHitProjectOutlier,
        RuleId::CacheToolSilenced,
        RuleId::OutlierSessionCost,
        RuleId::DayOverDaySpendZscore,
        RuleId::ProjectMomGrowth,
        RuleId::ClaudeWeeklyForecast,
        RuleId::CopilotPremiumPacing,
        RuleId::LimitRecentlyHit,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SavingsBasis {
    PerWeek,
    PerMonth,
    OneOff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Scope {
    All,
    Project { name: String },
    ProjectModel { project: String, model: String },
    Tool { tool: String },
    Session { id: String, project: String },
}

impl Scope {
    pub fn slug(&self) -> String {
        match self {
            Scope::All => "all".into(),
            Scope::Project { name } => format!("project={name}"),
            Scope::ProjectModel { project, model } => format!("project={project};model={model}"),
            Scope::Tool { tool } => format!("tool={tool}"),
            Scope::Session { id, .. } => format!("session={id}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Recommendation {
    pub rule_id: RuleId,
    pub severity: Severity,
    pub body_args: Vec<(&'static str, String)>,
    pub est_savings_usd: Option<f64>,
    pub est_savings_basis: Option<SavingsBasis>,
    pub scope: Scope,
    pub silenced_reason_key: Option<&'static str>,
    pub silenced_reason_args: Vec<(&'static str, String)>,
}

impl Recommendation {
    pub fn id(&self) -> String {
        format!("{}:{}", self.rule_id.key(), self.scope.slug())
    }
}

#[derive(Debug, Clone, Default)]
pub struct InsightsBundle {
    pub recommendations: Vec<Recommendation>,
}

impl InsightsBundle {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn push(&mut self, rec: Recommendation) {
        self.recommendations.push(rec);
    }

    pub fn extend(&mut self, others: impl IntoIterator<Item = Recommendation>) {
        self.recommendations.extend(others);
    }

    pub fn finalise(mut self) -> Self {
        self.recommendations.sort_by(|a, b| {
            b.severity
                .rank()
                .cmp(&a.severity.rank())
                .then_with(|| {
                    let a_savings = a.est_savings_usd.unwrap_or(0.0);
                    let b_savings = b.est_savings_usd.unwrap_or(0.0);
                    b_savings
                        .partial_cmp(&a_savings)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.rule_id.key().cmp(b.rule_id.key()))
                .then_with(|| a.scope.slug().cmp(&b.scope.slug()))
        });
        self.recommendations.truncate(25);
        self
    }
}
