//! A rule-based computer opponent.
//!
//! Plays instantly and needs no model download: it values spots by their
//! production (pips), spreads over resources, builds cities first, then
//! settlements, development cards and roads toward the next good spot, uses
//! the bank only when a trade completes a build, and aims the robber at the
//! leader. Offered trades are judged by the same heuristic other players use.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;

use crate::game::actions::{Action, PlayerId, TradeOffer, TradeResponse};
use crate::game::board::{
    self, adjacent_vertices, canonical_vertex, edge_vertices, pip_count, vertex_neighbors,
    EdgeCoord, HexCoord, PortType, Resource, VertexCoord,
};
use crate::game::rules;
use crate::game::state::{Building, GameState, PlayerState};
use crate::player::{Player, PlayerChoice};

const ROAD: [(Resource, u32); 2] = [(Resource::Brick, 1), (Resource::Wood, 1)];
const SETTLEMENT: [(Resource, u32); 4] = [
    (Resource::Brick, 1),
    (Resource::Wood, 1),
    (Resource::Sheep, 1),
    (Resource::Wheat, 1),
];
const CITY: [(Resource, u32); 2] = [(Resource::Wheat, 2), (Resource::Ore, 3)];
const DEV_CARD: [(Resource, u32); 3] = [
    (Resource::Sheep, 1),
    (Resource::Wheat, 1),
    (Resource::Ore, 1),
];

pub struct ComputerPlayer {
    name: String,
    /// A short pause before each decision, so people can follow the moves.
    pace: std::time::Duration,
}

impl ComputerPlayer {
    /// A computer player that pauses briefly before each move.
    pub fn new(name: String) -> Self {
        Self::with_pace(name, std::time::Duration::from_millis(300))
    }

    pub fn with_pace(name: String, pace: std::time::Duration) -> Self {
        Self { name, pace }
    }

    async fn think(&self) {
        if !self.pace.is_zero() {
            tokio::time::sleep(self.pace).await;
        }
    }
}

/// Pips per resource this player already produces.
fn production(state: &GameState, me: PlayerId) -> HashMap<Resource, u32> {
    let mut prod = HashMap::new();
    for (v, b) in &state.buildings {
        let (owner, mult) = match b {
            Building::Settlement(p) => (*p, 1),
            Building::City(p) => (*p, 2),
        };
        if owner != me {
            continue;
        }
        for h in vertex_neighbors(*v) {
            if let Some(hex) = state.board.get_hex(h) {
                if let (Some(r), Some(n)) = (hex.terrain.resource(), hex.number_token) {
                    *prod.entry(r).or_insert(0) += pip_count(n) as u32 * mult;
                }
            }
        }
    }
    prod
}

/// How good a spot is for this player: production, with a bonus for
/// resources not yet produced, and a little for a harbour.
fn vertex_value(state: &GameState, me: PlayerId, v: VertexCoord) -> f32 {
    let prod = production(state, me);
    let mut value = 0.0;
    let mut seen = HashSet::new();
    for h in vertex_neighbors(v) {
        if let Some(hex) = state.board.get_hex(h) {
            if let (Some(r), Some(n)) = (hex.terrain.resource(), hex.number_token) {
                let pips = pip_count(n) as f32;
                let robbed = if state.robber_hex == h { 0.5 } else { 1.0 };
                value += pips * robbed;
                if !prod.contains_key(&r) && seen.insert(r) {
                    value += 2.0;
                }
                // Early on, brick and wood build everything else.
                if matches!(r, Resource::Brick | Resource::Wood) && state.turn_number < 20 {
                    value += pips * 0.2;
                }
            }
        }
    }
    if let Some(port) = state.board.port_at_vertex(v) {
        value += match port.port_type {
            PortType::Generic => 1.0,
            PortType::Specific(r) => {
                if prod.get(&r).copied().unwrap_or(0) >= 5 {
                    2.5
                } else {
                    0.5
                }
            }
        };
    }
    value
}

fn is_free_spot(state: &GameState, v: VertexCoord) -> bool {
    let c = canonical_vertex(v);
    if !board::vertex_neighbors(c)
        .iter()
        .any(|h| state.board.has_hex(*h))
    {
        return false;
    }
    if state.buildings.contains_key(&c) {
        return false;
    }
    adjacent_vertices(c)
        .iter()
        .all(|n| !state.buildings.contains_key(&canonical_vertex(*n)))
}

