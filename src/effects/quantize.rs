use super::{AtomicBool, Ordering, Pixel};
use std::collections::HashSet;
#[derive(Clone, Copy, Default)]
struct Bin {
    sum: [u64; 3],
    weight: u64,
}
impl Bin {
    fn color(self) -> [u8; 3] {
        self.sum
            .map(|sum| ((sum + self.weight / 2) / self.weight) as u8)
    }
}
fn cell(pixel: Pixel) -> usize {
    ((pixel[0] as usize >> 3) << 10) | ((pixel[1] as usize >> 3) << 5) | (pixel[2] as usize >> 3)
}
fn span(group: &[usize], bins: &[Bin]) -> (usize, u8, u64) {
    let (mut min, mut max, mut weight) = ([255u8; 3], [0u8; 3], 0u64);
    for &index in group {
        let bin = bins[index];
        let color = bin.color();
        weight += bin.weight;
        for c in 0..3 {
            min[c] = min[c].min(color[c]);
            max[c] = max[c].max(color[c]);
        }
    }
    let axis = (0..3)
        .max_by_key(|&c| (max[c] - min[c], std::cmp::Reverse(c)))
        .unwrap();
    (axis, max[axis] - min[axis], weight)
}
pub(super) fn render(source: &[Pixel], colors: u16, cancel: &AtomicBool) -> Option<Vec<Pixel>> {
    if !(2..=256).contains(&colors) {
        return None;
    }
    let mut bins = vec![Bin::default(); 32768];
    let mut unique = HashSet::new();
    for (i, &pixel) in source.iter().enumerate() {
        if i.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
            return None;
        }
        if pixel[3] == 0 {
            continue;
        }
        if unique.len() <= colors as usize {
            unique.insert([pixel[0], pixel[1], pixel[2]]);
        }
        let bin = &mut bins[cell(pixel)];
        let alpha = u64::from(pixel[3]);
        bin.weight += alpha;
        for (c, &value) in pixel[..3].iter().enumerate() {
            bin.sum[c] += u64::from(value) * alpha;
        }
    }
    if unique.len() <= colors as usize {
        return Some(source.to_vec());
    }
    let occupied: Vec<_> = bins
        .iter()
        .enumerate()
        .filter_map(|(i, b)| (b.weight > 0).then_some(i))
        .collect();
    let mut groups = vec![occupied];
    while groups.len() < colors as usize {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let choice = groups
            .iter()
            .enumerate()
            .filter(|(_, group)| group.len() > 1)
            .map(|(index, group)| {
                let (axis, range, weight) = span(group, &bins);
                (index, axis, range, u64::from(range) * weight)
            })
            .filter(|(_, _, range, _)| *range > 0)
            .max_by_key(|&(index, _, _, score)| (score, std::cmp::Reverse(index)));
        let Some((index, axis, _, _)) = choice else {
            break;
        };
        let group = &mut groups[index];
        group.sort_unstable_by_key(|&i| (bins[i].color()[axis], i));
        let total = group.iter().map(|&i| bins[i].weight).sum::<u64>();
        let mut accumulated = 0;
        let split = group
            .iter()
            .position(|&i| {
                accumulated += bins[i].weight;
                accumulated >= total.div_ceil(2)
            })
            .unwrap()
            + 1;
        let right = group.split_off(split.clamp(1, group.len() - 1));
        groups.push(right);
    }
    let palette: Vec<_> = groups
        .iter()
        .map(|group| {
            let mut combined = Bin::default();
            for &i in group {
                combined.weight += bins[i].weight;
                for c in 0..3 {
                    combined.sum[c] += bins[i].sum[c];
                }
            }
            combined.color()
        })
        .collect();
    let mut lookup = vec![[0u8; 3]; bins.len()];
    for (index, bin) in bins.iter().enumerate().filter(|(_, bin)| bin.weight > 0) {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let color = bin.color();
        lookup[index] = *palette
            .iter()
            .min_by_key(|candidate| {
                (0..3)
                    .map(|c| (i32::from(color[c]) - i32::from(candidate[c])).pow(2))
                    .sum::<i32>()
            })
            .unwrap();
    }
    let mut output = source.to_vec();
    for (i, pixel) in output.iter_mut().enumerate() {
        if i.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
            return None;
        }
        if pixel[3] > 0 {
            let index = cell(*pixel);
            pixel[..3].copy_from_slice(&lookup[index]);
        }
    }
    Some(output)
}
