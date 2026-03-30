import re

with open("crates/doom-app/src/savegame.rs", "r") as f:
    content = f.read()

diff1 = """<<<<<<< SEARCH
    /// Secret count at save time.
    pub secret_count: u32,
}
=======
    /// Secret count at save time.
    pub secret_count: u32,
    /// Player arcade score.
    pub player_score: u32,
}
>>>>>>> REPLACE"""
with open("patch_sg1.diff", "w") as f:
    f.write(diff1)

diff2 = """<<<<<<< SEARCH
        kill_count: gs.kill_count,
        item_count: gs.item_count,
        secret_count: gs.secret_count,
    }
}
=======
        kill_count: gs.kill_count,
        item_count: gs.item_count,
        secret_count: gs.secret_count,
        player_score: gs.player.score,
    }
}
>>>>>>> REPLACE"""
with open("patch_sg2.diff", "w") as f:
    f.write(diff2)

diff3 = """<<<<<<< SEARCH
    gs.kill_count = payload.kill_count;
    gs.item_count = payload.item_count;
    gs.secret_count = payload.secret_count;

    // Restore player health.
=======
    gs.kill_count = payload.kill_count;
    gs.item_count = payload.item_count;
    gs.secret_count = payload.secret_count;
    gs.player.score = payload.player_score;

    // Restore player health.
>>>>>>> REPLACE"""
with open("patch_sg3.diff", "w") as f:
    f.write(diff3)

diff4 = """<<<<<<< SEARCH
            kill_count: 0,
            item_count: 0,
            secret_count: 0,
        };
        let config = bincode::config::standard();
=======
            kill_count: 0,
            item_count: 0,
            secret_count: 0,
            player_score: 0,
        };
        let config = bincode::config::standard();
>>>>>>> REPLACE"""
with open("patch_sg4.diff", "w") as f:
    f.write(diff4)
