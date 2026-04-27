use clap::Parser;
use doom_game::arena::MonsterArena;
use doom_types::mobj_kind::MobjKind;
use std::str::FromStr;

#[derive(Parser, Debug)]
#[command(name = "doom-arena", about = "Monster 1v1 simulator")]
pub struct ArenaArgs {
    /// First monster kind
    #[arg(long)]
    fighter_a: String,

    /// Second monster kind
    #[arg(long)]
    fighter_b: String,
}

pub fn run_arena(args: &ArenaArgs) -> anyhow::Result<()> {
    let a = MobjKind::from_str(&args.fighter_a)
        .map_err(|_| anyhow::anyhow!("Unknown monster: {}", args.fighter_a))?;
    let b = MobjKind::from_str(&args.fighter_b)
        .map_err(|_| anyhow::anyhow!("Unknown monster: {}", args.fighter_b))?;

    println!("Welcome to the DOOM ARENA!");
    println!("In the red corner: {:?}", a);
    println!("In the blue corner: {:?}", b);
    println!("FIGHT!");

    let result = MonsterArena::simulate_duel(a, b);

    println!("\n*** MATCH FINISHED in {} tics ***", result.duration_tics);
    println!("WINNER: {:?}", result.winner);
    println!("REMAINING HEALTH: {}", result.remaining_health);

    Ok(())
}
