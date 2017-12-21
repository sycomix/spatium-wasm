extern crate ndarray;
extern crate ndarray_rand;
extern crate pcg_rand;
extern crate rand;

mod game;
use game::{Action, Game, GameState};

use std::sync::{Arc, RwLock, RwLockReadGuard};
use std::collections::HashMap;

use ndarray_rand::RandomExt;
use ndarray::Array2;
use ndarray::{Array, Ix1, Ix2};
use ndarray::prelude::*;

use pcg_rand::Pcg32Basic;

use rand::SeedableRng;
use rand::distributions::Range;
use rand::distributions::IndependentSample;

pub trait SpatiumSys {
    fn info(&self, &str) {}
    fn random(&mut self) -> f64;
    fn clear_screen(&self) {}
    fn draw_sprite(&self, i: usize, x: usize, y: usize) {}
    fn frame_info(&self, info: &str) {}
    fn episode_number(&self, i: usize) {}
}

pub struct SpatiumSysHelper<T: SpatiumSys> {
    sys: Arc<RwLock<T>>,
}

impl<T> Clone for SpatiumSysHelper<T>
where
    T: SpatiumSys,
{
    fn clone(&self) -> Self {
        SpatiumSysHelper {
            sys: Arc::clone(&self.sys),
        }
    }
}

impl<T: SpatiumSys> SpatiumSysHelper<T> {
    fn new(t: T) -> SpatiumSysHelper<T> {
        SpatiumSysHelper {
            sys: Arc::new(RwLock::new(t)),
        }
    }
    fn read(&self) -> RwLockReadGuard<T> {
        self.sys.read().unwrap()
    }
    fn info<S: Into<String>>(&self, s: S) {
        self.sys.read().unwrap().info(s.into().as_ref())
    }
    fn draw_sprite(&self, i: usize, x: usize, y: usize) {
        self.sys.read().unwrap().draw_sprite(i, x, y)
    }
    fn clear_screen(&self) {
        self.sys.read().unwrap().clear_screen()
    }
}

pub struct Spatium<T: SpatiumSys> {
    sys: SpatiumSysHelper<T>,
    rng: Pcg32Basic,
    episode: usize,
    step: usize,
    q: HashMap<Array<u8, Ix2>, Array<f32, Ix1>>,
    game: Option<Game>,
    last_state: Option<(GameState, usize, bool)>,
}

impl<T: SpatiumSys> Spatium<T> {
    pub fn new(sys: T) -> Spatium<T> {
        let n = Spatium {
            sys: SpatiumSysHelper::new(sys),
            rng: Pcg32Basic::from_seed([42, 42]),
            step: 0,
            episode: 0,
            q: HashMap::new(),
            game: None,
            last_state: None,
        };
        n.sys.info("Running Spatium");
        n
    }
    fn render(&self, game: &Game, action: Option<&Action>) {
        let sys = self.sys.read();

        sys.clear_screen();
        for s in &game.blocks {
            sys.draw_sprite(1, s.x, s.y);
        }
        for s in &game.food {
            sys.draw_sprite(2, s.x, s.y);
        }
        sys.draw_sprite(0, game.agent.x, game.agent.y);

        let action_str = action.map(|a| format!("{}", a)).unwrap_or(format!("None"));

        let frame_info = format!(
            "Episode: {}\nStep: {}\nDone: {}\nAction: {}",
            self.episode, game.step, game.done, action_str
        );
        sys.frame_info(frame_info.as_str());

        sys.episode_number(self.episode);
    }
    fn is_final_state(&self) -> bool {
        self.last_state.as_ref().map(|s| s.2).unwrap_or(false)
    }
    fn do_final_frame(&mut self) -> bool {
        let game = self.game.take().unwrap();
        self.render(&game, None);
        self.last_state = None;
        self.episode += 1;

        let sys = self.sys.read();
        sys.episode_number(self.episode);

        self.sys.info(format!(
            "Episode {} complete at step {}",
            self.episode, game.step
        ));

        // check if this was the last episode
        if self.episode >= 10 {
            self.sys.info(format!("All episodes executed"));
            return false;
        }
        return true;
    }
    fn reset_game(&mut self) {
        let (game, s, r, done) = Game::new();
        self.game = Some(game);
        self.last_state = Some((s, r, done));
        self.step += 1;
    }
    fn execute_action(&mut self, mut game: Game, action: &Action) -> (GameState, usize, bool) {
        // render current state and new action
        self.render(&game, Some(&action));

        // step game using action
        let state = game.step(self.sys.clone(), &action);

        // prepare for next step
        self.game = Some(game);
        self.last_state = Some(state.clone());
        self.step += 1;

        state
    }
    fn process(&mut self, game: Game, s: GameState) {
        let mut q_val = self.q
            .get(&s.arr)
            .map(|a| a.to_owned())
            .unwrap_or(Array::zeros((4)));

        let noise: Array1<f32> = Array1::random_using((4), Range::new(0., 3.), &mut self.rng);
        // self.sys.info(format!("{}", noise));

        let final_q_val = q_val.clone() + noise;

        let action_i = argmax(&final_q_val).0;
        let action = action_i.into();

        // render the current game and the decided action
        let (s1, r, done) = self.execute_action(game, &action);

        // update Q
        let s1_q_val = self.q
            .get(&s1.arr)
            .map(|a| a.to_owned())
            .unwrap_or(Array::zeros((4)));
        let r1 = argmax(&s1_q_val).1;

        let lr = 0.8f32;
        let y = 0.95f32;

        let existing = q_val[[action_i]];
        q_val[[action_i]] = existing + lr * (r as f32 + y * r1 - existing);
        self.q.insert(s.arr, q_val);
    }
    pub fn step(&mut self) -> bool {
        // render final state
        if self.is_final_state() {
            // returns false on end of final episode
            return self.do_final_frame();
        }

        // setup new game
        if self.last_state.is_none() {
            self.reset_game();
        }

        // extract current state
        let game = self.game.take().unwrap();
        let (s, last_r, _) = self.last_state.take().unwrap();

        // process step
        self.process(game, s);

        true
    }
}

fn argmax(arr: &Array1<f32>) -> (usize, f32) {
    arr.iter()
        .enumerate()
        .fold((0, std::f32::MIN), |(max_i, max_n), (i, n)| {
            if *n > max_n {
                (i, *n)
            } else {
                (max_i, max_n)
            }
        })
}

#[cfg(test)]
mod tests {
    extern crate rand;

    use super::*;

    struct SpatiumDummy;

    impl SpatiumSys for SpatiumDummy {
        fn info(&self, s: &str) {
            println!("info: {}", s);
        }
        fn random(&mut self) -> f64 {
            rand::random()
        }
    }

    #[test]
    fn test_max() {
        let mut n: Array<f32, Ix1> = Array::zeros((4));
        n[[0]] = 0f32;
        n[[1]] = 1f32;
        n[[2]] = 5f32;
        n[[3]] = 2f32;
        assert_eq!(argmax(&n), (2, 5f32));

        let mut n: Array<f32, Ix1> = Array::zeros((4));
        n[[0]] = -5f32;
        n[[1]] = -6f32;
        n[[2]] = -7f32;
        n[[3]] = -2f32;
        assert_eq!(argmax(&n), (3, -2f32));
    }

    #[test]
    fn it_works() {
        let mut spat = Spatium::new(SpatiumDummy {});
        while spat.step() {}
    }
}