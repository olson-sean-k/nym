use crate::actuator::Actuator;
use crate::pattern::{FromPattern, ToPattern};
use crate::transform::Transform;

#[derive(Clone, Copy, Debug)]
pub struct Policy {
    pub parents: bool,
    pub overwrite: bool,
}

#[derive(Clone, Debug)]
pub struct Environment {
    policy: Policy,
}

impl Environment {
    pub fn new(policy: Policy) -> Self {
        Environment { policy }
    }

    pub fn transform<'f, 't>(
        &self,
        from: FromPattern<'f>,
        to: ToPattern<'t>,
    ) -> Transform<'_, 'f, 't> {
        Transform::new(self, from, to)
    }

    pub fn actuator(&self) -> Actuator<'_> {
        Actuator::new(self)
    }

    pub fn policy(&self) -> &Policy {
        &self.policy
    }
}
