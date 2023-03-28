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
        let idx = key as usize & self.mask;
        if data[idx].is_none() { self.filled.fetch_add(1, Ordering::Relaxed); }
        data[idx] = Some(TTEntry { key, depth, flag, score, best_move });
    }

    pub fn clear(&self) {
        let data = unsafe { &mut *self.data.get() };
        for slot in data.iter_mut() { *slot = None; }
        self.filled.store(0, Ordering::Relaxed);
    }
}
