extern crate anyhow;
extern crate clap;
extern crate cpal;

use std::{
  io::Read,
  sync::{mpsc::Receiver, Arc},
};

use atomic_float::{AtomicState, State};
use clap::arg;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam::atomic::AtomicCell;

#[derive(Debug)]
struct Opt {
  #[cfg(all(
    any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"),
    feature = "jack"
  ))]
  jack: bool,

  device: String,
}

impl Opt {
  fn from_args() -> Self {
    let app = clap::Command::new("beep").arg(arg!([DEVICE] "The audio device to use"));
    #[cfg(all(
      any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"),
      feature = "jack"
    ))]
    let app = app.arg(arg!(-j --jack "Use the JACK host"));
    let matches = app.get_matches();
    let device = matches.value_of("DEVICE").unwrap_or("default").to_string();

    #[cfg(all(
      any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"),
      feature = "jack"
    ))]
    return Opt {
      jack: matches.is_present("jack"),
      device,
    };

    #[cfg(any(
      not(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd")),
      not(feature = "jack")
    ))]
    Opt { device }
  }
}

fn main() -> anyhow::Result<()> {
  let opt = Opt::from_args();

  // Conditionally compile with jack if the feature is specified.
  #[cfg(all(
    any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"),
    feature = "jack"
  ))]
  // Manually check for flags. Can be passed through cargo with -- e.g.
  // cargo run --release --example beep --features jack -- --jack
  let host = if opt.jack {
    cpal::host_from_id(
      cpal::available_hosts()
        .into_iter()
        .find(|id| *id == cpal::HostId::Jack)
        .expect(
          "make sure --features jack is specified. only works on OSes where jack is available",
        ),
    )
    .expect("jack host unavailable")
  } else {
    cpal::default_host()
  };

  #[cfg(any(
    not(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd")),
    not(feature = "jack")
  ))]
  let host = cpal::default_host();

  let device = if opt.device == "default" {
    host.default_output_device()
  } else {
    host
      .output_devices()?
      .find(|x| x.name().map(|y| y == opt.device).unwrap_or(false))
  }
  .expect("failed to find output device");
  println!("Output device: {}", device.name()?);

  let config = device.default_output_config().unwrap();
  println!("Default output config: {:?}", config);

  match config.sample_format() {
    cpal::SampleFormat::F32 => run::<f32>(&device, &config.into()),
    cpal::SampleFormat::I16 => run::<i16>(&device, &config.into()),
    cpal::SampleFormat::U16 => run::<u16>(&device, &config.into()),
  }
}

enum KbdKey {
  Pressed(u16),
  Released(u16),
}

impl KbdKey {
  fn unwrap(&self) -> u16 {
    match self {
      KbdKey::Pressed(key) => *key,
      KbdKey::Released(key) => *key,
    }
  }
  #[allow(dead_code)]
  fn is_pressed(&self) -> bool {
    if let KbdKey::Pressed(_) = self {
      true
    } else {
      false
    }
  }
}

struct KeyboardHook {
  key_receiver: Receiver<KbdKey>,
}

impl KeyboardHook {
  fn new() -> Self {
    let (key_sender, key_receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
      use std::process::{Command, Stdio};
      let mut child = Command::new("sudo")
        .arg("./kbd")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
      let stdout = child.stdout.as_mut().unwrap();
      loop {
        let mut press_buf = [0u8; 1];
        let mut keycode_buf = [0u8; 2];
        stdout.read(&mut press_buf).unwrap();
        stdout.read(&mut keycode_buf).unwrap();
        let press = press_buf[0] == 1;
        let keycode = u16::from_le_bytes(keycode_buf);
        if key_sender
          .send(if press {
            KbdKey::Pressed(keycode)
          } else {
            KbdKey::Released(keycode)
          })
          .is_err()
        {
          return;
        }
      }
    });
    KeyboardHook { key_receiver }
  }

  fn read(&self) -> KbdKey {
    self.key_receiver.recv().unwrap()
  }
}

trait TriangleExt {
  fn triangle(self) -> Self;
}

impl TriangleExt for f32 {
  fn triangle(self) -> Self {
    const T: f32 = 2. * std::f32::consts::PI;
    (self / T - (self / T + 0.5).floor()).abs() * 2.
  }
}

trait SquareExt {
  fn square(self) -> Self;
}

