//! The analytical event log: struct-of-arrays with interned identifiers
//! (design doc `03 §4`). Six primitive columns, 1:1 with the Parquet
//! archive schema — a million-event funnel fold is a sequential scan over
//! 2-byte stage ids. This is the *accelerator* representation; the
//! `LogRecord` stream stays the truth (doc `04`).

use std::collections::HashMap;

use crate::event::Event;
use crate::slug::Stage;
use crate::token::Token;

/// Dense stage identifier (interned).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StageId(pub u16);

/// Dense token identifier (interned, per-store).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TokenId(pub u32);

/// Append-only intern tables. Extension is committer-owned in stores
/// (G-1); in-process users own their instance, so `&mut` is the guard.
#[derive(Debug, Default)]
pub struct InternTables {
    stages: Vec<Stage>,
    stage_index: HashMap<Stage, StageId>,
    tokens: Vec<Token>,
    token_index: HashMap<Token, TokenId>,
}

impl InternTables {
    /// Intern a stage (idempotent).
    pub fn stage(&mut self, stage: &Stage) -> StageId {
        if let Some(id) = self.stage_index.get(stage) {
            return *id;
        }
        #[allow(clippy::cast_possible_truncation)] // stage vocabulary is small by design
        let id = StageId(self.stages.len() as u16);
        self.stages.push(stage.clone());
        self.stage_index.insert(stage.clone(), id);
        id
    }

    /// Intern a token (idempotent).
    pub fn token(&mut self, token: Token) -> TokenId {
        if let Some(id) = self.token_index.get(&token) {
            return *id;
        }
        #[allow(clippy::cast_possible_truncation)] // u32 tokens per store is the design bound
        let id = TokenId(self.tokens.len() as u32);
        self.tokens.push(token);
        self.token_index.insert(token, id);
        id
    }

    /// Resolve a stage id back to its slug.
    #[must_use]
    pub fn stage_name(&self, id: StageId) -> Option<&Stage> {
        self.stages.get(id.0 as usize)
    }

    /// Look a token's dense id up without interning.
    #[must_use]
    pub fn token_id(&self, token: Token) -> Option<TokenId> {
        self.token_index.get(&token).copied()
    }
}

/// Sentinel for "no variant recorded" in the packed column.
const NO_VARIANT: u8 = u8::MAX;

/// The seven-column `SoA` log. Fixed-width rows — a consequence of I-1
/// (events carry no payload), and the load-bearing fact behind lock-free
/// tail reads in stores (doc `15 §2`). The `regions` column packs the
/// contract touch bitmask; `0` doubles as "none" because an empty mask
/// carries no information (unlike `variant`, where index 0 is real).
#[derive(Debug, Default)]
pub struct EventLog {
    token_ids: Vec<TokenId>,
    stage_ids: Vec<StageId>,
    actors: Vec<u8>,
    variants: Vec<u8>,
    regions: Vec<u8>,
    at_ms: Vec<u64>,
    seqs: Vec<u32>,
}

impl EventLog {
    /// Append one event, interning through `tables`.
    pub fn push(&mut self, event: &Event, tables: &mut InternTables) {
        self.token_ids.push(tables.token(event.token));
        self.stage_ids.push(tables.stage(&event.stage));
        self.actors.push(event.actor.code());
        self.variants.push(event.variant.unwrap_or(NO_VARIANT));
        self.regions.push(event.regions.unwrap_or(0));
        self.at_ms.push(event.at.as_unix_ms());
        self.seqs.push(event.seq.0);
    }

    /// Number of rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.token_ids.len()
    }

    /// True when empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.token_ids.is_empty()
    }

    /// Stage counts for one token — the hot funnel fold: one sequential
    /// pass over two narrow columns, counts into a dense array.
    #[must_use]
    pub fn stage_counts(&self, token: TokenId, tables: &InternTables) -> Vec<(Stage, u64)> {
        let mut counts = vec![0u64; tables.stages.len()];
        for (t, s) in self.token_ids.iter().zip(&self.stage_ids) {
            if *t == token {
                counts[s.0 as usize] += 1;
            }
        }
        counts
            .into_iter()
            .enumerate()
            .filter(|(_, c)| *c > 0)
            .filter_map(|(i, c)| {
                #[allow(clippy::cast_possible_truncation)] // i < stages.len() <= u16::MAX
                tables.stage_name(StageId(i as u16)).map(|s| (s.clone(), c))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{ActorClass, Seq};
    use crate::{ResolverContext, Timestamp};

    fn ev(token: Token, stage: Stage, seq: u32) -> Event {
        Event {
            token,
            stage,
            actor: ActorClass::from_context(&ResolverContext::human()),
            at: Timestamp::from_unix_ms(u64::from(seq)),
            seq: Seq(seq),
            variant: if seq % 2 == 0 { Some(1) } else { None },
            regions: None,
            entry: None,
        }
    }

    #[test]
    fn interning_is_idempotent_and_dense() {
        let mut tables = InternTables::default();
        let a = tables.stage(&Stage::resolve());
        let b = tables.stage(&Stage::resolve());
        assert_eq!(a, b);
        let c = tables.stage(&Stage::run());
        assert_eq!(c.0, a.0 + 1, "dense, append-only ids");
    }

    #[test]
    fn soa_counts_agree_with_naive_counting() {
        let mut tables = InternTables::default();
        let mut log = EventLog::default();
        let t1 = Token::parse("one").unwrap();
        let t2 = Token::parse("two").unwrap();
        for i in 0..10 {
            log.push(&ev(t1, Stage::resolve(), i), &mut tables);
        }
        for i in 0..3 {
            log.push(&ev(t2, Stage::run(), i), &mut tables);
            log.push(&ev(t1, Stage::run(), 100 + i), &mut tables);
        }
        let id1 = tables.token_id(t1).unwrap();
        let counts = log.stage_counts(id1, &tables);
        assert!(counts.contains(&(Stage::resolve(), 10)));
        assert!(counts.contains(&(Stage::run(), 3)));
        assert_eq!(log.len(), 16);
    }
}
