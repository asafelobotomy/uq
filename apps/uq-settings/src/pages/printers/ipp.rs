//! The Internet Printing Protocol as CUPS speaks it (RFC 8010): encode one request, decode one
//! response. Only what the Printers page needs: integers, enums, booleans and strings; anything
//! else (a collection, a date, a resolution) is decoded past and kept as [`Value::Other`], so
//! an unexpected attribute never breaks a read.

/// Operation ids (RFC 8011 and CUPS's own).
pub mod op {
    pub const PRINT_JOB: u16 = 0x0002;
    pub const CANCEL_JOB: u16 = 0x0008;
    pub const GET_JOBS: u16 = 0x000A;
    pub const PAUSE_PRINTER: u16 = 0x0010;
    pub const RESUME_PRINTER: u16 = 0x0011;
    pub const CUPS_GET_DEFAULT: u16 = 0x4001;
    pub const CUPS_GET_PRINTERS: u16 = 0x4002;
    pub const CUPS_ADD_MODIFY_PRINTER: u16 = 0x4003;
    pub const CUPS_DELETE_PRINTER: u16 = 0x4004;
    pub const CUPS_ACCEPT_JOBS: u16 = 0x4008;
    pub const CUPS_SET_DEFAULT: u16 = 0x400A;
    pub const CUPS_GET_DEVICES: u16 = 0x400B;
    pub const CUPS_GET_PPDS: u16 = 0x400C;
}

/// Delimiter and value tags.
pub mod tag {
    pub const OPERATION: u8 = 0x01;
    pub const JOB: u8 = 0x02;
    pub const END: u8 = 0x03;
    pub const PRINTER: u8 = 0x04;
    pub const INTEGER: u8 = 0x21;
    pub const BOOLEAN: u8 = 0x22;
    pub const ENUM: u8 = 0x23;
    pub const BEGIN_COLLECTION: u8 = 0x34;
    pub const TEXT_WITH_LANGUAGE: u8 = 0x35;
    pub const NAME_WITH_LANGUAGE: u8 = 0x36;
    pub const END_COLLECTION: u8 = 0x37;
    pub const TEXT: u8 = 0x41;
    pub const NAME: u8 = 0x42;
    pub const KEYWORD: u8 = 0x44;
    pub const URI: u8 = 0x45;
    pub const CHARSET: u8 = 0x47;
    pub const LANGUAGE: u8 = 0x48;
    pub const MIME_TYPE: u8 = 0x49;
}

/// `successful-ok` and its two "but" variants are below 0x0100.
pub fn is_success(status: u16) -> bool {
    status < 0x0100
}

/// A value to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Out {
    Int(i32),
    Enum(i32),
    Bool(bool),
    /// A string with its value tag (text, name, keyword, uri, mimeType…).
    Str(u8, String),
}

impl Out {
    fn tag(&self) -> u8 {
        match self {
            Out::Int(_) => tag::INTEGER,
            Out::Enum(_) => tag::ENUM,
            Out::Bool(_) => tag::BOOLEAN,
            Out::Str(t, _) => *t,
        }
    }

    fn bytes(&self) -> Vec<u8> {
        match self {
            Out::Int(v) | Out::Enum(v) => v.to_be_bytes().to_vec(),
            Out::Bool(b) => vec![u8::from(*b)],
            Out::Str(_, s) => s.as_bytes().to_vec(),
        }
    }
}

/// One group's attributes to send: `[(name, values)]`.
pub type Attrs = Vec<(String, Vec<Out>)>;

/// One request: the operation, its attribute groups, and any document data after them.
#[derive(Debug, Clone)]
pub struct Request {
    pub op: u16,
    pub id: u32,
    /// `(group tag, [(name, values)])`, in order. The operation group comes first.
    pub groups: Vec<(u8, Attrs)>,
    pub data: Vec<u8>,
}

impl Request {
    /// A request with the operation group's charset and language already set, as every IPP
    /// request must start.
    pub fn new(op: u16) -> Self {
        let mut r = Self {
            op,
            id: 1,
            groups: vec![(tag::OPERATION, Vec::new())],
            data: Vec::new(),
        };
        r.op_attr("attributes-charset", Out::Str(tag::CHARSET, "utf-8".into()));
        r.op_attr(
            "attributes-natural-language",
            Out::Str(tag::LANGUAGE, "en".into()),
        );
        r
    }

    /// Add an attribute to the operation group.
    pub fn op_attr(&mut self, name: &str, value: Out) -> &mut Self {
        self.groups[0].1.push((name.to_string(), vec![value]));
        self
    }

    /// Add a multi-valued attribute to the operation group.
    pub fn op_attrs(&mut self, name: &str, values: Vec<Out>) -> &mut Self {
        self.groups[0].1.push((name.to_string(), values));
        self
    }

    /// Add an attribute to the printer group (created after the operation group on first use).
    pub fn printer_attr(&mut self, name: &str, value: Out) -> &mut Self {
        if self.groups.last().map(|g| g.0) != Some(tag::PRINTER) {
            self.groups.push((tag::PRINTER, Vec::new()));
        }
        let last = self.groups.len() - 1;
        self.groups[last].1.push((name.to_string(), vec![value]));
        self
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(256 + self.data.len());
        b.extend_from_slice(&[2, 0]);
        b.extend_from_slice(&self.op.to_be_bytes());
        b.extend_from_slice(&self.id.to_be_bytes());
        for (group, attrs) in &self.groups {
            b.push(*group);
            for (name, values) in attrs {
                for (i, v) in values.iter().enumerate() {
                    b.push(v.tag());
                    let name = if i == 0 { name.as_bytes() } else { &[] };
                    push_len(&mut b, name);
                    push_len(&mut b, &v.bytes());
                }
            }
        }
        b.push(tag::END);
        b.extend_from_slice(&self.data);
        b
    }
}

