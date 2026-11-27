//! `weft.infrastructure.DefinePriorityClass` (`PLAN.md` § Obligations,
//! `ess/1-infrastructure/infrastructure.yaml`): `negative-grace`, `exists` (a class of that
//! name), `defined` otherwise.

use weft_types::behaviour::PriorityClassStorage;
use weft_types::infrastructure::obligations::DefinePriorityClassBehavior;
use weft_types::infrastructure::{
    DefinePriorityClass, DefinePriorityClassOutcome, InvalidGracePeriod, PriorityClassData, PriorityClassDefined,
    PriorityClassExists, PriorityClassSnapshot, PriorityClassState,
};
use weft_types::obligation::UnmetObligation;

use crate::Ports;

impl DefinePriorityClassBehavior for Ports {
    fn define_priority_class(
        &mut self,
        input: DefinePriorityClass,
    ) -> Result<DefinePriorityClassOutcome, UnmetObligation> {
        if input.grace_seconds < 0 {
            return Ok(DefinePriorityClassOutcome::NegativeGrace { error: InvalidGracePeriod });
        }
        if PriorityClassStorage::list(self)
            .iter()
            .any(|row| row.data.name == input.name)
        {
            return Ok(DefinePriorityClassOutcome::Exists { error: PriorityClassExists });
        }
        PriorityClassStorage::put(
            self,
            PriorityClassSnapshot {
                state: PriorityClassState::Active,
                data: PriorityClassData {
                    name: input.name.clone(),
                    value: input.value,
                    preempts: input.preempts,
                    preemptible: input.preemptible,
                    grace_seconds: input.grace_seconds,
                },
            },
        );
        Ok(DefinePriorityClassOutcome::Defined {
            priority_class_defined: PriorityClassDefined {
                name: input.name,
                value: input.value,
                preempts: input.preempts,
                preemptible: input.preemptible,
                grace_seconds: input.grace_seconds,
            },
        })
    }
}
