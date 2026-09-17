// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Preparation, off the interface thread.
//!
//! Computing a plan reads the whole archive and resolves every anchor, and
//! applying one patches, verifies under Bitwig's own JVM and moves files into
//! place. Seconds either way, and on the interface thread that is a window that
//! stops answering during the one operation a user most wants to watch.
//!
//! So it runs on another thread and reports back. The interface holds the
//! progress and never the plan, which also means it cannot be tempted to ask the
//! plan a question halfway through.

use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread;

use orng_tools::{Installation, OrngHome, Plan, Step, Strategy, UserLibrary};

/// What the worker says as it goes.
enum Progress {
    /// Which steps this plan will actually run. Sent once, before the first of
    /// them begins, because a step that will be skipped has to be drawn as
    /// skipped rather than go missing and change the count under the reader.
    Planned(Vec<Step>),
    /// This step has started. The one before it is therefore finished.
    Began(Step),
    /// Nothing more is coming.
    Finished(Result<(), String>),
}

/// What has happened to one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// This plan will run it, and has not reached it.
    Waiting,
    /// This plan does not run it. Drawn, and drawn as not run.
    NotRun,
    Running,
    Done,
    /// The run stopped here.
    Failed,
}

/// A preparation in flight, and then its result.
pub struct Preparing {
    updates: Receiver<Progress>,
    /// Every step there is, in the order they run.
    pub steps: [(Step, State); 5],
    /// `None` while it is still going.
    pub outcome: Option<Result<(), String>>,
}

impl Preparing {
    /// Start one. Returns immediately.
    pub fn start(
        install: Installation,
        library: UserLibrary,
        home: OrngHome,
        placement: Strategy,
    ) -> Preparing {
        let (tx, updates) = channel();
        thread::spawn(move || {
            let plan = match Plan::compute(&install, &library, &home, placement) {
                Ok(plan) => plan,
                // A plan that cannot be computed has written nothing, which is
                // the property the transaction exists to have. Report it as the
                // whole run failing rather than as a step failing, because no
                // step ran.
                Err(e) => {
                    let _ = tx.send(Progress::Finished(Err(e.to_string())));
                    return;
                }
            };
            let _ = tx.send(Progress::Planned(plan.steps().collect()));

            let reporter = tx.clone();
            let result = plan.apply(|step| {
                let _ = reporter.send(Progress::Began(step));
            });
            let _ = tx.send(Progress::Finished(result.map_err(|e| e.to_string())));
        });

        Preparing {
            updates,
            // Every step, waiting, until the plan says which it skips.
            steps: Step::ALL.map(|step| (step, State::Waiting)),
            outcome: None,
        }
    }

    /// Take whatever the worker has said since last time.
    ///
    /// Returns whether anything moved, so the caller can ask for a repaint only
    /// when there is something new to draw.
    pub fn poll(&mut self) -> bool {
        let mut moved = false;
        loop {
            match self.updates.try_recv() {
                Ok(progress) => {
                    self.apply(progress);
                    moved = true;
                }
                // The worker is still running and has nothing new to say.
                Err(TryRecvError::Empty) => break,
                // The sender is gone. If it never said how it ended, it died
                // without reporting, and silence must not read as success.
                Err(TryRecvError::Disconnected) => {
                    if self.outcome.is_none() {
                        self.finish(Err("preparation stopped without reporting".to_owned()));
                        moved = true;
                    }
                    break;
                }
            }
        }
        moved
    }

    fn apply(&mut self, progress: Progress) {
        match progress {
            Progress::Planned(will_run) => {
                for (step, state) in &mut self.steps {
                    if !will_run.contains(step) {
                        *state = State::NotRun;
                    }
                }
            }
            Progress::Began(began) => {
                // A step starting is what says the one before it finished: the
                // library reports a step as it begins, and the call returning is
                // what says the last one is done.
                for (step, state) in &mut self.steps {
                    if *state == State::Running {
                        *state = State::Done;
                    }
                    if *step == began {
                        *state = State::Running;
                    }
                }
            }
            Progress::Finished(result) => self.finish(result),
        }
    }