/// Value of building a road along this edge: the best free spot it reaches
/// directly, or one step further on at half weight.
fn edge_value(state: &GameState, me: PlayerId, e: EdgeCoord) -> f32 {
    let (a, b) = edge_vertices(e);
    let mut best: f32 = 0.0;
    for v in [a, b] {
        let c = canonical_vertex(v);
        if is_free_spot(state, c) {
            best = best.max(vertex_value(state, me, c));
        }
        for n in adjacent_vertices(c) {
            let n = canonical_vertex(n);
            if is_free_spot(state, n) {
                best = best.max(vertex_value(state, me, n) * 0.5);
            }
        }
    }
    best
}

fn can_afford(ps: &PlayerState, cost: &[(Resource, u32)]) -> bool {
    ps.has_resources(cost)
}

/// What this player is saving for, most wanted first.
fn targets(state: &GameState, me: PlayerId) -> Vec<&'static [(Resource, u32)]> {
    let ps = &state.players[me];
    let mut t: Vec<&'static [(Resource, u32)]> = Vec::new();
    if ps.cities_remaining > 0 && !rules::legal_city_vertices(state, me).is_empty() {
        t.push(&CITY);
    }
    if ps.settlements_remaining > 0 && !rules::legal_settlement_vertices(state, me).is_empty() {
        t.push(&SETTLEMENT);
    }
    if ps.settlements_remaining > 0 && ps.roads_remaining > 0 {
        t.push(&ROAD);
    }
    t.push(&DEV_CARD);
    t
}

/// How many of each resource are still missing for the most wanted target.
fn missing_for_target(state: &GameState, me: PlayerId) -> Vec<(Resource, u32)> {
    let ps = &state.players[me];
    for cost in targets(state, me) {
        let missing: Vec<(Resource, u32)> = cost
            .iter()
            .filter_map(|&(r, n)| {
                let have = ps.resource_count(r);
                (have < n).then(|| (r, n - have))
            })
            .collect();
        if !missing.is_empty() {
            return missing;
        }
    }
    Vec::new()
}

/// Whether a bank trade turns an unaffordable target into an affordable one
/// without spending what that target needs.
fn bank_trade_helps(state: &GameState, me: PlayerId, give: Resource, get: Resource) -> bool {
    let ps = &state.players[me];
    let spend = trade_ratio(state, me, give);
    if give == get || ps.resource_count(give) < spend {
        return false;
    }
    for cost in targets(state, me) {
        if can_afford(ps, cost) {
            return false;
        }
        let mut after = ps.clone();
        after.remove_resource(give, spend);
        after.add_resource(get, 1);
        if can_afford(&after, cost) {
            return true;
        }
    }
    false
}

fn trade_ratio(state: &GameState, me: PlayerId, give: Resource) -> u32 {
    let mut ratio = 4;
    for port in &state.board.ports {
        let owned = [port.vertices.0, port.vertices.1].iter().any(|v| {
            matches!(state.buildings.get(&canonical_vertex(*v)),
                Some(Building::Settlement(p)) | Some(Building::City(p)) if *p == me)
        });
        if !owned {
            continue;
        }
        match port.port_type {
            PortType::Generic => ratio = ratio.min(3),
            PortType::Specific(r) if r == give => ratio = ratio.min(2),
            PortType::Specific(_) => {}
        }
    }
    ratio
}

fn leader_weight(state: &GameState, p: PlayerId) -> f32 {
    1.0 + state.victory_points(p) as f32 * 0.5
}

/// Pips of opponents' buildings on a hex, weighted toward the leader.
fn hex_harm(state: &GameState, me: PlayerId, h: HexCoord) -> f32 {
    let Some(hex) = state.board.get_hex(h) else {
        return -1.0;
    };
    let pips = hex.number_token.map(pip_count).unwrap_or(0) as f32;
    let mut harm = 0.0;
    for v in board::hex_vertices(h) {
        match state.buildings.get(&canonical_vertex(v)) {
            Some(Building::Settlement(p)) if *p == me => return -10.0,
            Some(Building::City(p)) if *p == me => return -10.0,
            Some(Building::Settlement(p)) => harm += pips * leader_weight(state, *p),
            Some(Building::City(p)) => harm += 2.0 * pips * leader_weight(state, *p),
            None => {}
        }
    }
    harm
}

