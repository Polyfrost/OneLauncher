use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

pub type Set = BTreeSet<String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Fail,
    Good,
    Unsure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanKind {
    NewBisection,
    Continuation,
    Complement,
    Verification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub kind: PlanKind,
    pub stable: Set,
    pub first: Vec<String>,
    pub second: Vec<String>,
    pub conflict: Set,
}

impl Plan {
    #[must_use]
    pub fn targets(&self) -> Set {
        match self.kind {
            PlanKind::Verification => self.conflict.clone(),
            PlanKind::Complement => self.stable.iter().chain(&self.second).cloned().collect(),
            PlanKind::Continuation | PlanKind::NewBisection => {
                self.stable.iter().chain(&self.first).cloned().collect()
            }
        }
    }

    #[must_use]
    pub fn tested_half(&self) -> &[String] {
        match self.kind {
            PlanKind::Complement => &self.second,
            PlanKind::Verification => &[],
            _ => &self.first,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub stable: Set,
    pub candidates: Vec<String>,
}

impl Step {
    fn new(stable: Set, mut candidates: Vec<String>) -> Self {
        candidates.sort();
        Self { stable, candidates }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub round: u32,
    pub iteration: u32,
    pub step: u32,
    pub unsure_count: u32,
    pub conflict: Set,
    pub candidates: Vec<String>,
    pub stable: Set,
    pub stack: Vec<Step>,
    pub handling_unsure: bool,
    pub verifying: bool,
    pub halted: bool,
    pub complete: bool,
    pub total: usize,
}

impl State {
    #[must_use]
    pub fn new(candidates: Set, round: u32) -> Self {
        Self {
            round,
            iteration: 1,
            step: 1,
            unsure_count: 0,
            conflict: Set::new(),
            total: candidates.len(),
            candidates: candidates.into_iter().collect(),
            stable: Set::new(),
            stack: Vec::new(),
            handling_unsure: false,
            verifying: false,
            halted: false,
            complete: false,
        }
    }

    #[must_use]
    pub fn candidate_set(&self) -> Set {
        match self.stack.last() {
            Some(step) => step.candidates.iter().cloned().collect(),
            None => self.candidates.iter().cloned().collect(),
        }
    }

    #[must_use]
    pub fn cleared_set(&self) -> Set {
        let stable = match self.stack.last() {
            Some(step) => &step.stable,
            None => &self.stable,
        };
        stable.difference(&self.conflict).cloned().collect()
    }

    fn unsure_context(&self) -> Step {
        match self.stack.last() {
            Some(step) => step.clone(),
            None => Step::new(self.conflict.clone(), self.candidates.clone()),
        }
    }

    #[must_use]
    pub fn plan(&self) -> Option<Plan> {
        if self.complete || self.halted {
            return None;
        }
        let make = |kind, stable: Set, candidates: &[String]| {
            let (first, second) = split(candidates);
            Some(Plan {
                kind,
                stable,
                first,
                second,
                conflict: self.conflict.clone(),
            })
        };
        if self.handling_unsure {
            let context = self.unsure_context();
            return make(PlanKind::Complement, context.stable, &context.candidates);
        }
        if let Some(step) = self.stack.last() {
            return make(
                PlanKind::Continuation,
                step.stable.clone(),
                &step.candidates,
            );
        }
        if self.verifying {
            return Some(Plan {
                kind: PlanKind::Verification,
                stable: Set::new(),
                first: Vec::new(),
                second: Vec::new(),
                conflict: self.conflict.clone(),
            });
        }
        if self.candidates.is_empty() {
            return None;
        }
        make(
            PlanKind::NewBisection,
            self.conflict.clone(),
            &self.candidates,
        )
    }

    fn found(&mut self, id: &str) {
        self.conflict.insert(id.to_string());
        self.stable = self.conflict.clone();
        self.candidates.retain(|c| c != id);
        self.stack.clear();
        self.handling_unsure = false;
        self.verifying = true;
    }

    fn exhaust(&mut self) {
        self.stack.clear();
        self.complete = true;
    }

    fn replace_top(&mut self, step: Step) {
        self.stack.pop();
        self.stack.push(step);
    }

    #[must_use]
    pub fn apply(&self, plan: &Plan, outcome: Outcome) -> Self {
        let mut next = self.clone();
        next.verifying = false;
        next.step += 1;

        if plan.kind == PlanKind::Verification {
            if outcome == Outcome::Unsure {
                next.verifying = true;
                next.unsure_count += 1;
                return next;
            }
            next.step = 0;
            if outcome == Outcome::Fail {
                next.complete = true;
            } else {
                next.iteration += 1;
            }
            next.handling_unsure = false;
            return next;
        }

        if outcome == Outcome::Unsure {
            next.unsure_count += 1;
        }

        if self.handling_unsure {
            let context = self.unsure_context();
            let (first, second) = split(&context.candidates);
            next.handling_unsure = false;
            match outcome {
                Outcome::Fail => {
                    if let [only] = second.as_slice() {
                        next.found(only);
                    } else {
                        next.replace_top(Step::new(context.stable, second));
                    }
                }
                Outcome::Good => {
                    let stable = context.stable.iter().chain(&second).cloned().collect();
                    next.replace_top(Step::new(stable, first));
                }
                Outcome::Unsure => next.halted = true,
            }
            return next;
        }

        let current = self.unsure_context();
        let (first, second) = split(&current.candidates);
        match outcome {
            Outcome::Fail => {
                if let [only] = first.as_slice() {
                    next.found(only);
                } else {
                    next.stack.push(Step::new(current.stable, first));
                }
            }
            Outcome::Good => {
                next.stack.pop();
                if second.is_empty() {
                    next.exhaust();
                } else {
                    let stable = current.stable.iter().chain(&first).cloned().collect();
                    next.stack.push(Step::new(stable, second));
                }
            }
            Outcome::Unsure => {
                if second.is_empty() {
                    next.halted = true;
                } else {
                    next.handling_unsure = true;
                }
            }
        }
        next
    }

    pub fn remove(&mut self, gone: &Set) {
        self.conflict.retain(|c| !gone.contains(c));
        self.stable.retain(|c| !gone.contains(c));
        self.candidates.retain(|c| !gone.contains(c));
        for step in &mut self.stack {
            step.stable.retain(|c| !gone.contains(c));
            step.candidates.retain(|c| !gone.contains(c));
        }
        self.stack.retain(|step| !step.candidates.is_empty());
    }

    pub fn add(&mut self, joined: &Set) {
        let mut all: Set = self.candidates.iter().cloned().collect();
        all.extend(joined.iter().cloned());
        self.candidates = all.into_iter().collect();
    }

    #[must_use]
    pub fn estimate(&self) -> u32 {
        let found = self.conflict.len() + usize::from(!self.complete);
        if self.total == 0 || found == 0 {
            return 0;
        }
        let log = usize::BITS - self.total.saturating_sub(1).leading_zeros();
        u32::try_from(found).unwrap_or(u32::MAX) * (log + 1) + self.unsure_count
    }
}

pub(super) fn split(items: &[String]) -> (Vec<String>, Vec<String>) {
    let mid = items.len().div_ceil(2);
    (items[..mid].to_vec(), items[mid..].to_vec())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Test {
    pub plan: Plan,
    pub outcome: Outcome,
    pub before: State,
    pub enabled: Set,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: usize) -> Set {
        (0..n).map(|i| format!("m{i:02}")).collect()
    }

    fn run(n: usize, broken: impl Fn(&Set) -> Option<bool>) -> State {
        let mut state = State::new(ids(n), 1);
        for _ in 0..500 {
            let Some(plan) = state.plan() else {
                return state;
            };
            let outcome = match broken(&plan.targets()) {
                Some(true) => Outcome::Fail,
                Some(false) => Outcome::Good,
                None => Outcome::Unsure,
            };
            state = state.apply(&plan, outcome);
        }
        panic!("search did not finish");
    }

    #[test]
    fn finds_every_single_culprit() {
        for culprit in 0..13 {
            let name = format!("m{culprit:02}");
            let state = run(13, |on| Some(on.contains(&name)));
            assert!(state.complete);
            assert_eq!(state.conflict, Set::from([name]));
        }
    }

    #[test]
    fn finds_a_pair_that_only_breaks_together() {
        for a in 0..10 {
            for b in (a + 1)..10 {
                let (x, y) = (format!("m{a:02}"), format!("m{b:02}"));
                let state = run(10, |on| Some(on.contains(&x) && on.contains(&y)));
                assert!(state.complete, "{x} {y}");
                assert_eq!(state.conflict, Set::from([x, y]));
            }
        }
    }

    #[test]
    fn finds_a_triple() {
        let wanted = Set::from(["m02".to_string(), "m07".to_string(), "m11".to_string()]);
        let state = run(16, |on| Some(wanted.is_subset(on)));
        assert_eq!(state.conflict, wanted);
    }

    #[test]
    fn no_problem_means_an_empty_result() {
        let state = run(8, |_| Some(false));
        assert!(state.complete);
        assert!(state.conflict.is_empty());
    }

    #[test]
    fn a_secondary_crash_is_worked_around() {
        let culprit = "m05".to_string();
        let secondary = "m01".to_string();
        let state = run(12, |on| {
            if on.contains(&secondary) && !on.contains("m00") {
                None
            } else {
                Some(on.contains(&culprit))
            }
        });
        assert!(state.complete || state.halted);
        if state.complete {
            assert!(state.conflict.contains(&culprit));
        }
    }

    #[test]
    fn an_unclear_check_asks_again_instead_of_blaming_another_mod() {
        let culprit = "m03".to_string();
        let mut state = State::new(ids(8), 1);
        let mut unsure_once = true;
        for _ in 0..100 {
            let Some(plan) = state.plan() else { break };
            let outcome = if plan.kind == PlanKind::Verification && unsure_once {
                unsure_once = false;
                Outcome::Unsure
            } else if plan.targets().contains(&culprit) {
                Outcome::Fail
            } else {
                Outcome::Good
            };
            state = state.apply(&plan, outcome);
        }
        assert!(state.complete);
        assert_eq!(state.conflict, Set::from([culprit]));
    }

    #[test]
    fn an_unclear_last_suspect_halts_instead_of_finishing() {
        let state = run(1, |_| None);
        assert!(state.halted);
        assert!(!state.complete);
    }

    #[test]
    fn removing_candidates_prunes_the_stack() {
        let mut state = State::new(ids(8), 1);
        let plan = state.plan().unwrap();
        state = state.apply(&plan, Outcome::Fail);
        state.remove(&plan.first.iter().cloned().collect());
        assert!(state.stack.is_empty());
    }

    #[test]
    fn removing_a_cleared_mod_drops_it_from_the_next_test() {
        let mut state = State::new(ids(8), 1);
        let plan = state.plan().unwrap();
        state = state.apply(&plan, Outcome::Good);
        let cleared = plan.first[0].clone();
        state.remove(&Set::from([cleared.clone()]));
        assert!(!state.plan().unwrap().targets().contains(&cleared));
    }

    #[test]
    fn estimate_counts_log2_launches() {
        assert_eq!(State::new(ids(100), 1).estimate(), 8);
        assert_eq!(State::new(ids(1), 1).estimate(), 1);
    }
}