impl SquareExt for f32 {
  fn square(self) -> Self {
    self.sin().signum()
  }
}

mod atomic_float;

fn sin(v: f32) -> f32 {
  v.sin()
}
fn square(v: f32) -> f32 {
  v.square()
}
fn triangle(v: f32) -> f32 {
  v.triangle()
}

#[derive(Clone, Copy)]
struct FnPtr {
  ptr: &'static dyn Fn(f32) -> f32,
}

impl FnPtr {
  fn new(ptr: &'static dyn Fn(f32) -> f32) -> Self {
    Self { ptr }
  }
  fn call(&self, x: f32) -> f32 {
    let fnptr = self.ptr;
    fnptr(x)
  }
}
unsafe impl Send for FnPtr {}

pub fn run<T>(device: &cpal::Device, config: &cpal::StreamConfig) -> Result<(), anyhow::Error>
where
  T: cpal::Sample,
{
  let notes = Arc::new(vec![
    AtomicState::new(
      State::Silent,
      0.98,
      0.0005,
      0.00002,
      0.01,
      0.00002
    );
    23
  ]);
  let sample_rate = config.sample_rate.0 as f32;
  let channels = config.channels as usize;

  let fnptr: Arc<AtomicCell<FnPtr>> = Arc::new(AtomicCell::new(FnPtr::new(&sin)));
  let fnptr_tx = Arc::clone(&fnptr);

  // Produce a sinusoid of maximum amplitude.
  let mut sample_clock = 0f32;
  let notes_tx = Arc::clone(&notes);
  let mut next_value = move || {
    sample_clock = (sample_clock + 1.) % (2. * sample_rate); //(sample_clock + 1.0) % sample_rate;
    let mut sum = 0.0;
    let mut idx = 0;
    let frequencies = [
      27.5, 30.868, 32.703, 36.708, 41.203, 43.654, 48.999, 55., 61.735, 65.406, 73.416, 82.407,
      87.307, 97.999, 110., 123.47, 130.81, 146.83, 164.81, 174.61, 196., 220., 246.94, 261.6,
      293.67, 329.63, 349.23, 392.00, 440.00, 493.88, 523.25, 587.33, 659.26, 698.46, 783.99,
      880.00, 987.77, 1046.5, 1174.7, 1318.5, 1568.0, 1760.0, 1975.5, 2093.0, 2349.3, 2637.0,
      2793.8, 3136.0, 3520.0, 3951.1, 4186.0,
    ];
    for note in notes_tx.as_ref() {
      let t = sample_clock * frequencies[idx + 23] * std::f32::consts::PI / sample_rate;
      sum += fnptr_tx.load().call(t) / 2. * note.next().powf(2.) / (idx + 2) as f32;
      idx += 1;
    }
    sum
  };

  let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

  let stream = device.build_output_stream(
    config,
    move |data: &mut [T], _: &cpal::OutputCallbackInfo| write_data(data, channels, &mut next_value),
    err_fn,
  )?;
  stream.play()?;
  let hook = KeyboardHook::new();
  loop {
    let key = hook.read();
    match key.unwrap() {
      highrow if highrow >= 16 && highrow <= 27 => {
        let peeked = notes[(highrow - 16) as usize].peek();
        notes[(highrow - 16) as usize].set(if key.is_pressed() {
          State::Attack(peeked)
        } else {
          State::Release(peeked)
        })
      }
      middlerow if middlerow >= 30 && middlerow <= 40 => {
        let peeked = notes[(middlerow - 30 + 12) as usize].peek();
        notes[(middlerow - 30 + 12) as usize].set(if key.is_pressed() {
          State::Attack(peeked)
        } else {
          State::Release(peeked)
        })
      }
      key if key == 44 => {
        fnptr.store(FnPtr::new(&sin));
      }
      key if key == 45 => {
        fnptr.store(FnPtr::new(&triangle));
      }
      key if key == 46 => {
        fnptr.store(FnPtr::new(&square));
      }
      _ => {
        if let KbdKey::Released(keycode) = key {
          if keycode == 1 {
            break;
          }
        }
      }
    };
  }

  Ok(())
}

fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut dyn FnMut() -> f32)
where
  T: cpal::Sample,
{
  for frame in output.chunks_mut(channels) {
    let value: T = cpal::Sample::from::<f32>(&next_sample());
    for sample in frame.iter_mut() {
      *sample = value;
    }
  }
}