fn robber_on_me(state: &GameState, me: PlayerId) -> bool {
    board::hex_vertices(state.robber_hex).iter().any(|v| {
        matches!(state.buildings.get(&canonical_vertex(*v)),
            Some(Building::Settlement(p)) | Some(Building::City(p)) if *p == me)
    })
}

fn best_index<T>(items: &[T], score: impl Fn(&T) -> f32) -> usize {
    let mut best = 0;
    let mut best_score = f32::MIN;
    for (i, item) in items.iter().enumerate() {
        let s = score(item);
        if s > best_score {
            best = i;
            best_score = s;
        }
    }
    best
}

#[async_trait]
impl Player for ComputerPlayer {
    fn name(&self) -> &str {
        &self.name
    }

    async fn choose_action(
        &self,
        state: &GameState,
        me: PlayerId,
        choices: &[PlayerChoice],
    ) -> (usize, String) {
        self.think().await;
        let ps = &state.players[me];
        let useful_bank_trade = rules::legal_actions(state).iter().any(|a| {
            matches!(a, Action::BankTrade { give, get } if bank_trade_helps(state, me, *give, *get))
        });
        let knights_ahead = state
            .largest_army_size
            .max(2)
            .saturating_sub(ps.knights_played);
        let score = |c: &PlayerChoice| -> f32 {
            match c {
                PlayerChoice::BuildCityIntent | PlayerChoice::GameAction(Action::BuildCity(_)) => {
                    100.0
                }
                PlayerChoice::BuildSettlementIntent
                | PlayerChoice::GameAction(Action::BuildSettlement(_)) => 90.0,
                PlayerChoice::PlayKnight => {
                    if robber_on_me(state, me) || knights_ahead <= 1 {
                        70.0
                    } else {
                        -1.0
                    }
                }
                PlayerChoice::RollDice => 50.0,
                PlayerChoice::PlayRoadBuilding => 60.0,
                PlayerChoice::PlayYearOfPlenty => 58.0,
                PlayerChoice::PlayMonopoly => 45.0,
                PlayerChoice::GameAction(Action::BankTrade { give, get }) => {
                    if bank_trade_helps(state, me, *give, *get) {
                        80.0
                    } else {
                        -1.0
                    }
                }
                PlayerChoice::BankTradeIntent => {
                    if useful_bank_trade {
                        80.0
                    } else {
                        -1.0
                    }
                }
                PlayerChoice::BuildRoadIntent | PlayerChoice::GameAction(Action::BuildRoad(_)) => {
                    let spots = rules::legal_settlement_vertices(state, me).len();
                    if ps.settlements_remaining == 0 || ps.roads_remaining == 0 {
                        -1.0
                    } else if spots == 0 {
                        40.0
                    } else if can_afford(ps, &SETTLEMENT) {
                        -1.0
                    } else {
                        5.0
                    }
                }
                PlayerChoice::GameAction(Action::BuyDevCard) => 35.0,
                PlayerChoice::GameAction(Action::EndTurn) => 0.0,
                _ => -5.0,
            }
        };
        let idx = best_index(choices, score);
        (idx, format!("Computer: {}", choices[idx]))
    }

    async fn choose_settlement(
        &self,
        state: &GameState,
        me: PlayerId,
        legal: &[VertexCoord],
        _round: u8,
        _names: &[String],
    ) -> (usize, String) {
        self.think().await;
        let idx = best_index(legal, |v| vertex_value(state, me, *v));
        (idx, "Computer: best production".into())
    }

    async fn choose_road(
        &self,
        state: &GameState,
        me: PlayerId,
        legal: &[EdgeCoord],
        _names: &[String],
    ) -> (usize, String) {
        self.think().await;
        let idx = best_index(legal, |e| edge_value(state, me, *e));
        (idx, "Computer: toward the next spot".into())
    }

    async fn choose_robber_hex(
        &self,
        state: &GameState,
        me: PlayerId,
        legal: &[HexCoord],
    ) -> (usize, String) {
        self.think().await;
        let idx = best_index(legal, |h| hex_harm(state, me, *h));
        (idx, "Computer: on the leader".into())
    }

