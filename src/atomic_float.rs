use std::{
  fmt::Display,
  sync::atomic::{AtomicU32, Ordering},
};

pub struct AtomicF32 {
  val: AtomicU32,
}

#[allow(dead_code)]
impl AtomicF32 {
  pub fn new(val: f32) -> AtomicF32 {
    AtomicF32 {
      val: unsafe { std::mem::transmute(val) },
    }
  }
  pub fn store(&self, val: f32, order: Ordering) {
    self.val.store(unsafe { std::mem::transmute(val) }, order);
  }

  pub fn load(&self, order: Ordering) -> f32 {
    unsafe { std::mem::transmute(self.val.load(order)) }
  }

  pub fn compare_exchange_weak(
    &self,
    current: f32,
    new: f32,
    success: Ordering,
    failure: Ordering,
  ) -> Result<f32, f32> {
    self
      .val
      .compare_exchange_weak(
        unsafe { std::mem::transmute(current) },
        unsafe { std::mem::transmute(new) },
        success,
        failure,
      )
      .map_or_else(
        |fail| Err(unsafe { std::mem::transmute(fail) }),
        |succ| Ok(unsafe { std::mem::transmute(succ) }),
      )
  }
  pub fn compare_exchange(
    &self,
    current: f32,
    new: f32,
    success: Ordering,
    failure: Ordering,
  ) -> Result<f32, f32> {
    self
      .val
      .compare_exchange(
        unsafe { std::mem::transmute(current) },
        unsafe { std::mem::transmute(new) },
        success,
        failure,
      )
      .map_or_else(
        |fail| Err(unsafe { std::mem::transmute(fail) }),
        |succ| Ok(unsafe { std::mem::transmute(succ) }),
      )
  }
}

impl Clone for AtomicF32 {
  fn clone(&self) -> Self {
    Self {
      val: unsafe { std::mem::transmute(self.val.load(Ordering::Relaxed)) },
    }
  }
}

#[derive(Clone, Copy, PartialEq)]
pub enum State {
  Attack(f32),
  Decay(f32),
  Sustain,
  Release(f32),
  Silent,
}

impl Eq for State {}

#[allow(dead_code)]
impl State {
  pub fn is_attack(&self) -> bool {
    use State::*;
    if let Attack(_) = self {
      return true;
    }
    return false;
  }
  pub fn is_decay(&self) -> bool {
    use State::*;
    if let Decay(_) = self {
      return true;
    }
    return false;
  }
  pub fn is_sustain(&self) -> bool {
    use State::*;
    if let Sustain = self {
      return true;
    }
    return false;
  }
  pub fn is_release(&self) -> bool {
    use State::*;
    if let Release(_) = self {
      return true;
    }
    return false;
  }
  pub fn is_silent(&self) -> bool {
    use State::*;
    if let Silent = self {
      return true;
    }
    return false;
  }
}

impl Display for State {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!(
      "{}",
      match self {
        State::Attack(val) => format!("Attack({})", val),
        State::Decay(val) => format!("Decay({})", val),
        State::Sustain => "Sustain".to_owned(),
        State::Release(val) => format!("Release({})", val),
        State::Silent => "Silent".to_owned(),
      }
    ))
  }
}

use crossbeam::atomic::AtomicCell;

pub struct AtomicState {
  cell: AtomicCell<State>,
  attack_max: f32,
  attack_gain: f32,
  decay_fall: f32,
  sustain_level: f32,
  release_fall: f32,
}

impl Display for AtomicState {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!("{}", self.cell.load()))
  }
}

impl Clone for AtomicState {
  fn clone(&self) -> Self {
    Self {
      cell: AtomicCell::new(self.cell.load()),
      attack_max: self.attack_max,
      attack_gain: self.attack_gain,
      decay_fall: self.decay_fall,
      sustain_level: self.sustain_level,
      release_fall: self.release_fall,
    }
  }
}

impl AtomicState {
  pub fn new(
    state: State,
    attack_max: f32,
    attack_gain: f32,
    decay_fall: f32,
    sustain_level: f32,
    release_fall: f32,
  ) -> Self {
    Self {
      cell: AtomicCell::new(state),
      attack_max,
      attack_gain,
      decay_fall,
      sustain_level,
      release_fall,
    }
  }

  pub fn set(&self, state: State) {
    self.cell.store(state)
  }

  pub fn next(&self) -> f32 {
    use State::*;
    let update = |old| {
      Some(match old {
        Attack(val) => {
          if val > self.attack_max {
            Decay(self.attack_max)
          } else {
            Attack(val + self.attack_gain)
          }
        }
        Decay(val) => {
          if val < self.sustain_level {
            Sustain
          } else {
            Decay(val - self.decay_fall)
          }
        }
        Sustain => Sustain,
        Release(val) => {
          if val < 0. {
            Silent
          } else {
            Release(val - self.release_fall)
          }
        }
        Silent => Silent,
      })
    };
    if let Ok(old) = self.cell.fetch_update(update) {
      match old {
        State::Attack(val) => val,
        State::Decay(val) => val,
        State::Sustain => self.sustain_level,
        State::Release(val) => val,
        State::Silent => 0.0,
      }
    } else {
      unreachable!();
    }
  }
  #[allow(dead_code)]
  pub fn is_silent(&self) -> bool {
    return self.cell.load().is_silent();
  }
}
