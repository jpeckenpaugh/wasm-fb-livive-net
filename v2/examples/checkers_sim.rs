use appliance::checkers_sm::{CheckersState, Difficulty, select_full_ai_turn, PLAYER_RED};

fn main() {
    let mut state = CheckersState::new();
    println!("Starting simulation: P1 (Red, Hard 6-Ply) vs P2 (Black, Easy 2-Ply)...");

    let mut turn_count = 0;
    while state.winner == 0 && turn_count < 150 {
        let diff = if state.turn == PLAYER_RED {
            Difficulty::Hard // 6-ply
        } else {
            Difficulty::Easy // 2-ply
        };
        let p = if state.turn == PLAYER_RED { "P1 (Red, 6-Ply)" } else { "P2 (Black, 2-Ply)" };
        let rec = select_full_ai_turn(&mut state, diff);
        if rec.hop_len < 2 {
            println!("Turn {}: {} has no legal moves!", turn_count + 1, p);
            break;
        }
        turn_count += 1;
        let mut pdn = String::new();
        for (i, h) in rec.hops[..rec.hop_len as usize].iter().enumerate() {
            if i > 0 {
                pdn.push_str(if rec.is_jump { "x" } else { "-" });
            }
            pdn.push_str(&(h + 1).to_string());
        }
        let (red_c, blk_c) = state.piece_counts();
        println!("Turn {:2}: {} played {:>8} -> Red: {:2}, Black: {:2}, Hash: 0x{:08x}", 
            turn_count, p, pdn, red_c, blk_c, state.state_hash());
    }

    println!("\nSimulation completed in {} turns.", turn_count);
    match state.winner {
        1 => println!("🏆 WINNER: Player 1 (Red, 6-Ply Minimax) WON!"),
        2 => println!("🏆 WINNER: Player 2 (Black, 2-Ply Minimax) WON!"),
        3 => println!("🤝 RESULT: Draw / Stalemate"),
        _ => println!("❓ RESULT: Inconclusive (stopped at {} turns)", turn_count),
    }
}
