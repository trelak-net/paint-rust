use super::*;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
#[derive(Clone)]
pub(super) struct Buffer {
    data: Arc<Vec<u8>>,
    start: usize,
    length: usize,
}
impl std::ops::Deref for Buffer {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.data[self.start..self.start + self.length]
    }
}
pub(super) struct Reader<'a, R> {
    graph: &'a Graph,
    input: R,
    cancel: &'a AtomicBool,
    allow_deferred: bool,
    cache: HashMap<usize, Buffer>,
    visiting: HashSet<usize>,
    allocated: usize,
}
impl<'a, R: Read> Reader<'a, R> {
    pub fn new(graph: &'a Graph, input: R, cancel: &'a AtomicBool, allow_deferred: bool) -> Self {
        Self {
            graph,
            input,
            cancel,
            allow_deferred,
            cache: HashMap::new(),
            visiting: HashSet::new(),
            allocated: 0,
        }
    }
    pub fn read(&mut self, value: &Value) -> Result<Buffer, String> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(crate::files::CANCELLED.into());
        }
        let value = self.graph.resolve(value)?;
        let key = value as *const Value as usize;
        if let Some(buffer) = self.cache.get(&key) {
            return Ok(buffer.clone());
        }
        if self.visiting.len() >= 32 || !self.visiting.insert(key) {
            return Err("Cyclic or excessive pixel parent nesting".into());
        }
        let g = self.graph;
        g.require_type(value, "PaintDotNet.MemoryBlock")?;
        let length = usize::try_from(
            g.optional(value, "length64")?
                .or(g.optional(value, "length")?)
                .ok_or("Missing buffer length")?
                .number()?,
        )
        .map_err(|_| "Invalid buffer length")?;
        if length > MAX_BYTES {
            return Err("Pixel memory exceeds limit".into());
        }
        let buffer = if boolean(g, value, "hasParent")? {
            let offset = usize::try_from(
                g.optional(value, "parentOffset64")?
                    .or(g.optional(value, "parentOffset")?)
                    .ok_or("Missing parent offset")?
                    .number()?,
            )
            .map_err(|_| "Invalid parent offset")?;
            let parent = self.read(g.field(value, "parentBlock")?)?;
            if offset
                .checked_add(length)
                .is_none_or(|end| end > parent.length)
            {
                return Err("Pixel view exceeds parent buffer".into());
            }
            Buffer {
                data: parent.data,
                start: parent.start + offset,
                length,
            }
        } else {
            self.allocated = self
                .allocated
                .checked_add(length)
                .filter(|n| *n <= MAX_BYTES)
                .ok_or("Pixel memory exceeds limit")?;
            let deferred = g
                .optional(value, "deferred")?
                .map(Value::number)
                .transpose()?
                .unwrap_or(0);
            let data = if deferred == 1 {
                if !self.allow_deferred {
                    return Err("Deferred pixels in compressed graph".into());
                }
                Arc::new(pixels(&mut self.input, length, self.cancel)?)
            } else if deferred != 0 {
                return Err("Invalid deferred flag".into());
            } else if let Some(data) = g.optional(value, "pointerData")? {
                let Value::Bytes(data) = data else {
                    return Err("Invalid embedded pixel bytes".into());
                };
                if data.len() != length {
                    return Err("Embedded pixel length mismatch".into());
                }
                data.clone()
            } else {
                let count = number(g, value, "chunkCount")?;
                if !(0..=4096).contains(&count) || number(g, value, "chunkFormat")? != 0 {
                    return Err("Unsupported embedded pixel chunks".into());
                }
                let mut data = Vec::with_capacity(length);
                for index in 0..count {
                    if self.cancel.load(Ordering::Relaxed) {
                        return Err(crate::files::CANCELLED.into());
                    }
                    let Value::Bytes(chunk) = g.field(value, &format!("chunk{index}"))? else {
                        return Err("Invalid embedded pixel chunk".into());
                    };
                    if chunk.len() > length - data.len() {
                        return Err("Embedded pixel chunks exceed buffer".into());
                    }
                    data.extend_from_slice(chunk);
                }
                if data.len() != length {
                    return Err("Embedded pixel chunks are incomplete".into());
                }
                Arc::new(data)
            };
            Buffer {
                data,
                start: 0,
                length,
            }
        };
        self.visiting.remove(&key);
        self.cache.insert(key, buffer.clone());
        Ok(buffer)
    }
}
