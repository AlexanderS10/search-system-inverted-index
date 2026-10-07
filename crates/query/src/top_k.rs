//! Top-K Heap Accumulator
//!
//! This will maintain a top-k highest scoring documents using a BOUNDED min-heap
//!
//! The root of the heap will store the lowest score currently in the top-k
//! Any new candiate with a score lower than or equal to this threshold is discarded in O(1)
//!
use std::cmp::Ordering;
use std::collections::BinaryHeap;

//A candidate document scored by BM25
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScoredDoc {
    pub doc_id: u32,
    pub score: f64,
}
// rust requires here Eq for types implementing Ord (yes fight the compiler 101)
impl Eq for ScoredDoc {}

//IMplement Ord to turn BinaryHeap (whcih is a max-heap) into a min-heap
impl Ord for ScoredDoc {
    fn cmp(&self, other: &Self) -> Ordering {
        //reverse the order for a min-heap
        return other
            .score
            .total_cmp(&self.score)
            .then_with(|| other.doc_id.cmp(&self.doc_id));
    }
}

impl PartialOrd for ScoredDoc {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        return Some(self.cmp(other));
    }
}

pub struct TopKHeap {
    k: usize,
    heap: BinaryHeap<ScoredDoc>,
}
impl TopKHeap {
    ///Creates a new top-k collector with given k capacity
    pub fn new(k: usize) -> Self {
        return Self {
            k,
            heap: BinaryHeap::with_capacity(k),
        };
    }

    ///Evaluates the cadidate doc against the top-k
    pub fn push(&mut self, doc_id: u32, score: f64) {
        if self.k == 0 {
            return;
        }
        let candidate = ScoredDoc { doc_id, score };
        if self.heap.len() < self.k {
            self.heap.push(candidate);
        } else if let Some(min_doc) = self.heap.peek() {
            if score > min_doc.score {
                self.heap.pop();
                self.heap.push(candidate);
            }
        }
    }

    ///Returns the lowest score currently in the heap
    #[allow(dead_code)]
    pub fn min_score(&self) -> f64 {
        if self.heap.len() < self.k {
            return 0.0;
        }
        return self.heap.peek().map(|d| d.score).unwrap_or(0.0);
    }

    ///Consumes the heap and returns results sorted from highest to lowest score
    pub fn into_sorted_vec(self) -> Vec<ScoredDoc> {
        let mut results = self.heap.into_vec();
        results.sort_by(|a, b| b.score.total_cmp(&a.score));
        return results;
    }
}
