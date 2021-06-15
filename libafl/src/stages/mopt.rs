use core::marker::PhantomData;

use crate::{
    bolts::rands::Rand,
    corpus::Corpus,
    fuzzer::Evaluator,
    inputs::Input,
    mutators::{MOptMode, Mutator},
    stages::{MutationalStage, Stage},
    state::{HasClientPerfStats, HasCorpus, HasMOpt, HasRand, HasSolutions},
    Error,
};

const limit_time_bound: f64 = 1.1;

#[derive(Clone, Debug)]
pub struct MOptStage<C, E, EM, I, M, R, S, Z>
where
    C: Corpus<I>,
    M: Mutator<I, S>,
    I: Input,
    R: Rand,
    S: HasClientPerfStats + HasCorpus<C, I> + HasSolutions<C, I> + HasRand<R> + HasMOpt<I, R>,
    Z: Evaluator<E, EM, I, S>,
{
    mutator: M,
    #[allow(clippy::type_complexity)]
    phantom: PhantomData<(C, E, EM, I, R, S, Z)>,
}

impl<C, E, EM, I, M, R, S, Z> MutationalStage<C, E, EM, I, M, S, Z>
    for MOptStage<C, E, EM, I, M, R, S, Z>
where
    C: Corpus<I>,
    M: Mutator<I, S>,
    I: Input,
    R: Rand,
    S: HasClientPerfStats + HasCorpus<C, I> + HasSolutions<C, I> + HasRand<R> + HasMOpt<I, R>,
    Z: Evaluator<E, EM, I, S>,
{
    /// The mutator, added to this stage
    #[inline]
    fn mutator(&self) -> &M {
        &self.mutator
    }

    /// The list of mutators, added to this stage (as mutable ref)
    #[inline]
    fn mutator_mut(&mut self) -> &mut M {
        &mut self.mutator
    }

    /// Gets the number of iterations as a random number
    fn iterations(&self, state: &mut S) -> usize {
        // TODO: we want to use calculate_score here

        1 + state.rand_mut().below(128) as usize
    }

    #[allow(clippy::cast_possible_wrap)]
    fn perform_mutational(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        state: &mut S,
        manager: &mut EM,
        corpus_idx: usize,
    ) -> Result<(), Error> {
        match state.mopt().key_module() {
            MOptMode::CORE_FUZZING => {
                if state.mopt().finds_since_switching() == 0 {
                    // Now, we have just switched back from PILOT_FUZZING mode
                    let finds = state.corpus().count() + state.solutions().count();
                    state.mopt_mut().add_finds_since_switching(finds);
                    state.mopt_mut().set_last_limit_time_start();
                }

                let num = self.iterations(state);

                for i in 0..num {
                    let mut input = state
                        .corpus()
                        .get(corpus_idx)?
                        .borrow_mut()
                        .load_input()?
                        .clone();

                    self.mutator_mut().mutate(state, &mut input, i as i32)?;

                    state.mopt_mut().add_core_time(1);

                    let finds_before = state.corpus().count() + state.solutions().count();

                    let (_, corpus_idx) = fuzzer.evaluate_input(state, executor, manager, input)?;

                    let finds = state.corpus().count() + state.solutions().count();
                    if state.corpus().count() + state.solutions().count() > finds_before {
                        let diff = finds - finds_before;
                        state.mopt_mut().add_total_finds(diff);
                        for i in 0..state.mopt().operator_num() {
                            if state.mopt().core_operator_ctr(i)
                                > state.mopt().core_operator_ctr_last(i)
                            {
                                state.mopt_mut().add_core_operator_finds(i, diff);
                            }
                        }
                    }

                    self.mutator_mut().post_exec(state, i as i32, corpus_idx)?;
                }
            }
            MOptMode::PILOT_FUZZING => {}
        }

        Ok(())
    }
}

impl<C, E, EM, I, M, R, S, Z> Stage<E, EM, S, Z> for MOptStage<C, E, EM, I, M, R, S, Z>
where
    C: Corpus<I>,
    M: Mutator<I, S>,
    I: Input,
    R: Rand,
    S: HasClientPerfStats + HasCorpus<C, I> + HasSolutions<C, I> + HasRand<R> + HasMOpt<I, R>,
    Z: Evaluator<E, EM, I, S>,
{
    #[inline]
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        state: &mut S,
        manager: &mut EM,
        corpus_idx: usize,
    ) -> Result<(), Error> {
        self.perform_mutational(fuzzer, executor, state, manager, corpus_idx)
    }
}

impl<C, E, EM, I, M, R, S, Z> MOptStage<C, E, EM, I, M, R, S, Z>
where
    C: Corpus<I>,
    M: Mutator<I, S>,
    I: Input,
    R: Rand,
    S: HasClientPerfStats + HasCorpus<C, I> + HasSolutions<C, I> + HasRand<R> + HasMOpt<I, R>,
    Z: Evaluator<E, EM, I, S>,
{
    /// Creates a new default mutational stage
    pub fn new(mutator: M) -> Self {
        Self {
            mutator,
            phantom: PhantomData,
        }
    }
}
