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

const LIMIT_TIME_BOUND: f64 = 1.1;
const PERIOD_PILOT_COEF: f64 = 5000.0;

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

    #[allow(clippy::cast_possible_wrap, clippy::cast_precision_loss)]
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
                    state.mopt_mut().set_finds_since_switching(finds);
                    state.mopt_mut().set_last_limit_time_start();
                }

                let num = self.iterations(state);

                for stage_id in 0..num {
                    let mut input = state
                        .corpus()
                        .get(corpus_idx)?
                        .borrow_mut()
                        .load_input()?
                        .clone();

                    self.mutator_mut()
                        .mutate(state, &mut input, stage_id as i32)?;

                    let core_time = state.mopt().core_time();
                    state.mopt_mut().set_core_time(core_time + 1);

                    let finds_before = state.corpus().count() + state.solutions().count();

                    let (_, corpus_idx) = fuzzer.evaluate_input(state, executor, manager, input)?;

                    self.mutator_mut()
                        .post_exec(state, stage_id as i32, corpus_idx)?;

                    let finds = state.corpus().count() + state.solutions().count();
                    if finds > finds_before {
                        let diff = finds - finds_before;
                        let cur_total_finds = state.mopt().total_finds();
                        state.mopt_mut().set_total_finds(diff + cur_total_finds);
                        for i in 0..state.mopt().operator_num() {
                            if state.mopt().core_operator_ctr_per_stage(i)
                                > state.mopt().core_operator_ctr_last(i)
                            {
                                let prev = state.mopt().core_operator_finds_per_stage(i);
                                state
                                    .mopt_mut()
                                    .set_core_operator_finds_per_stage(i, diff + prev);
                            }
                        }
                    }

                    if (finds as f64)
                        > (finds as f64) * LIMIT_TIME_BOUND
                            + (state.mopt().finds_since_switching() as f64)
                    {
                        // Move to the Pilot fuzzing mode
                        state.mopt_mut().set_key_module(MOptMode::PILOT_FUZZING);
                        state.mopt_mut().set_finds_since_switching(0);
                    }

                    if state.mopt().core_time() > state.mopt().period_core() {
                        // Make a call to pso_update()
                        state.mopt_mut().set_core_time(0);
                        let total_finds = state.mopt().total_finds();
                        state.mopt_mut().set_finds_until_last_switching(total_finds);
                        state.mopt_mut().update_core_operator_ctr_pso();
                        state.mopt_mut().pso_update()?;
                    }
                }
            }
            MOptMode::PILOT_FUZZING => {
                let num = self.iterations(state);
                for stage_id in 0..num {
                    let mut input = state
                        .corpus()
                        .get(corpus_idx)?
                        .borrow_mut()
                        .load_input()?
                        .clone();

                    self.mutator_mut()
                        .mutate(state, &mut input, stage_id as i32)?;

                    let pilot_time = state.mopt().pilot_time();
                    state.mopt_mut().set_pilot_time(pilot_time + 1);

                    let finds_before = state.corpus().count() + state.solutions().count();

                    let (_, corpus_idx) = fuzzer.evaluate_input(state, executor, manager, input)?;

                    self.mutator_mut()
                        .post_exec(state, stage_id as i32, corpus_idx)?;

                    let swarm_now = state.mopt().swarm_now();
                    let finds = state.corpus().count() + state.solutions().count();
                    let diff = finds - finds_before;

                    if finds > finds_before {
                        let cur_total_finds = state.mopt().total_finds();
                        state.mopt_mut().set_total_finds(diff + cur_total_finds);
                        for i in 0..state.mopt().operator_num() {
                            if state.mopt().pilot_operator_ctr_per_stage(swarm_now, i)
                                > state.mopt().pilot_operator_ctr_last(swarm_now, i)
                            {
                                let prev =
                                    state.mopt().pilot_operator_finds_per_stage(swarm_now, i);
                                state.mopt_mut().set_pilot_operator_finds_per_stage(
                                    swarm_now,
                                    i,
                                    diff + prev,
                                );
                            }
                        }
                    }

                    if state.mopt().pilot_time() > state.mopt().period_pilot() {
                        let new_finds =
                            state.mopt().total_finds() - state.mopt().finds_until_last_switching();
                        let f = (new_finds as f64)
                            / ((state.mopt().pilot_time() as f64) / (PERIOD_PILOT_COEF));
                        state.mopt_mut().set_swarm_fitness(swarm_now, f);
                        state.mopt_mut().set_pilot_time(0);
                        let total_finds = state.mopt().total_finds();
                        state.mopt_mut().set_finds_until_last_switching(total_finds);
                        state.mopt_mut().update_pilot_operator_ctr_pso(swarm_now);

                        state.mopt_mut().set_swarm_now(swarm_now + 1);

                        if state.mopt().swarm_now() == state.mopt().swarm_num() {
                            // Move to CORE_FUZING mode
                            state.mopt_mut().set_key_module(MOptMode::CORE_FUZZING);

                            state.mopt_mut().init_core_module()?;
                        }
                    }
                }
            }
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
