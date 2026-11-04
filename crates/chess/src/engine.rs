//! Regras do xadrez e um adversário simples (busca alfa-beta).
//!
//! Tabuleiro: 64 casas, índice = fileira * 8 + coluna (a1 = 0, h8 = 63).
//! Peças positivas são brancas, negativas pretas.

pub const P: i8 = 1;
pub const N: i8 = 2;
pub const B: i8 = 3;
pub const R: i8 = 4;
pub const Q: i8 = 5;
pub const K: i8 = 6;

const WK: u8 = 1;
const WQ: u8 = 2;
const BK: u8 = 4;
const BQ: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mv {
    pub from: u8,
    pub to: u8,
    /// Peça da promoção (tipo, positivo) ou 0.
    pub promo: i8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pos {
    pub b: [i8; 64],
    /// 1 = brancas jogam, -1 = pretas.
    pub side: i8,
    pub castle: u8,
    pub ep: Option<u8>,
    pub half: u32,
}

const KNIGHT: [(i8, i8); 8] = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)];
const KING: [(i8, i8); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
const ROOK_DIRS: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const BISHOP_DIRS: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

fn at(r: i8, f: i8) -> Option<u8> {
    ((0..8).contains(&r) && (0..8).contains(&f)).then(|| (r * 8 + f) as u8)
}

impl Pos {
    pub fn start() -> Self {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap()
    }

    pub fn from_fen(fen: &str) -> Option<Self> {
        let f: Vec<&str> = fen.split_whitespace().collect();
        let mut b = [0i8; 64];
        let (mut r, mut c) = (7i8, 0i8);
        for ch in f.first()?.chars() {
            match ch {
                '/' => {
                    r -= 1;
                    c = 0;
                }
                '1'..='8' => c += ch as i8 - b'0' as i8,
                _ => {
                    let t = match ch.to_ascii_lowercase() {
                        'p' => P,
                        'n' => N,
                        'b' => B,
                        'r' => R,
                        'q' => Q,
                        'k' => K,
                        _ => return None,
                    };
                    b[at(r, c)? as usize] = if ch.is_ascii_uppercase() { t } else { -t };
                    c += 1;
                }
            }
        }
        let side = if f.get(1) == Some(&"b") { -1 } else { 1 };
        let mut castle = 0;
        for ch in f.get(2).unwrap_or(&"-").chars() {
            castle |= match ch {
                'K' => WK,
                'Q' => WQ,
                'k' => BK,
                'q' => BQ,
                _ => 0,
            };
        }
        let ep = f.get(3).filter(|s| s.len() == 2).map(|s| {
            let s = s.as_bytes();
            (s[1] - b'1') * 8 + (s[0] - b'a')
        });
        Some(Self { b, side, castle, ep, half: f.get(4).and_then(|v| v.parse().ok()).unwrap_or(0) })
    }

    pub fn king(&self, side: i8) -> Option<u8> {
        self.b.iter().position(|p| *p == K * side).map(|i| i as u8)
    }

    /// A casa `sq` é atacada por peças de `by`?
    pub fn attacked(&self, sq: u8, by: i8) -> bool {
        let (r, f) = ((sq / 8) as i8, (sq % 8) as i8);
        let is = |s: Option<u8>, p: i8| s.is_some_and(|s| self.b[s as usize] == p * by);
        for df in [-1, 1] {
            if is(at(r - by, f + df), P) {
                return true;
            }
        }
        if KNIGHT.iter().any(|(dr, df)| is(at(r + dr, f + df), N)) || KING.iter().any(|(dr, df)| is(at(r + dr, f + df), K)) {
            return true;
        }
        for (dirs, slider) in [(ROOK_DIRS, R), (BISHOP_DIRS, B)] {
            for (dr, df) in dirs {
                let (mut rr, mut ff) = (r + dr, f + df);
                while let Some(s) = at(rr, ff) {
                    let p = self.b[s as usize];
                    if p != 0 {
                        if p == slider * by || p == Q * by {
                            return true;
                        }
                        break;
                    }
                    rr += dr;
                    ff += df;
                }
            }
        }
        false
    }

    pub fn in_check(&self) -> bool {
        self.king(self.side).is_some_and(|k| self.attacked(k, -self.side))
    }

    fn pseudo(&self) -> Vec<Mv> {
        let mut out = Vec::with_capacity(48);
        let s = self.side;
        let push = |out: &mut Vec<Mv>, from: u8, to: u8, pawn: bool| {
            let last = if s == 1 { 7 } else { 0 };
            if pawn && to / 8 == last {
                for p in [Q, R, B, N] {
                    out.push(Mv { from, to, promo: p });
                }
            } else {
                out.push(Mv { from, to, promo: 0 });
            }
        };
        for from in 0..64u8 {
            let p = self.b[from as usize];
            if p == 0 || p.signum() != s {
                continue;
            }
            let (r, f) = ((from / 8) as i8, (from % 8) as i8);
            match p.abs() {
                P => {
                    if let Some(one) = at(r + s, f) {
                        if self.b[one as usize] == 0 {
                            push(&mut out, from, one, true);
                            let start = if s == 1 { 1 } else { 6 };
                            if r == start {
                                if let Some(two) = at(r + 2 * s, f) {
                                    if self.b[two as usize] == 0 {
                                        out.push(Mv { from, to: two, promo: 0 });
                                    }
                                }
                            }
                        }
                    }
                    for df in [-1, 1] {
                        if let Some(t) = at(r + s, f + df) {
                            let q = self.b[t as usize];
                            if (q != 0 && q.signum() == -s) || self.ep == Some(t) {
                                push(&mut out, from, t, true);
                            }
                        }
                    }
                }
                N | K => {
                    let offs = if p.abs() == N { &KNIGHT } else { &KING };
                    for (dr, df) in offs {
                        if let Some(t) = at(r + dr, f + df) {
                            if self.b[t as usize].signum() != s {
                                out.push(Mv { from, to: t, promo: 0 });
                            }
                        }
                    }
                }
                t => {
                    let dirs: Vec<(i8, i8)> = match t {
                        B => BISHOP_DIRS.to_vec(),
                        R => ROOK_DIRS.to_vec(),
                        _ => ROOK_DIRS.iter().chain(BISHOP_DIRS.iter()).copied().collect(),
                    };
                    for (dr, df) in dirs {
                        let (mut rr, mut ff) = (r + dr, f + df);
                        while let Some(t) = at(rr, ff) {
                            let q = self.b[t as usize];
                            if q.signum() == s {
                                break;
                            }
                            out.push(Mv { from, to: t, promo: 0 });
                            if q != 0 {
                                break;
                            }
                            rr += dr;
                            ff += df;
                        }
                    }
                }
            }
        }
        // Roque
        let (rank, kbit, qbit) = if s == 1 { (0u8, WK, WQ) } else { (56u8, BK, BQ) };
        let e = rank + 4;
        if self.b[e as usize] == K * s && !self.attacked(e, -s) {
            if self.castle & kbit != 0 && self.b[(rank + 5) as usize] == 0 && self.b[(rank + 6) as usize] == 0 && self.b[(rank + 7) as usize] == R * s && !self.attacked(rank + 5, -s) && !self.attacked(rank + 6, -s) {
                out.push(Mv { from: e, to: rank + 6, promo: 0 });
            }
            if self.castle & qbit != 0
                && self.b[(rank + 3) as usize] == 0
                && self.b[(rank + 2) as usize] == 0
                && self.b[(rank + 1) as usize] == 0
                && self.b[rank as usize] == R * s
                && !self.attacked(rank + 3, -s)
                && !self.attacked(rank + 2, -s)
            {
                out.push(Mv { from: e, to: rank + 2, promo: 0 });
            }
        }
        out
    }

    pub fn make(&self, m: Mv) -> Pos {
        let mut n = self.clone();
        let p = n.b[m.from as usize];
        let captured = n.b[m.to as usize];
        let pawn = p.abs() == P;
        n.b[m.to as usize] = if m.promo != 0 { m.promo * self.side } else { p };
        n.b[m.from as usize] = 0;
        if pawn && Some(m.to) == self.ep && m.from % 8 != m.to % 8 {
            n.b[(m.to as i16 - 8 * self.side as i16) as usize] = 0;
        }
        if p.abs() == K && (m.to as i16 - m.from as i16).abs() == 2 {
            let (rf, rt) = match m.to {
                6 => (7, 5),
                2 => (0, 3),
                62 => (63, 61),
                _ => (56, 59),
            };
            n.b[rt] = n.b[rf];
            n.b[rf] = 0;
        }
        n.ep = (pawn && (m.to as i16 - m.from as i16).abs() == 16).then(|| (m.from + m.to) / 2);
        if p.abs() == K {
            n.castle &= if self.side == 1 { !(WK | WQ) } else { !(BK | BQ) };
        }
        for (sq, bit) in [(0u8, WQ), (7, WK), (56, BQ), (63, BK)] {
            if m.from == sq || m.to == sq {
                n.castle &= !bit;
            }
        }
        n.half = if pawn || captured != 0 { 0 } else { self.half + 1 };
        n.side = -self.side;
        n
    }

    pub fn legal(&self) -> Vec<Mv> {
        self.pseudo()
            .into_iter()
            .filter(|m| {
                let n = self.make(*m);
                n.king(self.side).is_some_and(|k| !n.attacked(k, -self.side))
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Adversário
// ---------------------------------------------------------------------------

const VALUE: [i32; 7] = [0, 100, 320, 330, 500, 900, 0];
const MATE: i32 = 100_000;

/// Avaliação do ponto de vista de quem joga.
fn eval(p: &Pos) -> i32 {
    let mut score = 0;
    for (i, &pc) in p.b.iter().enumerate() {
        if pc == 0 {
            continue;
        }
        let (r, f) = ((i / 8) as i32, (i % 8) as i32);
        let rel_r = if pc > 0 { r } else { 7 - r };
        let center = 6 - ((2 * f - 7).abs() + (2 * r - 7).abs()) / 2;
        let bonus = match pc.abs() {
            P => rel_r * 8 + if (2..=5).contains(&f) { rel_r * 2 } else { 0 },
            N => center * 8,
            B => center * 5,
            R => if rel_r == 6 { 20 } else { 0 },
            Q => center * 2,
            K => if rel_r == 0 && (f <= 2 || f >= 6) { 25 } else { -center * 3 },
            _ => 0,
        };
        let v = VALUE[pc.unsigned_abs() as usize] + bonus;
        score += if pc > 0 { v } else { -v };
    }
    score * p.side as i32
}

fn order(p: &Pos, moves: &mut [Mv]) {
    // Capturas valiosas com peças baratas primeiro.
    moves.sort_by_key(|m| {
        let victim = VALUE[p.b[m.to as usize].unsigned_abs() as usize];
        let attacker = VALUE[p.b[m.from as usize].unsigned_abs() as usize];
        -(victim * 10 - attacker / 10 + if m.promo != 0 { 800 } else { 0 })
    });
}

fn quiesce(p: &Pos, mut alpha: i32, beta: i32, depth: u32) -> i32 {
    let stand = eval(p);
    if stand >= beta || depth == 0 {
        return stand;
    }
    alpha = alpha.max(stand);
    let mut caps: Vec<Mv> = p.legal().into_iter().filter(|m| p.b[m.to as usize] != 0 || m.promo == Q).collect();
    order(p, &mut caps);
    for m in caps {
        let s = -quiesce(&p.make(m), -beta, -alpha, depth - 1);
        if s >= beta {
            return s;
        }
        alpha = alpha.max(s);
    }
    alpha
}

fn negamax(p: &Pos, depth: u32, mut alpha: i32, beta: i32, ply: i32) -> i32 {
    let mut moves = p.legal();
    if moves.is_empty() {
        return if p.in_check() { -MATE + ply } else { 0 };
    }
    if p.half >= 100 {
        return 0;
    }
    if depth == 0 {
        return quiesce(p, alpha, beta, 4);
    }
    order(p, &mut moves);
    let mut best = -MATE * 2;
    for m in moves {
        let s = -negamax(&p.make(m), depth - 1, -beta, -alpha, ply + 1);
        best = best.max(s);
        alpha = alpha.max(s);
        if alpha >= beta {
            break;
        }
    }
    best
}

/// Melhor jogada na profundidade pedida. A semente embaralha a ordem das
/// jogadas, para o computador não jogar sempre igual entre opções empatadas.
pub fn best_move(p: &Pos, depth: u32, seed: u64) -> Option<Mv> {
    let mut moves = p.legal();
    let mut x = seed | 1;
    for i in (1..moves.len()).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        moves.swap(i, (x % (i as u64 + 1)) as usize);
    }
    order(p, &mut moves);
    let mut best: Option<(i32, Mv)> = None;
    let mut alpha = -MATE * 2;
    for m in moves {
        let s = -negamax(&p.make(m), depth.saturating_sub(1), -MATE * 2, -alpha, 1);
        if best.is_none_or(|(b, _)| s > b) {
            best = Some((s, m));
            alpha = alpha.max(s);
        }
    }
    best.map(|(_, m)| m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perft(p: &Pos, d: u32) -> u64 {
        if d == 0 {
            return 1;
        }
        p.legal().into_iter().map(|m| perft(&p.make(m), d - 1)).sum()
    }

    #[test]
    fn perft_inicial() {
        let p = Pos::start();
        assert_eq!(perft(&p, 1), 20);
        assert_eq!(perft(&p, 2), 400);
        assert_eq!(perft(&p, 3), 8902);
        assert_eq!(perft(&p, 4), 197281);
    }

    #[test]
    fn perft_kiwipete() {
        // Posição famosa que testa roque, en passant e promoção.
        let p = Pos::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();
        assert_eq!(perft(&p, 1), 48);
        assert_eq!(perft(&p, 2), 2039);
        assert_eq!(perft(&p, 3), 97862);
    }

    #[test]
    fn perft_posicao3() {
        let p = Pos::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1").unwrap();
        assert_eq!(perft(&p, 4), 43238);
    }

    #[test]
    fn ve_mate_em_um() {
        // Mate do pastor: Dxf7#.
        let p = Pos::from_fen("r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4").unwrap();
        let m = best_move(&p, 2, 1).unwrap();
        assert_eq!((m.from, m.to), (39, 53));
    }
}


#[cfg(test)]
mod tempo {
    use super::*;
    #[test]
    #[ignore]
    fn mede() {
        for fen in ["r1bq1rk1/pp2bppp/2n1pn2/2pp4/3P4/2PBPN2/PP1N1PPP/R1BQ1RK1 w - - 0 8", "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"] {
            let p = Pos::from_fen(fen).unwrap();
            for d in [5] {
                let t = std::time::Instant::now();
                let m = best_move(&p, d, 5);
                eprintln!("profundidade {d}: {:?} em {:?}", m.map(|m| (m.from, m.to)), t.elapsed());
            }
        }
    }
}
