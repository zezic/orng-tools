// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Applying, off the interface thread.
//!
//! Computing a plan reads the whole archive and resolves every anchor, and
//! applying one patches, verifies under Bitwig's own JVM and moves files into
//! place. Seconds either way, and on the interface thread that is a window that
//! stops answering during the one operation a user most wants to watch.
//!
//! So it runs on another thread and reports back. The interface holds the
//! progress and never the plan, which also means it cannot be tempted to ask the
//! plan a question halfway through.
//!
//! The worker wakes the window itself. An `egui::Context` is a handle that can
//! be cloned across threads, and `request_repaint` on it is how a thread that
//! is not the drawing one says there is something new to draw. The alternative
//! is for the window to wake on a timer and look - which burns a wakeup ten
//! times a second through a verification that takes half a minute, and still
//! answers late.

use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread;

use eframe::egui;
use orng_tools::{Destination, Manifest, Plan, Step, Update};

/// What one press of the primary action has to do.
///
/// Not a flag on the worker, because the two differ in what they may do rather
/// than in how they are drawn: one modifies the installation and needs Bitwig
/// closed, the other writes files and does not (decision 6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Work {
    /// The installation already reads the entry list. Only the entries change,
    /// which is a handful of file writes.
    Entries,
    /// The installation does not read the entry list yet, so it is prepared
    /// first and the entries follow - in the same press, because a fresh
    /// installation with documents waiting needs both and the order between
    /// them is not the user's to get right.
    ///
    /// The entry write is not an afterthought here. A Bitwig update replaces
    /// the installation wholesale, description bundles included, so the write
    /// that follows a re-preparation is what puts the descriptions back.
    PrepareThenEntries,
}

/// What the worker says as it goes.
enum Progress {
    /// Which steps this plan will actually run. Sent once, before the first of
    /// them begins, because a step that will be skipped has to be drawn as
    /// skipped rather than go missing and change the count under the reader.
    Planned(Vec<Step>),
    /// This step has started. The one before it is therefore finished.
    Began(Step),
    /// The installation is prepared, and the entries are being written.
    Registering,
    /// Nothing more is coming. On success this carries the entry list as it now
    /// stands on disk, so the window can show what was written rather than read
    /// it back or work it out again.
    Finished(Result<Manifest, String>),
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

/// Which half of the work is going on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Preparing,
    /// Placing documents, writing the entry list and the description bundles.
    Registering,
}

/// Work in flight, and then its result.
pub struct Applying {
    updates: Receiver<Progress>,
    /// Every step of the preparation, in the order they run - or `None` when
    /// this press prepares nothing. The design gives an entries-only apply a
    /// line in the action bar rather than a step list, because it is instant
    /// and there is nothing to watch.
    pub steps: Option<[(Step, State); 5]>,
    pub stage: Stage,
    /// `None` while it is still going, and then the entry list that was written.
    pub outcome: Option<Result<Manifest, String>>,
}

impl Applying {
    /// Start it. Returns immediately.
    pub fn start(
        work: Work,
        to: Destination,
        update: Update,
        ctx: egui::Context,
    ) -> Applying {
        let (tx, updates) = channel();
        thread::spawn(move || {
            // Every send is followed by a wake, so the window redraws when
            // something happened and stays asleep when nothing did.
            let say = move |progress| {
                let _ = tx.send(progress);
                ctx.request_repaint();
            };
            let result = run(work, &to, update, &say);
            say(Progress::Finished(result.map_err(|e| e.to_string())));
        });

        Applying {
            updates,
            // Every step, waiting, until the plan says which it skips.
            steps: match work {
                Work::PrepareThenEntries => Some(Step::ALL.map(|step| (step, State::Waiting))),
                Work::Entries => None,
            },
            stage: match work {
                Work::PrepareThenEntries => Stage::Preparing,
                Work::Entries => Stage::Registering,
            },
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
                        self.finish(Err("the work stopped without reporting".to_owned()));
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
                for (step, state) in self.steps.iter_mut().flatten() {
                    if !will_run.contains(step) {
                        *state = State::NotRun;
                    }
                }
            }
            Progress::Began(began) => {
                // A step starting is what says the one before it finished: the
                // library reports a step as it begins, and the call returning is
                // what says the last one is done.
                for (step, state) in self.steps.iter_mut().flatten() {
                    if *state == State::Running {
                        *state = State::Done;
                    }
                    if *step == began {
                        *state = State::Running;
                    }
                }
            }
            // The last step reported is only finished once the next thing
            // starts, and after the last one that next thing is this.
            Progress::Registering => {
                for (_, state) in self.steps.iter_mut().flatten() {
                    if *state == State::Running {
                        *state = State::Done;
                    }
                }
                self.stage = Stage::Registering;
            }
            Progress::Finished(result) => self.finish(result),
        }
    }

    fn finish(&mut self, result: Result<Manifest, String>) {
        let failed = result.is_err();
        for (_, state) in self.steps.iter_mut().flatten() {
            if *state == State::Running {
                *state = if failed { State::Failed } else { State::Done };
            }
        }
        self.outcome = Some(result);
    }

    pub fn is_running(&self) -> bool {
        self.outcome.is_none()
    }

    /// Whether this press touched the installation, which is what decides
    /// whether the machine has to be read again afterwards.
    pub fn prepared(&self) -> bool {
        self.steps.is_some()
    }

    /// One held still in a given state, for drawing it without running
    /// anything. Tests only: real work writes to an installation, and rendering
    /// a picture of one must not.
    #[cfg(test)]
    pub fn frozen(
        steps: Option<[(Step, State); 5]>,
        stage: Stage,
        outcome: Option<Result<Manifest, String>>,
    ) -> Applying {
        let (tx, updates) = channel();
        // The sender is kept alive on purpose. Dropping it disconnects the
        // channel, and a disconnected channel with no outcome means the worker
        // died, which `poll` correctly turns into a failure - so a held-still
        // run would draw itself as one the moment it was polled.
        std::mem::forget(tx);
        Applying { updates, steps, stage, outcome }
    }
}