fn push_len(b: &mut Vec<u8>, bytes: &[u8]) {
    // IPP lengths are 16 bits; every value the page sends is far shorter.
    let len = u16::try_from(bytes.len()).unwrap_or(u16::MAX);
    b.extend_from_slice(&len.to_be_bytes());
    b.extend_from_slice(&bytes[..usize::from(len)]);
}

/// A value received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// An integer or an enum.
    Int(i32),
    Bool(bool),
    /// Any string type, with or without a language.
    Text(String),
    /// A type the page doesn't read, or an out-of-band value ("unknown", "no-value").
    Other,
}

/// One attribute group of a response (a printer, a job, a device).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Group {
    pub tag: u8,
    pub attrs: Vec<(String, Vec<Value>)>,
}

impl Group {
    fn values(&self, name: &str) -> &[Value] {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map_or(&[], |(_, v)| v.as_slice())
    }

    pub fn str(&self, name: &str) -> Option<&str> {
        self.values(name).iter().find_map(|v| match v {
            Value::Text(s) => Some(s.as_str()),
            _ => None,
        })
    }

    /// The attribute's text, or "" when it is missing.
    pub fn text(&self, name: &str) -> String {
        self.str(name).unwrap_or_default().to_string()
    }

    pub fn strs(&self, name: &str) -> Vec<String> {
        self.values(name)
            .iter()
            .filter_map(|v| match v {
                Value::Text(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn int(&self, name: &str) -> Option<i32> {
        self.values(name).iter().find_map(|v| match v {
            Value::Int(i) => Some(*i),
            _ => None,
        })
    }

    pub fn bool(&self, name: &str) -> Option<bool> {
        self.values(name).iter().find_map(|v| match v {
            Value::Bool(b) => Some(*b),
            _ => None,
        })
    }
}

/// A decoded response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub groups: Vec<Group>,
}

impl Response {
    /// The groups with `tag` (every printer group, every job group).
    pub fn groups(&self, tag: u8) -> impl Iterator<Item = &Group> {
        self.groups.iter().filter(move |g| g.tag == tag)
    }

    /// The server's own words for its status (`status-message`), if it sent them.
    pub fn message(&self) -> Option<&str> {
        self.groups(tag::OPERATION)
            .find_map(|g| g.str("status-message"))
    }
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(n)
            .filter(|e| *e <= self.b.len())
            .ok_or("the printing service sent a cut-off answer")?;
        let s = &self.b[self.at..end];
        self.at = end;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, String> {
        let s = self.take(2)?;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }

    fn sized(&mut self) -> Result<&'a [u8], String> {
        let n = self.u16()?;
        self.take(usize::from(n))
    }
}

fn decode_value(t: u8, raw: &[u8]) -> Value {
    let text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    match t {
        tag::INTEGER | tag::ENUM if raw.len() == 4 => {
            Value::Int(i32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]))
        }
        tag::BOOLEAN if raw.len() == 1 => Value::Bool(raw[0] != 0),
        tag::TEXT_WITH_LANGUAGE | tag::NAME_WITH_LANGUAGE => {
            // A language, then the text, each with its own length.
            let mut r = Reader { b: raw, at: 0 };
            match r.sized().and_then(|_| r.sized()) {
                Ok(s) => Value::Text(text(s)),
                Err(_) => Value::Other,
            }
        }
        0x40..=0x5F => Value::Text(text(raw)),
        _ => Value::Other,
    }
}

pub fn decode(b: &[u8]) -> Result<Response, String> {
    decode_at(b).map(|(r, _)| r)
}

/// A request as a server reads it: its groups (the op id in `status`) and the document after.
#[cfg(test)]
pub fn decode_request(b: &[u8]) -> Result<(Response, Vec<u8>), String> {
    decode_at(b).map(|(r, at)| (r, b[at..].to_vec()))
}

fn decode_at(b: &[u8]) -> Result<(Response, usize), String> {
    let mut r = Reader { b, at: 0 };
    r.take(2)?; // version
    let status = r.u16()?;
    r.take(4)?; // request id
    let mut groups: Vec<Group> = Vec::new();
    loop {
        let t = r.u8()?;
        if t == tag::END {
            break;
        }
        if t < 0x10 {
            groups.push(Group {
                tag: t,
                attrs: Vec::new(),
            });
            continue;
        }
        let name = String::from_utf8_lossy(r.sized()?).into_owned();
        let raw = r.sized()?;
        let value = if t == tag::BEGIN_COLLECTION {
            // Skip the members, nested collections included.
            let mut depth = 1u32;
            while depth > 0 {
                let inner = r.u8()?;
                r.sized()?;
                r.sized()?;
                match inner {
                    tag::BEGIN_COLLECTION => depth += 1,
                    tag::END_COLLECTION => depth -= 1,
                    _ => {}
                }
            }
            Value::Other
        } else {
            decode_value(t, raw)
        };
        let group = groups
            .last_mut()
            .ok_or("the printing service sent an attribute outside any group")?;
        if name.is_empty() {
            // Another value of the attribute before it.
            if let Some(last) = group.attrs.last_mut() {
                last.1.push(value);
            }
        } else {
            group.attrs.push((name, vec![value]));
        }
    }
    Ok((Response { status, groups }, r.at))
}

#[cfg(test)]
mod tests;
