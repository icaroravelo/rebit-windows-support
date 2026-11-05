//! Sample-counted ramps: event timing is independent of render buffer size.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ramp {
    pub current: f64,
    target: f64,
    step: f64,
    remaining: u32,
}
impl Ramp {
    pub fn new(value: f64) -> Self {
        Self {
            current: value,
            target: value,
            step: 0.,
            remaining: 0,
        }
    }
    pub fn reset(&mut self, value: f64) {
        *self = Self::new(value);
    }
    pub fn target(&mut self, value: f64, samples: u32) {
        if value == self.target {
            return;
        }
        self.remaining = samples.max(1);
        self.target = value;
        self.step = (value - self.current) / self.remaining as f64;
    }
    pub fn next(&mut self) -> f64 {
        if self.remaining > 0 {
            self.current += self.step;
            self.remaining -= 1;
            if self.remaining == 0 {
                self.current = self.target;
            }
        }
        self.current
    }
    pub fn running(&self) -> bool {
        self.remaining != 0
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Blend(pub [Ramp; 3]);
impl Blend {
    pub fn new(index: i32) -> Self {
        Self(std::array::from_fn(|i| {
            Ramp::new(f64::from(i == index as usize))
        }))
    }
    pub fn target(&mut self, index: i32, samples: u32) {
        for (i, ramp) in self.0.iter_mut().enumerate() {
            ramp.target(f64::from(i == index as usize), samples);
        }
    }
    pub fn current(&self) -> [f64; 3] {
        self.0.map(|r| r.current)
    }
    pub fn next(&mut self) -> [f64; 3] {
        std::array::from_fn(|i| self.0[i].next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_targets_do_not_restart_and_retargeting_is_continuous() {
        let mut r = Ramp::new(0.);
        r.target(1., 4);
        assert_eq!(r.next(), 0.25);
        r.target(1., 4);
        assert_eq!(r.next(), 0.5);
        r.target(0., 2);
        assert_eq!(r.next(), 0.25);
        assert_eq!(r.next(), 0.);
        assert!(!r.running());
    }
}