    async fn choose_steal_target(
        &self,
        state: &GameState,
        _me: PlayerId,
        targets: &[PlayerId],
        _names: &[String],
    ) -> (usize, String) {
        let idx = best_index(targets, |p| {
            state.victory_points(*p) as f32 * 10.0 + state.players[*p].total_resources() as f32
        });
        (idx, "Computer: from the leader".into())
    }

    async fn choose_discard(
        &self,
        state: &GameState,
        me: PlayerId,
        count: usize,
    ) -> (Vec<Resource>, String) {
        let mut hand = state.players[me].clone();
        let keep = missing_for_target(state, me);
        let mut out = Vec::new();
        for _ in 0..count {
            // Throw away the most plentiful card, sparing what the target needs.
            let r = *Resource::all()
                .iter()
                .filter(|r| hand.resource_count(**r) > 0)
                .max_by_key(|r| {
                    let wanted = keep.iter().any(|(k, _)| k == *r) as u32;
                    hand.resource_count(**r) * 2 - wanted
                })
                .expect("discard count never exceeds the hand");
            hand.remove_resource(r, 1);
            out.push(r);
        }
        (out, "Computer: kept what it is building toward".into())
    }

    async fn choose_resource(
        &self,
        state: &GameState,
        me: PlayerId,
        context: &str,
    ) -> (Resource, String) {
        if context.contains("MONOPOLY") {
            let r = *Resource::all()
                .iter()
                .max_by_key(|r| {
                    (0..state.num_players)
                        .filter(|p| *p != me)
                        .map(|p| state.players[p].resource_count(**r))
                        .sum::<u32>()
                })
                .unwrap_or(&Resource::Ore);
            return (r, "Computer: the most held".into());
        }
        let missing = missing_for_target(state, me);
        let r = missing.first().map(|(r, _)| *r).unwrap_or_else(|| {
            *Resource::all()
                .iter()
                .min_by_key(|r| state.players[me].resource_count(**r))
                .unwrap_or(&Resource::Ore)
        });
        (r, "Computer: what it needs next".into())
    }

    async fn propose_trade(
        &self,
        _state: &GameState,
        _me: PlayerId,
    ) -> Option<(TradeOffer, String)> {
        None
    }

    async fn respond_to_trade(
        &self,
        state: &GameState,
        me: PlayerId,
        offer: &TradeOffer,
        _names: &[String],
    ) -> (TradeResponse, String) {
        let value = crate::trading::negotiation::trade_value_heuristic(offer, &state.players[me]);
        let near_win = state.victory_points(offer.from) >= 8;
        if value > 0.5 && !near_win {
            (TradeResponse::Accept, "Computer: a fair trade".into())
        } else {
            (
                TradeResponse::Reject {
                    reason: if near_win {
                        "Not helping you win.".into()
                    } else {
                        "Not worth it for me.".into()
                    },
                },
                "Computer: declined".into(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::board::Board;
    use crate::game::orchestrator::GameOrchestrator;

    #[tokio::test]
    async fn computer_games_finish_with_a_winner() {
        let mut finished = 0;
        for _ in 0..5 {
            let state = GameState::new(Board::default_board(), 4);
            let players: Vec<Box<dyn Player>> = (0..4)
                .map(|i| {
                    Box::new(ComputerPlayer::with_pace(
                        format!("C{i}"),
                        Default::default(),
                    )) as Box<dyn Player>
                })
                .collect();
            let mut orch = GameOrchestrator::new(state, players);
            orch.max_turns = 400;
            if let Ok(winner) = orch.run().await {
                assert!(orch.state.victory_points(winner) >= 10);
                finished += 1;
            }
        }
        assert!(finished >= 4, "only {finished} of 5 games finished");
    }

    #[tokio::test]
    async fn computer_beats_random_players() {
        use crate::player::random::RandomPlayer;
        let mut wins = 0;
        for _ in 0..6 {
            let state = GameState::new(Board::default_board(), 3);
            let players: Vec<Box<dyn Player>> = vec![
                Box::new(ComputerPlayer::with_pace("C".into(), Default::default())),
                Box::new(RandomPlayer::new("R1".into())),
                Box::new(RandomPlayer::new("R2".into())),
            ];
            let mut orch = GameOrchestrator::new(state, players);
            orch.max_turns = 400;
            if let Ok(0) = orch.run().await {
                wins += 1;
            }
        }
        assert!(wins >= 5, "computer won only {wins} of 6");
    }
}
