use std::collections::HashMap;
use std::io::Read;

#[derive(Clone, Debug)]
pub(super) enum Value {
    Null,
    Integer(i64),
    Text(String),
    Ref(i32),
    Object(String, HashMap<String, Value>),
    Array(Vec<Value>),
    Bytes(std::sync::Arc<Vec<u8>>),
}
#[derive(Clone)]
struct Schema {
    name: String,
    fields: Vec<String>,
    primitives: Vec<Option<u8>>,
}
pub(super) struct Graph {
    pub root: i32,
    nodes: HashMap<i32, Value>,
}
impl Graph {
    pub fn resolve<'a>(&'a self, mut value: &'a Value) -> Result<&'a Value, String> {
        for _ in 0..64 {
            if let Value::Ref(id) = value {
                value = self.nodes.get(id).ok_or("Missing object reference")?;
            } else {
                return Ok(value);
            }
        }
        Err("Cyclic object reference".into())
    }
    pub fn root(&self) -> Result<&Value, String> {
        self.nodes
            .get(&self.root)
            .ok_or("Missing document root".into())
    }
    pub fn field<'a>(&'a self, value: &'a Value, name: &str) -> Result<&'a Value, String> {
        match self.resolve(value)? {
            Value::Object(_, fields) => self.resolve(
                fields
                    .get(name)
                    .ok_or_else(|| format!("Missing field {name}"))?,
            ),
            _ => Err("Expected an object".into()),
        }
    }
    pub fn optional<'a>(
        &'a self,
        value: &'a Value,
        name: &str,
    ) -> Result<Option<&'a Value>, String> {
        match self.resolve(value)? {
            Value::Object(_, fields) => fields.get(name).map(|v| self.resolve(v)).transpose(),
            _ => Err("Expected an object".into()),
        }
    }
    pub fn require_type(&self, value: &Value, expected: &str) -> Result<(), String> {
        match self.resolve(value)? {
            Value::Object(name, _) if name == expected => Ok(()),
            _ => Err(format!("Unsupported object schema; expected {expected}")),
        }
    }
}
impl Value {
    pub fn number(&self) -> Result<i64, String> {
        if let Self::Integer(n) = self {
            Ok(*n)
        } else {
            Err("Expected integer".into())
        }
    }
    pub fn text(&self) -> Result<&str, String> {
        if let Self::Text(s) = self {
            Ok(s)
        } else {
            Err("Expected string".into())
        }
    }
    pub fn array(&self) -> Result<&[Value], String> {
        if let Self::Array(a) = self {
            Ok(a)
        } else {
            Err("Expected array".into())
        }
    }
}
struct Parser<R> {
    input: R,
    remaining: usize,
    pixel_bytes_left: usize,
    values_left: usize,
    nodes: HashMap<i32, Value>,
    schemas: HashMap<i32, Schema>,
    nulls: usize,
}
impl<R: Read> Parser<R> {
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], String> {
        self.remaining = self
            .remaining
            .checked_sub(N)
            .ok_or("Object graph exceeds size limit")?;
        let mut bytes = [0; N];
        self.input
            .read_exact(&mut bytes)
            .map_err(|e| e.to_string())?;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.bytes::<1>()?[0])
    }
    fn int(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.bytes()?))
    }
    fn size(&mut self) -> Result<usize, String> {
        let n = self.int()?;
        if !(0..=65536).contains(&n) {
            return Err("Invalid object collection size".into());
        }
        Ok(n as usize)
    }
    fn string(&mut self) -> Result<String, String> {
        let mut n = 0u32;
        for shift in (0..35).step_by(7) {
            let b = self.byte()?;
            if shift == 28 && b > 7 {
                return Err("Invalid string length".into());
            }
            n |= u32::from(b & 127) << shift;
            if b & 128 == 0 {
                let n = n as usize;
                if n > 1024 * 1024 {
                    return Err("String exceeds limit".into());
                }
                self.remaining = self
                    .remaining
                    .checked_sub(n)
                    .ok_or("Object graph exceeds size limit")?;
                let mut bytes = vec![0; n];
                self.input
                    .read_exact(&mut bytes)
                    .map_err(|e| e.to_string())?;
                return String::from_utf8(bytes).map_err(|e| e.to_string());
            }
        }
        Err("Invalid string length".into())
    }
    fn primitive(&mut self, kind: u8) -> Result<Value, String> {
        let n = match kind {
            1 | 2 => i64::from(self.byte()?),
            7 => i64::from(i16::from_le_bytes(self.bytes()?)),
            8 => i64::from(self.int()?),
            9 | 12 | 13 => i64::from_le_bytes(self.bytes()?),
            10 => i64::from(self.byte()? as i8),
            14 => i64::from(u16::from_le_bytes(self.bytes()?)),
            15 => i64::from(u32::from_le_bytes(self.bytes()?)),
            16 => {
                i64::try_from(u64::from_le_bytes(self.bytes()?)).map_err(|_| "Integer overflow")?
            }
            6 => {
                self.bytes::<8>()?;
                return Ok(Value::Null);
            }
            11 => {
                self.bytes::<4>()?;
                return Ok(Value::Null);
            }
            17 => return Ok(Value::Null),
            18 => return self.string().map(Value::Text),
            _ => return Err(format!("Unsupported primitive {kind}")),
        };
        Ok(Value::Integer(n))
    }
    fn additional_type(&mut self, kind: u8) -> Result<Option<u8>, String> {
        match kind {
            0 | 7 => Ok(Some(self.byte()?)),
            1 | 2 | 5 | 6 => Ok(None),
            3 => {
                self.string()?;
                Ok(None)
            }
            4 => {
                self.string()?;
                self.int()?;
                Ok(None)
            }
            _ => Err("Unsupported member type".into()),
        }
    }
    fn insert(&mut self, id: i32, value: Value) -> Result<Value, String> {
        if self.nodes.len() >= 65536 || self.nodes.insert(id, value).is_some() {
            return Err("Duplicate object identifier or object limit exceeded".into());
        }
        Ok(Value::Ref(id))
    }
    fn object(&mut self, id: i32, schema: Schema, depth: usize) -> Result<Value, String> {
        self.values_left = self
            .values_left
            .checked_sub(schema.fields.len() + 1)
            .ok_or("Object allocation limit exceeded")?;
        let mut fields = HashMap::new();
        for (name, primitive) in schema.fields.iter().zip(&schema.primitives) {
            let value = match primitive {
                Some(p) => self.primitive(*p)?,
                None => self.record(depth + 1)?,
            };
            if fields.insert(name.clone(), value).is_some() {
                return Err("Duplicate member".into());
            }
        }
        self.insert(id, Value::Object(schema.name, fields))
    }
    fn record(&mut self, depth: usize) -> Result<Value, String> {
        if depth > 64 {
            return Err("Object nesting exceeds limit".into());
        }
        if self.nulls > 0 {
            self.nulls -= 1;
            return Ok(Value::Null);
        }
        loop {
            let kind = self.byte()?;
            match kind {
                1 => {
                    let id = self.int()?;
                    let metadata = self.int()?;
                    let schema = self
                        .schemas
                        .get(&metadata)
                        .ok_or("Missing class schema")?
                        .clone();
                    return self.object(id, schema, depth);
                }
                4 | 5 => {
                    let id = self.int()?;
                    let name = self.string()?;
                    if name.len() > 1024 {
                        return Err("Class name exceeds limit".into());
                    }
                    let count = self.size()?;
                    if count > 256 {
                        return Err("Too many class members".into());
                    }
                    let mut fields = Vec::with_capacity(count);
                    for _ in 0..count {
                        let field = self.string()?;
                        if field.len() > 256 {
                            return Err("Member name exceeds limit".into());
                        }
                        fields.push(field);
                    }
                    let mut kinds = Vec::with_capacity(count);
                    for _ in 0..count {
                        kinds.push(self.byte()?);
                    }
                    let mut primitives = Vec::with_capacity(count);
                    for kind in kinds {
                        let extra = self.additional_type(kind)?;
                        primitives.push(if kind == 0 { extra } else { None });
                    }
                    if kind == 5 {
                        self.int()?;
                    }
                    let schema = Schema {
                        name,
                        fields,
                        primitives,
                    };
                    if self.schemas.len() >= 65536
                        || self.schemas.insert(id, schema.clone()).is_some()
                    {
                        return Err("Duplicate class schema".into());
                    }
                    return self.object(id, schema, depth);
                }
                6 => {
                    let id = self.int()?;
                    let text = self.string()?;
                    return self.insert(id, Value::Text(text));
                }
                8 => {
                    let p = self.byte()?;
                    return self.primitive(p);
                }
                9 => return Ok(Value::Ref(self.int()?)),
                10 => return Ok(Value::Null),
                12 => {
                    self.int()?;
                    self.string()?;
                }
                13 | 14 => {
                    let count = if kind == 13 {
                        usize::from(self.byte()?)
                    } else {
                        self.size()?
                    };
                    if count == 0 {
                        return Err("Empty null run".into());
                    }
                    self.nulls = count - 1;
                    return Ok(Value::Null);
                }
                7 | 15 | 16 | 17 => {
                    let id = self.int()?;
                    let (count, primitive) = if kind == 7 {
                        let shape = self.byte()?;
                        if shape > 1 || self.int()? != 1 {
                            return Err("Unsupported array shape".into());
                        }
                        let count =
                            usize::try_from(self.int()?).map_err(|_| "Invalid array length")?;
                        let element = self.byte()?;
                        let primitive = self.additional_type(element)?;
                        (count, if element == 0 { primitive } else { None })
                    } else {
                        let count =
                            usize::try_from(self.int()?).map_err(|_| "Invalid array length")?;
                        (count, if kind == 15 { Some(self.byte()?) } else { None })
                    };
                    if primitive == Some(2) {
                        self.pixel_bytes_left = self
                            .pixel_bytes_left
                            .checked_sub(count)
                            .ok_or("Embedded pixel bytes exceed limit")?;
                        let mut bytes = vec![0u8; count];
                        self.input
                            .read_exact(&mut bytes)
                            .map_err(|e| e.to_string())?;
                        return self.insert(id, Value::Bytes(std::sync::Arc::new(bytes)));
                    }
                    if count > 65536 {
                        return Err("Invalid object collection size".into());
                    }
                    self.values_left = self
                        .values_left
                        .checked_sub(count + 1)
                        .ok_or("Array allocation limit exceeded")?;
                    let mut items = Vec::with_capacity(count);
                    for _ in 0..count {
                        items.push(match primitive {
                            Some(p) => self.primitive(p)?,
                            None => self.record(depth + 1)?,
                        });
                    }
                    if self.nulls > 0 {
                        return Err("Null run exceeds array length".into());
                    }
                    return self.insert(id, Value::Array(items));
                }
                _ => return Err(format!("Unsupported object record {kind}")),
            }
        }
    }
}
pub(super) fn read(input: impl Read) -> Result<Graph, String> {
    let mut p = Parser {
        input,
        remaining: 16 * 1024 * 1024,
        pixel_bytes_left: 256 * 1024 * 1024,
        values_left: 131072,
        nodes: HashMap::new(),
        schemas: HashMap::new(),
        nulls: 0,
    };
    if p.byte()? != 0 {
        return Err("Missing stream header".into());
    }
    let root = p.int()?;
    p.int()?;
    if p.int()? != 1 || p.int()? != 0 {
        return Err("Unsupported stream version".into());
    }
    loop {
        // A one-byte prefix allows detecting MessageEnd without consuming pixel data.
        let kind = p.byte()?;
        if kind == 11 {
            break;
        }
        let mut prefix = std::io::Cursor::new([kind]).chain(&mut p.input);
        let mut child = Parser {
            input: &mut prefix,
            remaining: p.remaining + 1,
            pixel_bytes_left: p.pixel_bytes_left,
            values_left: p.values_left,
            nodes: std::mem::take(&mut p.nodes),
            schemas: std::mem::take(&mut p.schemas),
            nulls: 0,
        };
        child.record(0)?;
        if child.nulls != 0 {
            return Err("Null run outside array".into());
        }
        p.remaining = child.remaining;
        p.pixel_bytes_left = child.pixel_bytes_left;
        p.values_left = child.values_left;
        p.nodes = child.nodes;
        p.schemas = child.schemas;
    }
    Ok(Graph {
        root,
        nodes: p.nodes,
    })
}

#[cfg(test)]
mod byte_array_tests {
    use super::*;
    fn prefix(count: i32) -> Vec<u8> {
        let mut data = vec![0];
        for n in [1i32, -1, 1, 0] {
            data.extend(n.to_le_bytes());
        }
        data.push(15);
        data.extend(1i32.to_le_bytes());
        data.extend(count.to_le_bytes());
        data.push(2);
        data
    }
    #[test]
    fn embedded_bytes_use_a_separate_bound_and_compact_storage() {
        let mut data = prefix(262144);
        data.extend(vec![57; 262144]);
        data.push(11);
        let graph = read(data.as_slice()).unwrap();
        let Value::Bytes(bytes) = graph.root().unwrap() else {
            panic!("Expected byte array");
        };
        assert_eq!(bytes.len(), 262144);
        assert!(bytes.iter().all(|b| *b == 57));
        for length in [-1, 256 * 1024 * 1024 + 1] {
            assert!(read(prefix(length).as_slice()).is_err());
        }
        assert!(read(prefix(262144).as_slice()).is_err());
    }
}
