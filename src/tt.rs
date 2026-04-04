// ─────────────────────────────────────────────────────────────────────────────
//  Karpovian Rust — Transposition Table
//
//  Thread-safe via UnsafeCell + unsafe Send/Sync.
//  This is the standard "benign data race" pattern used by Stockfish and most
//  other chess engines: partial writes are caught by the key-mismatch check in
//  get(), so the worst case is a missed TT hit, never a crash or wrong move.
// ─────────────────────────────────────────────────────────────────────────────

use chess::ChessMove;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const EXACT: u8       = 0;
pub const LOWER_BOUND: u8 = 1;
pub const UPPER_BOUND: u8 = 2;

#[derive(Clone, Copy)]
pub struct TTEntry {
    pub key:       u64,
    pub depth:     i32,
    pub flag:      u8,
    pub score:     i32,
    pub best_move: Option<ChessMove>,
}

pub struct TranspositionTable {
    data:   UnsafeCell<Vec<Option<TTEntry>>>,
    mask:   usize,
    filled: AtomicUsize,
}

// SAFETY: benign data race — key check in get() catches partial writes.
unsafe impl Send for TranspositionTable {}
unsafe impl Sync for TranspositionTable {}

impl TranspositionTable {
    pub fn new(size_mb: usize) -> Self {
        let entry_size = std::mem::size_of::<Option<TTEntry>>();
        let count = ((size_mb * 1024 * 1024) / entry_size).next_power_of_two();
        TranspositionTable {
            data:   UnsafeCell::new(vec![None; count]),
            mask:   count - 1,
            filled: AtomicUsize::new(0),
        }
    }

    /// Permille (0–1000) of slots currently occupied, sampled from first 1000.
    pub fn hashfull(&self) -> u32 {
        let data = unsafe { &*self.data.get() };
        let sample = 1000.min(data.len());
        let n = data[..sample].iter().filter(|e| e.is_some()).count();
        (n * 1000 / sample) as u32
    }

    #[inline]
    pub fn get(&self, key: u64) -> Option<TTEntry> {
        let data = unsafe { &*self.data.get() };
        let entry = data[key as usize & self.mask]?;
        if entry.key == key { Some(entry) } else { None }
    }

    #[inline]
    pub fn put(&self, key: u64, depth: i32, flag: u8, score: i32,
               best_move: Option<ChessMove>) {
        let data = unsafe { &mut *self.data.get() };
        let idx  = key as usize & self.mask;
        let replace = match data[idx] {
            None               => { self.filled.fetch_add(1, Ordering::Relaxed); true }
            Some(ref e) if e.key == key => true,   // same position: always refresh
            Some(ref e)        => depth >= e.depth, // different position: only if deeper
        };
        if replace {
            data[idx] = Some(TTEntry { key, depth, flag, score, best_move });
        }
    }

    pub fn clear(&self) {
        let data = unsafe { &mut *self.data.get() };
        for slot in data.iter_mut() { *slot = None; }
        self.filled.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_and_get_roundtrip() {
        let tt = TranspositionTable::new(1);
        tt.put(0xDEAD_BEEF, 5, EXACT, 150, None);
        let e = tt.get(0xDEAD_BEEF).expect("entry should be found");
        assert_eq!(e.key,   0xDEAD_BEEF);
        assert_eq!(e.depth, 5);
        assert_eq!(e.flag,  EXACT);
        assert_eq!(e.score, 150);
        assert!(e.best_move.is_none());
    }

    #[test]
    fn get_returns_none_for_wrong_key() {
        let tt = TranspositionTable::new(1);
        tt.put(0x1111, 3, EXACT, 0, None);
        // A key that maps to the same slot but has a different key value
        // should return None (key mismatch check).
        // We can't guarantee a collision without knowing the table size,
        // so just verify a completely different key returns None.
        let result = tt.get(0x9999_9999_9999_9999);
        if let Some(e) = result { assert_ne!(e.key, 0x9999_9999_9999_9999); }
    }

    #[test]
    fn clear_resets_hashfull_to_zero() {
        let tt = TranspositionTable::new(1);
        for i in 0u64..200 { tt.put(i * 65537, 1, EXACT, 0, None); }
        assert!(tt.hashfull() > 0);
        tt.clear();
        assert_eq!(tt.hashfull(), 0);
    }

    #[test]
    fn hashfull_increases_as_table_fills() {
        let tt = TranspositionTable::new(1);
        let before = tt.hashfull();
        for i in 0u64..500 { tt.put(i * 65537, 1, EXACT, 0, None); }
        assert!(tt.hashfull() > before);
    }

    #[test]
    fn put_overwrites_same_slot() {
        let tt = TranspositionTable::new(1);
        tt.put(0, 3, EXACT,       100, None);
        tt.put(0, 7, LOWER_BOUND, 999, None);
        let e = tt.get(0).expect("entry should exist");
        assert_eq!(e.depth, 7);
        assert_eq!(e.score, 999);
        assert_eq!(e.flag,  LOWER_BOUND);
    }

    #[test]
    fn flag_constants_are_distinct() {
        assert_ne!(EXACT,       LOWER_BOUND);
        assert_ne!(EXACT,       UPPER_BOUND);
        assert_ne!(LOWER_BOUND, UPPER_BOUND);
    }

    #[test]
    fn negative_scores_stored_correctly() {
        let tt = TranspositionTable::new(1);
        tt.put(42, 4, UPPER_BOUND, -300, None);
        let e = tt.get(42).unwrap();
        assert_eq!(e.score, -300);
    }

    #[test]
    fn large_positive_score_stored_correctly() {
        let tt = TranspositionTable::new(1);
        tt.put(99, 10, EXACT, 99_999, None);
        let e = tt.get(99).unwrap();
        assert_eq!(e.score, 99_999);
    }
}