/// The whole of what a press does, in the order it does it.
///
/// A plan that cannot be computed has written nothing, which is the property the
/// transaction exists to have, so it leaves the run failed with no step failed:
/// none ran.
fn run(
    work: Work,
    to: &Destination,
    update: Update,
    say: &impl Fn(Progress),
) -> orng_tools::Result<Manifest> {
    if work == Work::PrepareThenEntries {
        let plan = Plan::compute(to)?;
        say(Progress::Planned(plan.steps().collect()));
        plan.apply(|step| say(Progress::Began(step)))?;
        say(Progress::Registering);
    }
    update.apply(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build one without a worker, to drive the state machine by hand.
    fn idle() -> Applying {
        let (_tx, updates) = channel();
        Applying {
            updates,
            steps: Some(Step::ALL.map(|s| (s, State::Waiting))),
            stage: Stage::Preparing,
            outcome: None,
        }
    }

    fn state_of(p: &Applying, want: Step) -> State {
        p.steps
            .as_ref()
            .expect("this one prepares")
            .iter()
            .find(|(s, _)| *s == want)
            .expect("every step is listed")
            .1
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
        assert_eq!(
            p.steps.expect("this one prepares").len(),
            Step::ALL.len(),
            "a skipped step must not go missing"
        );
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

    /// The last step is finished by the entry write starting, exactly as every
    /// other step is finished by the next one starting. Without that, a
    /// successful preparation would sit with its final step still marked
    /// running for as long as the entries take to write.
    #[test]
    fn registering_finishes_the_last_step_of_the_preparation() {
        let mut p = idle();
        p.apply(Progress::Began(Step::Link));
        p.apply(Progress::Registering);

        assert_eq!(state_of(&p, Step::Link), State::Done);
        assert_eq!(p.stage, Stage::Registering);
        assert!(p.is_running(), "registering is not the end of the work");
    }

    /// A worker that dies without reporting must not read as success. It would
    /// otherwise show five green steps for an installation nothing was done to.
    #[test]
    fn a_worker_that_vanishes_is_a_failure_and_not_a_success() {
        let (tx, updates) = channel();
        let mut p = Applying {
            updates,
            steps: Some(Step::ALL.map(|s| (s, State::Waiting))),
            stage: Stage::Preparing,
            outcome: None,
        };
        tx.send(Progress::Began(Step::Backup)).unwrap();
        drop(tx);

        assert!(p.poll());
        assert!(!p.is_running());
        assert!(p.outcome.as_ref().expect("it ended").is_err());
        assert_eq!(state_of(&p, Step::Backup), State::Failed);
    }

    /// An entries-only apply has no steps at all, and the state machine has to
    /// survive being asked about them anyway.
    #[test]
    fn an_entries_only_apply_has_no_step_list_to_draw() {
        let (tx, updates) = channel();
        let mut p =
            Applying { updates, steps: None, stage: Stage::Registering, outcome: None };
        tx.send(Progress::Finished(Ok(Manifest::default()))).unwrap();

        assert!(p.poll());
        assert!(!p.is_running());
        assert!(p.outcome.as_ref().expect("it ended").is_ok());
        assert!(!p.prepared(), "an entries-only apply must not ask for a re-read");
    }
}
