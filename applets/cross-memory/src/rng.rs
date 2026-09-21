use sdk::api::time::Time;

pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new() -> Self {
        let time = Time::new();
        let seed = time.get_monotonic_time().as_nanos() as u64;
        Self { state: seed | 1 }
    }

    pub fn next_u32(&mut self) -> u32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state >> 16) as u32
    }
}
