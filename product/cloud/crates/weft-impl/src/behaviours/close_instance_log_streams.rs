//! `weft.observability.CloseInstanceLogStreams` (`ess/cloud/6-platform/observability.yaml`),
//! invoked by the hand-run binding `telemetry-close-log-streams-on-exit` on
//! `weft.workloads.InstanceExited` (`story:cloudspec-submission-job-exit`: the first exit on the
//! path). Its one outcome, `closed`: each `Open` `weft.observability.LogStream` of the instance
//! moves to `Closed` (`close`) with `closed_at` now and `retained_until` its `retention_days`
//! later; `InstanceLogStreamsClosed` counts them and `LogStreamClosed` names the first in the
//! order the storage port lists them.
//!
//! OPEN (ESS): the outcome carries one `LogStreamClosed` (ESS 0.99.2 generates one member for
//! `each: changed`), so `closed` over no open stream has no event to carry but a made-up one.
//! The command answers that case as an unmet obligation, and the binding invokes it only for an
//! instance with an open stream (`bindings/telemetry_close_log_streams_on_exit.rs`).

use time::Duration;
use weft_types::behaviour::LogStreamStorage;
use weft_types::obligation::UnmetObligation;
use weft_types::observability::obligations::CloseInstanceLogStreamsBehavior;
use weft_types::observability::{
    CloseInstanceLogStreams, CloseInstanceLogStreamsOutcome, InstanceLogStreamsClosed,
    LogStreamClosed, LogStreamState,
};

use crate::context::timestamp;
use crate::Ports;

impl CloseInstanceLogStreamsBehavior for Ports {
    fn close_instance_log_streams(
        &mut self,
        input: CloseInstanceLogStreams,
    ) -> Result<CloseInstanceLogStreamsOutcome, UnmetObligation> {
        let open: Vec<_> = LogStreamStorage::list(self)
            .into_iter()
            .filter(|stream| {
                stream.data.instance_id == input.instance_id && stream.state == LogStreamState::Open
            })
            .collect();
        let Some(first) = open.first() else {
            return Err(UnmetObligation {
                capability: "each: changed over no row",
                source: "CloseInstanceLogStreams closed no stream: its outcome carries one LogStreamClosed",
            });
        };
        let first = LogStreamClosed {
            log_stream_id: first.data.log_stream_id.clone(),
        };
        let now = self.now();
        let closed = i64::try_from(open.len()).unwrap_or(i64::MAX);
        for mut stream in open {
            stream.state = LogStreamState::Closed;
            stream.data.closed_at = Some(timestamp(now));
            stream.data.retained_until =
                Some(timestamp(now + Duration::days(stream.data.retention_days)));
            if let Some(broken) = stream.data.broken_invariant() {
                return Err(UnmetObligation {
                    capability: "entity invariant",
                    source: broken,
                });
            }
            LogStreamStorage::put(self, stream);
        }
        Ok(CloseInstanceLogStreamsOutcome::Closed {
            instance_log_streams_closed: InstanceLogStreamsClosed {
                instance_id: input.instance_id,
                closed,
            },
            log_stream_closed: first,
        })
    }
}
