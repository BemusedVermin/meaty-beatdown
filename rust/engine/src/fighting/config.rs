//! Why this module is (almost) empty: the engine intentionally holds **no combat constants**.
//!
//! Every combat magnitude — counter-hit bonus, parry freeze/recover, blockstun, knockdown duration,
//! throw-tech recover, chip — is **authored on the move** ([`super::frame::Attack`] /
//! [`super::frame::QualityKind`]), because there are **no generic/built-in moves**: every fighter
//! acts only through authored content whose values the game determines.
//!
//! The one engine-level number is [`super::sim::Sim::max_ticks`] — an *optional* safety bound so a
//! fully turn-based, timer-less bout cannot loop forever in a headless AI / replay run. It defaults
//! to `None` (no cap) and is **not** a game timer: the game has no rounds and no timer.
