#[macro_use]
extern crate lazy_static;
extern crate pcg_rand;
extern crate rand;
extern crate spatium_lib;
extern crate serde_json;

mod externs;
pub mod exports;

mod spatium_js_sys;

use std::sync::Mutex;
use rand::SeedableRng;

use spatium_lib::{RcRng, Spatium, SpatiumSys};
use spatium_js_sys::SpatiumJsSys;

type SpatiumJs = Mutex<Option<Spatium<SpatiumJsSys>>>;

lazy_static! {
  static ref DATA: SpatiumJs = Mutex::new(None);
}

fn setup(max_episodes: usize) {
    let mut data = DATA.lock().unwrap();
    *data = Some(Spatium::new(rng(), SpatiumJsSys::new(), max_episodes));
    SpatiumJsSys::new().info("Setup complete");
}

fn random_u64() -> u64 {
    use std::u64::MAX;
    use externs::random;
    (random() * MAX as f64) as u64
}

fn rng() -> RcRng {
    use pcg_rand::Pcg32Basic;
    let rng = Pcg32Basic::from_seed([random_u64(), random_u64()]);
    RcRng::new(Box::new(rng))
}

fn step(count: usize) -> String {
    // let mut s = Spatium::new(rng(), SpatiumJsSys::new(), 250);
    // s.step(rng());
    // true
    match DATA.lock().unwrap().as_mut() {
        Some(data) => {
            let result: Vec<_> = (0..count).into_iter().map(|_| {
                data.step(rng())
            }).collect();
            // let result = data.step(rng());
            serde_json::to_string(&result).unwrap()
        },
        None => {
            SpatiumJsSys::new().fatal("Run spatium::setup() first");
            String::from("{\"done\": true}")
        }
    }
}
