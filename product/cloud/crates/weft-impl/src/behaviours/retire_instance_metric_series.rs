//! `weft.observability.RetireInstanceMetricSeries` (`ess/cloud/6-platform/observability.yaml`),
//! invoked by the hand-run binding `telemetry-retire-series-on-exit` on
//! `weft.workloads.InstanceExited` (`story:cloudspec-submission-job-exit`: the first exit on the
//! path). Its one outcome, `retired`: each `Active` `weft.observability.MetricSeries` of the
//! instance moves to `Retired` (`retire`); `InstanceMetricSeriesRetired` counts them and
//! `MetricSeriesRetired` names the first in the order the storage port lists them.
//!
//! OPEN (ESS): the outcome carries one `MetricSeriesRetired` (ESS 0.99.2 generates one member for
//! `each: changed`), so `retired` over no active series has no event to carry but a made-up one.
//! The command answers that case as an unmet obligation, and the binding invokes it only for an
//! instance with an active series (`bindings/telemetry_retire_series_on_exit.rs`).

use weft_types::behaviour::MetricSeriesStorage;
use weft_types::obligation::UnmetObligation;
use weft_types::observability::obligations::RetireInstanceMetricSeriesBehavior;
use weft_types::observability::{
    InstanceMetricSeriesRetired, MetricSeriesRetired, MetricSeriesState,
    RetireInstanceMetricSeries, RetireInstanceMetricSeriesOutcome,
};

use crate::Ports;

impl RetireInstanceMetricSeriesBehavior for Ports {
    fn retire_instance_metric_series(
        &mut self,
        input: RetireInstanceMetricSeries,
    ) -> Result<RetireInstanceMetricSeriesOutcome, UnmetObligation> {
        let active: Vec<_> = MetricSeriesStorage::list(self)
            .into_iter()
            .filter(|series| {
                series.data.instance_id.as_ref() == Some(&input.instance_id)
                    && series.state == MetricSeriesState::Active
            })
            .collect();
        let Some(first) = active.first() else {
            return Err(UnmetObligation {
                capability: "each: changed over no row",
                source: "RetireInstanceMetricSeries retired no series: its outcome carries one MetricSeriesRetired",
            });
        };
        let first = MetricSeriesRetired {
            metric_series_id: first.data.metric_series_id.clone(),
        };
        let retired = i64::try_from(active.len()).unwrap_or(i64::MAX);
        for mut series in active {
            series.state = MetricSeriesState::Retired;
            MetricSeriesStorage::put(self, series);
        }
        Ok(RetireInstanceMetricSeriesOutcome::Retired {
            instance_metric_series_retired: InstanceMetricSeriesRetired {
                instance_id: input.instance_id,
                retired,
            },
            metric_series_retired: first,
        })
    }
}
