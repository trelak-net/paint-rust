//! Bounded control-point curves with shape-preserving cubic interpolation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curve {
    points: [(u8, u8); 16],
    len: usize,
}
impl Default for Curve {
    fn default() -> Self {
        let mut points = [(0, 0); 16];
        points[1] = (255, 255);
        Self { points, len: 2 }
    }
}
impl Curve {
    pub fn points(&self) -> &[(u8, u8)] {
        &self.points[..self.len]
    }
    pub fn insert(&mut self, x: u8, y: u8) -> Option<usize> {
        match self.points().binary_search_by_key(&x, |p| p.0) {
            Ok(index) => {
                self.points[index].1 = y;
                Some(index)
            }
            Err(index) if self.len < self.points.len() => {
                self.points.copy_within(index..self.len, index + 1);
                self.points[index] = (x, y);
                self.len += 1;
                Some(index)
            }
            _ => None,
        }
    }
    pub fn move_point(&mut self, index: usize, x: u8, y: u8) {
        if index >= self.len {
            return;
        }
        let x = if index == 0 {
            0
        } else if index == self.len - 1 {
            255
        } else {
            x.clamp(self.points[index - 1].0 + 1, self.points[index + 1].0 - 1)
        };
        self.points[index] = (x, y);
    }
    pub fn remove(&mut self, index: usize) -> bool {
        if index == 0 || index >= self.len - 1 {
            return false;
        }
        self.points.copy_within(index + 1..self.len, index);
        self.len -= 1;
        self.points[self.len] = (0, 0);
        true
    }
    pub fn table(&self) -> [u8; 256] {
        let mut widths = [0.0; 15];
        let mut slopes = [0.0; 15];
        let mut tangents = [0.0; 16];
        for i in 0..self.len - 1 {
            widths[i] = f64::from(self.points[i + 1].0 - self.points[i].0);
            slopes[i] = (f64::from(self.points[i + 1].1) - f64::from(self.points[i].1)) / widths[i];
        }
        tangents[0] = slopes[0];
        tangents[self.len - 1] = slopes[self.len - 2];
        for i in 1..self.len - 1 {
            if slopes[i - 1] * slopes[i] > 0.0 {
                let a = 2.0 * widths[i] + widths[i - 1];
                let b = widths[i] + 2.0 * widths[i - 1];
                tangents[i] = (a + b) / (a / slopes[i - 1] + b / slopes[i]);
            }
        }
        let mut table = [0; 256];
        let mut interval = 0;
        for (x, output) in table.iter_mut().enumerate() {
            while interval + 2 < self.len && x > usize::from(self.points[interval + 1].0) {
                interval += 1;
            }
            let t = (x as f64 - f64::from(self.points[interval].0)) / widths[interval];
            let t2 = t * t;
            let t3 = t2 * t;
            let y = (2.0 * t3 - 3.0 * t2 + 1.0) * f64::from(self.points[interval].1)
                + (t3 - 2.0 * t2 + t) * widths[interval] * tangents[interval]
                + (-2.0 * t3 + 3.0 * t2) * f64::from(self.points[interval + 1].1)
                + (t3 - t2) * widths[interval] * tangents[interval + 1];
            *output = y.round().clamp(0.0, 255.0) as u8;
        }
        table
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Curves {
    /// Master RGB followed by red, green and blue.
    pub channels: [Curve; 4],
}
impl Curves {
    pub(crate) fn tables(&self) -> [[u8; 256]; 3] {
        let master = self.channels[0].table();
        std::array::from_fn(|channel| {
            let table = self.channels[channel + 1].table();
            std::array::from_fn(|value| table[usize::from(master[value])])
        })
    }
}
