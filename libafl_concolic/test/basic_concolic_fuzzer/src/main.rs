mod command_executor;
mod feedback;
mod metadata;
mod observer;

use concolic::serialization_format::shared_memory::{DEFAULT_ENV_NAME, DEFAULT_SIZE};
use libafl::feedbacks::EagerOrFeedback;
use libafl::{
    bolts::{
        current_nanos,
        rands::StdRand,
        shmem::{ShMem, ShMemProvider, StdShMemProvider},
        tuples::tuple_list,
    },
    corpus::{Corpus, InMemoryCorpus, OnDiskCorpus, QueueCorpusScheduler, Testcase},
    events::SimpleEventManager,
    feedbacks::{CrashFeedback, TimeFeedback},
    fuzzer::{Fuzzer, StdFuzzer},
    inputs::BytesInput,
    mutators::scheduled::{havoc_mutations, StdScheduledMutator},
    observers::TimeObserver,
    stages::mutational::StdMutationalStage,
    state::{HasCorpus, StdState},
    stats::SimpleStats,
};

use std::path::PathBuf;

use command_executor::CommandExecutor;
use feedback::ConcolicFeedback;
use observer::ConcolicObserver;

#[allow(clippy::similar_names)]
pub fn main() {
    //Coverage map shared between observer and executor
    let mut shmem = StdShMemProvider::new()
        .unwrap()
        .new_map(DEFAULT_SIZE)
        .unwrap();
    //let the forkserver know the shmid
    shmem.write_to_env(DEFAULT_ENV_NAME).unwrap();
    // Create an observation channel using the signals map
    /*let edges_observer = HitcountsMapObserver::new(ConstMapObserver::<_, MAP_SIZE>::new(
        "shared_mem",
        &mut shmem_map,
    )); */

    // Create an observation channel to keep track of the execution time
    let time_observer = TimeObserver::new("time");

    let concolic_observer = ConcolicObserver::new("concolic".to_string(), shmem.map_mut());

    // The state of the edges feedback.
    //let feedback_state = MapFeedbackState::with_observer(&edges_observer);

    // The state of the edges feedback for crashes.
    //let objective_state = MapFeedbackState::new("crash_edges", MAP_SIZE);

    // Feedback to rate the interestingness of an input
    // This one is composed by two Feedbacks in OR
    let feedback = TimeFeedback::new_with_observer(&time_observer);

    let feedback_conc = ConcolicFeedback::from_observer(&concolic_observer);

    let feedback = EagerOrFeedback::new(feedback, feedback_conc);

    // A feedback to choose if an input is a solution or not
    // We want to do the same crash deduplication that AFL does
    let objective = CrashFeedback::new();

    // create a State from scratch
    let mut state = StdState::new(
        // RNG
        StdRand::with_seed(current_nanos()),
        // Corpus that will be evolved, we keep it in memory for performance
        InMemoryCorpus::<BytesInput>::new(),
        // Corpus in which we store solutions (crashes in this example),
        // on disk so the user can get them after stopping the fuzzer
        OnDiskCorpus::new(PathBuf::from("./crashes")).unwrap(),
        // States of the feedbacks.
        // They are the data related to the feedbacks that you want to persist in the State.
        tuple_list!(),
    );

    // The Stats trait define how the fuzzer stats are reported to the user
    let stats = SimpleStats::new(|s| println!("{}", s));

    // The event manager handle the various events generated during the fuzzing loop
    // such as the notification of the addition of a new item to the corpus
    let mut mgr = SimpleEventManager::new(stats);

    // A minimization+queue policy to get testcasess from the corpus
    let scheduler = QueueCorpusScheduler::new();

    // A fuzzer with feedbacks and a corpus scheduler
    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    let mut executor =
        CommandExecutor::from_observers(tuple_list!(time_observer, concolic_observer));

    state
        .corpus_mut()
        .add(Testcase::new(BytesInput::new(vec![1, 2, 3, 4])))
        .unwrap();

    // Setup a mutational stage with a basic bytes mutator
    let mutator = StdScheduledMutator::new(havoc_mutations());
    let mut stages = tuple_list!(StdMutationalStage::new(mutator));

    fuzzer
        .fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
        .expect("Error in the fuzzing loop");
}