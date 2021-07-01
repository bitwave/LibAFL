use libafl::{
    bolts::tuples::Named, corpus::Testcase, events::EventFirer, executors::ExitKind,
    feedbacks::Feedback, inputs::Input, observers::ObserversTuple, state::HasMetadata, Error,
};

use crate::{metadata::ConcolicMetadata, observer::ConcolicObserver};

/// The concolic feedback. It is used to attach concolic tracing metadata to the testcase.
/// This feedback should be used in combination with another feedback as this feedback always considers testcases
/// to be not interesting.
/// Requires a [`ConcolicObserver`] to observe the concolic trace.
pub struct ConcolicFeedback {
    name: String,
    metadata: Option<ConcolicMetadata>,
}

impl ConcolicFeedback {
    pub fn from_observer(observer: &ConcolicObserver) -> Self {
        Self {
            name: observer.name().to_owned(),
            metadata: None,
        }
    }
}

impl Named for ConcolicFeedback {
    fn name(&self) -> &str {
        &self.name
    }
}

impl<I: Input, S> Feedback<I, S> for ConcolicFeedback {
    fn is_interesting<EM, OT>(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        _input: &I,
        observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool, Error>
    where
        EM: EventFirer<I, S>,
        OT: ObserversTuple,
    {
        self.metadata = observers
            .match_name::<ConcolicObserver>(&self.name)
            .map(|o| o.create_metadata_from_current_map());
        Ok(true)
    }

    fn append_metadata(
        &mut self,
        _state: &mut S,
        _testcase: &mut Testcase<I>,
    ) -> Result<(), Error> {
        if let Some(metadata) = self.metadata.take() {
            for (_id, _expression_type) in metadata.iter_messages() {
                println!("{} -> {:?}", _id, _expression_type);
            }
            _testcase.metadata_mut().insert(metadata);
        }
        Ok(())
    }

    fn discard_metadata(&mut self, _state: &mut S, _input: &I) -> Result<(), Error> {
        Ok(())
    }
}