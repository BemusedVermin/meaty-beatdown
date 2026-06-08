//! Weapons — your **spacing identity**. A weapon sets the lane reach and frame deltas, and grants a
//! move; the range ↔ speed ↔ damage tradeoff (spec R-4) lives in the data, so no weapon dominates.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WeaponClass {
    Dagger,
    Sword,
    Greatsword,
    Spear,
}

#[derive(Clone, Copy, Debug)]
pub struct Weapon {
    pub class: WeaponClass,
    pub min_range: f32,
    pub max_range: f32,
    pub startup_delta: i32,
    pub recovery_delta: i32,
    pub damage_delta: i32,
    /// STR score required to wield at full numbers (a floor — unmet ⇒ unusable).
    pub req_str: i32,
}

impl Weapon {
    /// The canonical weapon of a class.
    pub fn of(class: WeaponClass) -> Self {
        match class {
            WeaponClass::Dagger => Weapon {
                class,
                min_range: 0.2,
                max_range: 1.4,
                startup_delta: -1,
                recovery_delta: -1,
                damage_delta: -2,
                req_str: 6,
            },
            WeaponClass::Sword => Weapon {
                class,
                min_range: 0.4,
                max_range: 2.0,
                startup_delta: 0,
                recovery_delta: 0,
                damage_delta: 0,
                req_str: 9,
            },
            WeaponClass::Greatsword => Weapon {
                class,
                min_range: 0.6,
                max_range: 2.8,
                startup_delta: 4,
                recovery_delta: 6,
                damage_delta: 8,
                req_str: 14,
            },
            WeaponClass::Spear => Weapon {
                class,
                min_range: 1.0,
                max_range: 3.2,
                startup_delta: 1,
                recovery_delta: 2,
                damage_delta: 0,
                req_str: 9,
            },
        }
    }
}