    fn finish(&mut self, result: Result<(), String>) {
        let failed = result.is_err();
        for (_, state) in &mut self.steps {
            if *state == State::Running {
                *state = if failed { State::Failed } else { State::Done };
            }
        }
        self.outcome = Some(result);
    }

    pub fn is_running(&self) -> bool {
        self.outcome.is_none()
    }

    /// One held still in a given state, for drawing it without running
    /// anything. Tests only: a real preparation writes to an installation, and
    /// rendering a picture of one must not.
    #[cfg(test)]
    pub fn frozen(steps: [(Step, State); 5], outcome: Option<Result<(), String>>) -> Preparing {
        let (tx, updates) = channel();
        // The sender is kept alive on purpose. Dropping it disconnects the
        // channel, and a disconnected channel with no outcome means the worker
        // died, which `poll` correctly turns into a failure - so a held-still
        // preparation would draw itself as one the moment it was polled.
        std::mem::forget(tx);
        Preparing { updates, steps, outcome }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build one without a worker, to drive the state machine by hand.
    fn idle() -> Preparing {
        let (_tx, updates) = channel();
        Preparing { updates, steps: Step::ALL.map(|s| (s, State::Waiting)), outcome: None }
    }

    fn state_of(p: &Preparing, want: Step) -> State {
        p.steps.iter().find(|(s, _)| *s == want).expect("every step is listed").1
    }

    #[test]
    fn a_step_beginning_finishes_the_one_before_it() {
        let mut p = idle();
        p.apply(Progress::Began(Step::Backup));
        assert_eq!(state_of(&p, Step::Backup), State::Running);

        p.apply(Progress::Began(Step::Patch));
        assert_eq!(state_of(&p, Step::Backup), State::Done);
        assert_eq!(state_of(&p, Step::Patch), State::Running);
        assert_eq!(state_of(&p, Step::Verify), State::Waiting);
    }

    #[test]
    fn a_step_this_plan_skips_is_drawn_as_not_run() {
        let mut p = idle();
        // Under the copy strategy the link step is not run. It still has a row.
        p.apply(Progress::Planned(vec![Step::Backup, Step::Patch, Step::Verify, Step::Activate]));
        assert_eq!(state_of(&p, Step::Link), State::NotRun);
        assert_eq!(state_of(&p, Step::Backup), State::Waiting);
        assert_eq!(p.steps.len(), Step::ALL.len(), "a skipped step must not go missing");
    }

    #[test]
    fn the_step_that_was_running_when_it_failed_is_the_one_marked() {
        let mut p = idle();
        p.apply(Progress::Began(Step::Backup));
        p.apply(Progress::Began(Step::Patch));
        p.apply(Progress::Finished(Err("the archive did not verify".to_owned())));

        assert_eq!(state_of(&p, Step::Backup), State::Done);
        assert_eq!(state_of(&p, Step::Patch), State::Failed);
        assert_eq!(state_of(&p, Step::Verify), State::Waiting, "a step after the failure ran");
        assert!(!p.is_running());
    }

    /// A worker that dies without reporting must not read as success. It would
    /// otherwise show five green steps for an installation nothing was done to.
    #[test]
    fn a_worker_that_vanishes_is_a_failure_and_not_a_success() {
        let (tx, updates) = channel();
        let mut p =
            Preparing { updates, steps: Step::ALL.map(|s| (s, State::Waiting)), outcome: None };
        tx.send(Progress::Began(Step::Backup)).unwrap();
        drop(tx);

        assert!(p.poll());
        assert!(!p.is_running());
        assert!(p.outcome.as_ref().expect("it ended").is_err());
        assert_eq!(state_of(&p, Step::Backup), State::Failed);
    }
}
