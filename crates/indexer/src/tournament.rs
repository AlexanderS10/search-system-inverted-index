//! loser tree used to choose the next posting written to postings.bin
//! winners (smallest keys) climb during replay; losers stay in internal nodes
//! until sort is complete. log k sorting... replay strategy (should)
//! result in fewer comparison's than min heap.

pub(crate) struct TournamentTree {
    // internal nodes start at one, each node holds the loser
    // from the match played there. winner held separate
    losers: Vec<Option<usize>>,
    winner: Option<usize>,
    leaf_count: usize,
}

impl TournamentTree {
    // init me
    pub(crate) fn new(slot_count: usize) -> Self {
        let leaf_count = slot_count.max(1).next_power_of_two();
        Self {
            // one entry per internal node index range.
            // 1-indexed so p = node / 2, l = node*2, r= node*2+1
            losers: vec![None; leaf_count],
            winner: None,
            leaf_count,
        }
    }

    /// key must be sortable, keyed on term followed by doc_id
    pub(crate) fn build<K: Ord>(slot_count: usize, key: impl Fn(usize) -> Option<K>) -> Self {
        // initial population, winners is temp holder
        let mut tree = Self::new(slot_count);
        // holds idx positions
        let mut winners = vec![None; tree.leaf_count * 2];

        // fill leaves
        for slot in 0..slot_count {
            winners[tree.leaf_count + slot] = key(slot).map(|_| slot);
        }
        // walk nodes bottom up
        for node in (1..tree.leaf_count).rev() {
            //play l vs. r, assign
            let (winner, loser) = play(winners[node * 2], winners[node * 2 + 1], &key);
            winners[node] = winner;
            tree.losers[node] = loser;
        }
        // move overall winner to tree's storing
        tree.winner = winners[1];
        tree
    }

    /// get current smallest key value idx.
    pub(crate) fn winner(&self) -> Option<usize> {
        self.winner
    }

    /// recompute the winner after last winner advances.
    pub(crate) fn update<K: Ord>(&mut self, slot: usize, key: impl Fn(usize) -> Option<K>) {
        debug_assert_eq!(self.winner, Some(slot));
        // the changed slot is the new challenger if it has some posting
        //if run exhausted, eq none
        let mut challenger = key(slot).map(|_| slot);
        //start @ parent
        let mut node = self.leaf_count + slot;
        node /= 2;
        //replay up from current position.
        while node > 0 {
            let (winner, loser) = play(challenger, self.losers[node], &key);
            challenger = winner;
            self.losers[node] = loser;
            node /= 2;
        }

        self.winner = challenger;
    }
}

// given two idxs, return winner loser, optional because
// one side may be none
// tournament function calls the comparison logic.
fn play<K: Ord>(
    left: Option<usize>,
    right: Option<usize>,
    key: impl Fn(usize) -> Option<K>,
) -> (Option<usize>, Option<usize>) {
    match (left, right) {
        (None, None) => (None, None),
        (None, Some(winner)) | (Some(winner), None) => (Some(winner), None),
        (Some(left), Some(right)) => {
            if key(left) <= key(right) {
                (Some(left), Some(right))
            } else {
                (Some(right), Some(left))
            }
        }
    }
}
