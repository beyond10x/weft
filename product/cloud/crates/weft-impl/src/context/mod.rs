//! The context port: what the specification leaves to the implementation.
//!
//! - The clock, a parameter: [`Clock::system`] in the binary; a test reads the wall clock around
//!   a call.
//! - Identities and assigned values: UUIDv4 from [`mint`].
//! - External decisions: [`decider`], a table in code.
//! - The caller's attributes: set per request by the control host ([`crate::Ports::set_caller`]).

pub mod decider;

use std::sync::Arc;

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use weft_types::primitives::{Timestamp, Uuid};
use weft_types::shared;

/// A clock: the instant of each command decision.
#[derive(Clone)]
pub struct Clock(Arc<dyn Fn() -> OffsetDateTime + Send + Sync>);

impl Clock {
    /// The host's UTC clock.
    pub fn system() -> Self {
        Self(Arc::new(OffsetDateTime::now_utc))
    }

    /// A clock that answers `now`.
    pub fn from_fn(now: impl Fn() -> OffsetDateTime + Send + Sync + 'static) -> Self {
        Self(Arc::new(now))
    }

    /// The instant now.
    pub fn now(&self) -> OffsetDateTime {
        (self.0)()
    }
}

impl std::fmt::Debug for Clock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Clock")
    }
}

/// The attributes of the caller of a request, as its authentication found them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallerAttributes {
    /// `tenant_id`.
    pub tenant_id: Option<String>,
    /// `account_id`.
    pub account_id: Option<String>,
    /// `principal_id`.
    pub principal_id: Option<String>,
    /// `scope_id`: the scope the request acts in.
    pub scope_id: Option<String>,
}

/// The context of the command running now.
#[derive(Debug)]
pub struct State {
    /// The caller's attributes.
    pub caller: CallerAttributes,
    /// The clock.
    pub clock: Clock,
}

impl State {
    /// A context with no caller.
    pub fn new(clock: Clock) -> Self {
        Self {
            caller: CallerAttributes::default(),
            clock,
        }
    }
}

/// A new identity: a UUIDv4 in its canonical text.
pub fn mint() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn uuid() -> Uuid {
    Uuid(mint())
}

/// `instant` in RFC 3339, as a `Timestamp` carries it.
pub fn timestamp(instant: OffsetDateTime) -> Timestamp {
    Timestamp(
        instant
            .format(&Rfc3339)
            .expect("an instant formats in RFC 3339"),
    )
}

/// The instant a `Timestamp` names, where it is RFC 3339.
pub fn instant(timestamp: &Timestamp) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(&timestamp.0, &Rfc3339).ok()
}

impl crate::Ports {
    /// The instant of the command decision being made now.
    pub fn now(&self) -> OffsetDateTime {
        self.context().clock.now()
    }
}

impl weft_types::behaviour::Context for crate::Ports {
    fn caller_account_id(&self) -> Option<shared::AccountId> {
        self.context()
            .caller
            .account_id
            .clone()
            .map(|id| shared::AccountId(Uuid(id)))
    }

    fn caller_principal_id(&self) -> Option<shared::PrincipalId> {
        self.context()
            .caller
            .principal_id
            .clone()
            .map(|id| shared::PrincipalId(Uuid(id)))
    }

    fn caller_scope_id(&self) -> Option<shared::ScopeId> {
        self.context()
            .caller
            .scope_id
            .clone()
            .map(|id| shared::ScopeId(Uuid(id)))
    }

    fn caller_tenant_id(&self) -> Option<shared::TenantId> {
        self.context()
            .caller
            .tenant_id
            .clone()
            .map(|id| shared::TenantId(Uuid(id)))
    }

    fn generate_optional_timestamp(&mut self) -> Option<Timestamp> {
        None
    }

    fn generate_string(&mut self) -> String {
        mint()
    }

    fn generate_timestamp(&mut self) -> Timestamp {
        timestamp(self.now())
    }

    fn generate_weft_identity_signing_key_ref(&mut self) -> weft_types::identity::SigningKeyRef {
        weft_types::identity::SigningKeyRef(mint())
    }

    fn generate_weft_shared_cloud_id(&mut self) -> shared::CloudId {
        shared::CloudId(uuid())
    }

    fn generate_weft_shared_concurrency_id(&mut self) -> shared::ConcurrencyId {
        shared::ConcurrencyId(uuid())
    }

    fn generate_weft_shared_coverage_entry_id(&mut self) -> shared::CoverageEntryId {
        shared::CoverageEntryId(uuid())
    }

    fn generate_weft_shared_deadline_id(&mut self) -> shared::DeadlineId {
        shared::DeadlineId(uuid())
    }

    fn generate_weft_shared_engine_id(&mut self) -> shared::EngineId {
        shared::EngineId(uuid())
    }

    fn generate_weft_shared_filesystem_id(&mut self) -> shared::FilesystemId {
        shared::FilesystemId(uuid())
    }

    fn generate_weft_shared_metric_series_id(&mut self) -> shared::MetricSeriesId {
        shared::MetricSeriesId(uuid())
    }

    fn generate_weft_shared_node_capacity_id(&mut self) -> shared::NodeCapacityId {
        shared::NodeCapacityId(uuid())
    }

    fn generate_weft_shared_node_observation_id(&mut self) -> shared::NodeObservationId {
        shared::NodeObservationId(uuid())
    }

    fn generate_weft_shared_notification_target_id(&mut self) -> shared::NotificationTargetId {
        shared::NotificationTargetId(uuid())
    }

    fn generate_weft_shared_offering_plan_id(&mut self) -> shared::OfferingPlanId {
        shared::OfferingPlanId(uuid())
    }

    fn generate_weft_shared_principal_id(&mut self) -> shared::PrincipalId {
        shared::PrincipalId(uuid())
    }

    fn generate_weft_shared_publisher_id(&mut self) -> shared::PublisherId {
        shared::PublisherId(uuid())
    }

    fn generate_weft_shared_rate_id(&mut self) -> shared::RateId {
        shared::RateId(uuid())
    }

    fn generate_weft_shared_runtime_id(&mut self) -> shared::RuntimeId {
        shared::RuntimeId(uuid())
    }

    fn generate_weft_shared_telemetry_sink_id(&mut self) -> shared::TelemetrySinkId {
        shared::TelemetrySinkId(uuid())
    }

    fn generate_weft_shared_volume_id(&mut self) -> shared::VolumeId {
        shared::VolumeId(uuid())
    }

    fn generate_weft_shared_wake_id(&mut self) -> shared::WakeId {
        shared::WakeId(uuid())
    }

    fn external(
        &mut self,
        command: weft_types::behaviour::ExternalCommand<'_>,
        outcome: &'static str,
    ) -> bool {
        decider::decide(self.store(), command, outcome)
    }

    fn command_clock(&mut self) -> Option<Timestamp> {
        Some(timestamp(self.now()))
    }
}
