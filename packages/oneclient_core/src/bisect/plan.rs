use std::collections::BTreeSet;

pub type ModSet = BTreeSet<usize>;

pub fn closure(graph: &[Vec<usize>], roots: impl IntoIterator<Item = usize>) -> ModSet {
    let mut out = ModSet::new();
    let mut stack: Vec<usize> = roots.into_iter().collect();
    while let Some(node) = stack.pop() {
        if node >= graph.len() || !out.insert(node) {
            continue;
        }
        stack.extend(graph[node].iter().copied());
    }
    out
}

pub fn next_round(graph: &[Vec<usize>], suspects: &ModSet) -> Option<ModSet> {
    if suspects.len() < 2 {
        return None;
    }

    let half = suspects.len() / 2;
    let mut ordered: Vec<(usize, usize)> = suspects
        .iter()
        .map(|&s| (closure(graph, [s]).intersection(suspects).count(), s))
        .collect();
    ordered.sort_unstable();

    let mut roots: Vec<usize> = Vec::new();
    let mut enabled = ModSet::new();
    for (_, candidate) in ordered {
        let mut next_roots = roots.clone();
        next_roots.push(candidate);
        let next = closure(graph, next_roots.iter().copied());
        let covered = next.intersection(suspects).count();
        if covered >= suspects.len() {
            continue;
        }
        if !roots.is_empty() && covered > half {
            continue;
        }
        roots = next_roots;
        enabled = next;
        if covered >= half {
            break;
        }
    }

    (!roots.is_empty()).then_some(enabled)
}

pub fn narrow(suspects: &ModSet, enabled: &ModSet, broken: bool) -> ModSet {
    if broken {
        suspects.intersection(enabled).copied().collect()
    } else {
        suspects.difference(enabled).copied().collect()
    }
}

pub fn launches_left(suspects: usize) -> u32 {
    if suspects <= 1 {
        0
    } else {
        usize::BITS - (suspects - 1).leading_zeros()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[usize]) -> ModSet {
        items.iter().copied().collect()
    }

    #[test]
    fn splits_independent_mods_in_half() {
        let graph = vec![Vec::new(); 8];
        let suspects = set(&[0, 1, 2, 3, 4, 5, 6, 7]);
        let enabled = next_round(&graph, &suspects).unwrap();
        assert_eq!(enabled.len(), 4);
    }

    #[test]
    fn a_tested_mod_brings_its_library() {
        let mut graph = vec![Vec::new(); 4];
        graph[0] = vec![3];
        let suspects = set(&[0, 1, 2, 3]);
        let enabled = next_round(&graph, &suspects).unwrap();
        assert!(!enabled.contains(&0) || enabled.contains(&3));
    }

    #[test]
    fn finds_any_single_culprit() {
        let mut graph = vec![Vec::new(); 13];
        graph[2] = vec![9];
        graph[5] = vec![9, 11];
        graph[7] = vec![2];
        for culprit in 0..13 {
            let mut suspects: ModSet = (0..13).collect();
            let mut launches = 0;
            while let Some(enabled) = next_round(&graph, &suspects) {
                launches += 1;
                assert!(launches < 20);
                let broken = enabled.contains(&culprit);
                suspects = narrow(&suspects, &enabled, broken);
            }
            assert_eq!(suspects, set(&[culprit]), "culprit {culprit}");
        }
    }

    #[test]
    fn a_cycle_that_covers_every_suspect_cannot_split() {
        let graph = vec![vec![1], vec![0]];
        assert_eq!(next_round(&graph, &set(&[0, 1])), None);
    }

    #[test]
    fn one_suspect_is_the_answer() {
        assert_eq!(next_round(&[Vec::new()], &set(&[0])), None);
    }

    #[test]
    fn counts_launches_as_log2() {
        assert_eq!(launches_left(1), 0);
        assert_eq!(launches_left(2), 1);
        assert_eq!(launches_left(3), 2);
        assert_eq!(launches_left(100), 7);
        assert_eq!(launches_left(128), 7);
    }
}
