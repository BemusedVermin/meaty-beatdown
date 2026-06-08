//! # `debuglog` — a plain-text combat / exploration trace to a file
//!
//! A view-only diagnostic. At startup it opens [`LOG_PATH`] in the process working directory
//! (usually `rust/`) and appends human-readable lines as the game runs: state transitions, the
//! **fight setup** (so the *live* frame data is visible — e.g. whether a move's `hitstun` is below
//! its `total`), per-tick HP / reaction changes, every decision + commit, and overworld travel.
//!
//! Nothing here touches the simulation. If the file can't be opened, every call silently no-ops, so
//! the game still runs. The combat and exploration drivers write their own detail through
//! `Res<DebugLog>`; this module owns the sink + the always-on state-transition watchers.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::Mutex;

use bevy::prelude::*;

use crate::state::{CombatState, GameState};

/// Trace destination, relative to the working directory the game is launched from.
const LOG_PATH: &str = "tick-debug.log";

/// A shared, append-only text log. Interior-mutable (`Mutex`) so any number of systems can hold a
/// `Res<DebugLog>` and write concurrently — each just briefly locks the writer.
#[derive(Resource)]
pub struct DebugLog {
    out: Mutex<Option<BufWriter<File>>>,
}

impl DebugLog {
    /// Truncate-open the log; on failure the writer is `None` and [`line`](Self::line) no-ops.
    fn open(path: &str) -> Self {
        DebugLog { out: Mutex::new(File::create(path).ok().map(BufWriter::new)) }
    }

    /// Append one line as `CHANNEL | message`, flushed immediately so a crash still leaves a full
    /// trace on disk.
    pub fn line(&self, channel: &str, msg: impl AsRef<str>) {
        if let Ok(mut guard) = self.out.lock() {
            if let Some(w) = guard.as_mut() {
                let _ = writeln!(w, "{channel:<8}| {}", msg.as_ref());
                let _ = w.flush();
            }
        }
    }
}

/// Opens the trace and registers the always-on transition watchers.
pub struct DebugLogPlugin;

impl Plugin for DebugLogPlugin {
    fn build(&self, app: &mut App) {
        let log = DebugLog::open(LOG_PATH);
        log.line("boot", "════════ session start ════════");
        match std::path::Path::new(LOG_PATH).canonicalize() {
            Ok(p) => info!("debug trace → {}", p.display()),
            Err(_) => info!("debug trace → {LOG_PATH} (relative to cwd)"),
        }
        app.insert_resource(log)
            .add_systems(Update, log_combat_state.run_if(state_changed::<CombatState>))
            .add_systems(Update, log_game_state.run_if(state_changed::<GameState>));
    }
}

fn log_combat_state(log: Res<DebugLog>, s: Res<State<CombatState>>) {
    log.line("state", format!("CombatState → {:?}", s.get()));
}

fn log_game_state(log: Res<DebugLog>, s: Res<State<GameState>>) {
    log.line("state", format!("GameState → {:?}", s.get()));
}
