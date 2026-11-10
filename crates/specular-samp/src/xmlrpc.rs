//! The XML-RPC SAMP uses: method calls and responses whose values are strings, lists and
//! maps (SAMP's own types), with XML-RPC's `int`, `i4`, `boolean` and `double` read as
//! their text so that any hub can be understood.

use quick_xml::events::Event;

/// The deepest nesting read.
const MAX_DEPTH: usize = 64;
/// The most nodes in one document.
const MAX_NODES: usize = 1 << 20;

/// A SAMP value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    List(Vec<Value>),
    /// Members in their order.
    Map(Vec<(String, Value)>),
}

impl Default for Value {
    fn default() -> Value {
        Value::Str(String::new())
    }
}

impl Value {
    pub fn str(s: impl Into<String>) -> Value {
        Value::Str(s.into())
    }

    pub fn map<K: Into<String>>(m: impl IntoIterator<Item = (K, Value)>) -> Value {
        Value::Map(m.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }

    /// A map member.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_map()?
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// A map member that is a string.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_str()
    }

    fn write(&self, out: &mut String) {
        out.push_str("<value>");
        match self {
            Value::Str(s) => {
                out.push_str("<string>");
                escape(s, out);
                out.push_str("</string>");
            }
            Value::List(v) => {
                out.push_str("<array><data>");
                for x in v {
                    x.write(out);
                }
                out.push_str("</data></array>");
            }
            Value::Map(m) => {
                out.push_str("<struct>");
                for (k, v) in m {
                    out.push_str("<member><name>");
                    escape(k, out);
                    out.push_str("</name>");
                    v.write(out);
                    out.push_str("</member>");
                }
                out.push_str("</struct>");
            }
        }
        out.push_str("</value>");
    }
}

fn escape(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            // XML 1.0 has no other control characters; SAMP strings should not hold them.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
}

/// A method call's XML.
pub fn call(method: &str, params: &[Value]) -> String {
    let mut out = String::from("<?xml version=\"1.0\"?>\n<methodCall><methodName>");
    escape(method, &mut out);
    out.push_str("</methodName><params>");
    for p in params {
        out.push_str("<param>");
        p.write(&mut out);
        out.push_str("</param>");
    }
    out.push_str("</params></methodCall>\n");
    out
}

/// A successful response's XML (SAMP methods that return nothing give an empty string).
pub fn response(v: &Value) -> String {
    let mut out = String::from("<?xml version=\"1.0\"?>\n<methodResponse><params><param>");
    v.write(&mut out);
    out.push_str("</param></params></methodResponse>\n");
    out
}

/// A fault response's XML.
pub fn fault(code: i32, text: &str) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\"?>\n<methodResponse><fault><value><struct>\
         <member><name>faultCode</name><value><int>",
    );
    out.push_str(&code.to_string());
    out.push_str("</int></value></member><member><name>faultString</name><value><string>");
    escape(text, &mut out);
    out.push_str("</string></value></member></struct></value></fault></methodResponse>\n");
    out
}

/// Why a document was not understood.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Malformed(pub String);

impl std::fmt::Display for Malformed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "malformed XML-RPC: {}", self.0)
    }
}

/// A small element tree.
#[derive(Debug, Default)]
struct El {
    name: String,
    kids: Vec<El>,
    text: String,
}

impl El {
    fn kid(&self, name: &str) -> Option<&El> {
        self.kids.iter().find(|k| k.name == name)
    }
}

fn tree(xml: &[u8]) -> Result<El, Malformed> {
    let mut r = quick_xml::Reader::from_reader(xml);
    let mut stack: Vec<El> = vec![El::default()];
    let mut nodes = 0usize;
    let bad = |e: &dyn std::fmt::Display| Malformed(e.to_string());
    loop {
        match r.read_event().map_err(|e| bad(&e))? {
            Event::Start(e) => {
                nodes += 1;
                if stack.len() > MAX_DEPTH || nodes > MAX_NODES {
                    return Err(Malformed("too deep or too large".into()));
                }
                stack.push(El {
                    name: String::from_utf8_lossy(e.local_name().as_ref()).into_owned(),
                    ..El::default()
                });
            }
            Event::Empty(e) => {
                nodes += 1;
                if nodes > MAX_NODES {
                    return Err(Malformed("too large".into()));
                }
                let el = El {
                    name: String::from_utf8_lossy(e.local_name().as_ref()).into_owned(),
                    ..El::default()
                };
                if let Some(top) = stack.last_mut() {
                    top.kids.push(el);
                }
            }
            Event::End(_) => {
                let el = stack.pop().ok_or_else(|| Malformed("unbalanced".into()))?;
                match stack.last_mut() {
                    Some(top) => top.kids.push(el),
                    None => return Err(Malformed("unbalanced".into())),
                }
            }
            Event::Text(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&String::from_utf8_lossy(&t));
                }
            }
            Event::CData(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&String::from_utf8_lossy(&t));
                }
            }
            Event::GeneralRef(g) => {
                let n = String::from_utf8_lossy(&g);
                let s = quick_xml::escape::unescape(&format!("&{n};"))
                    .map_err(|e| bad(&e))?
                    .into_owned();
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&s);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if stack.len() != 1 {
        return Err(Malformed("unclosed element".into()));
    }
    stack
        .pop()
        .and_then(|mut root| root.kids.pop())
        .ok_or_else(|| Malformed("empty document".into()))
}

/// A `<value>` element.
fn value(v: &El) -> Result<Value, Malformed> {
    let Some(inner) = v.kids.first() else {
        // No type: a string.
        return Ok(Value::Str(v.text.clone()));
    };
    Ok(match inner.name.as_str() {
        "string" => Value::Str(inner.text.clone()),
        "int" | "i4" | "i8" | "double" | "boolean" | "dateTime.iso8601" | "base64" => {
            Value::Str(inner.text.trim().to_owned())
        }
        "array" => {
            let data = inner
                .kid("data")
                .ok_or_else(|| Malformed("array without data".into()))?;
            Value::List(
                data.kids
                    .iter()
                    .filter(|k| k.name == "value")
                    .map(value)
                    .collect::<Result<_, _>>()?,
            )
        }
        "struct" => Value::Map(
            inner
                .kids
                .iter()
                .filter(|k| k.name == "member")
                .map(|m| {
                    let name = m
                        .kid("name")
                        .ok_or_else(|| Malformed("member without a name".into()))?;
                    let v = m
                        .kid("value")
                        .ok_or_else(|| Malformed("member without a value".into()))?;
                    Ok((name.text.clone(), value(v)?))
                })
                .collect::<Result<_, Malformed>>()?,
        ),
        other => return Err(Malformed(format!("value type {other}"))),
    })
}

/// A parsed method call: its name and parameters.
pub fn parse_call(xml: &[u8]) -> Result<(String, Vec<Value>), Malformed> {
    let root = tree(xml)?;
    if root.name != "methodCall" {
        return Err(Malformed(format!("{} instead of methodCall", root.name)));
    }
    let name = root
        .kid("methodName")
        .ok_or_else(|| Malformed("no methodName".into()))?
        .text
        .trim()
        .to_owned();
    let params = match root.kid("params") {
        Some(p) => p
            .kids
            .iter()
            .filter(|k| k.name == "param")
            .map(|p| {
                p.kid("value")
                    .ok_or_else(|| Malformed("param without a value".into()))
                    .and_then(value)
            })
            .collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    Ok((name, params))
}

/// A parsed response: the value, or the fault's text.
pub fn parse_response(xml: &[u8]) -> Result<Result<Value, String>, Malformed> {
    let root = tree(xml)?;
    if root.name != "methodResponse" {
        return Err(Malformed(format!(
            "{} instead of methodResponse",
            root.name
        )));
    }
    if let Some(f) = root.kid("fault") {
        let v = f
            .kid("value")
            .ok_or_else(|| Malformed("fault without a value".into()))
            .and_then(value)?;
        let text = v.get_str("faultString").unwrap_or("fault").to_owned();
        return Ok(Err(text));
    }
    let v = root
        .kid("params")
        .and_then(|p| p.kid("param"))
        .and_then(|p| p.kid("value"))
        .map_or(Ok(Value::Str(String::new())), value)?;
    Ok(Ok(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let v = Value::map([
            ("samp.mtype", Value::str("coord.pointAt.sky")),
            (
                "samp.params",
                Value::map([("ra", Value::str("10.5")), ("dec", Value::str("-3 & <x>"))]),
            ),
            (
                "list",
                Value::List(vec![Value::str("a"), Value::List(vec![])]),
            ),
        ]);
        let xml = call("samp.hub.notifyAll", &[Value::str("key"), v.clone()]);
        let (m, p) = parse_call(xml.as_bytes()).unwrap();
        assert_eq!(m, "samp.hub.notifyAll");
        assert_eq!(p, [Value::str("key"), v.clone()]);
        assert_eq!(parse_response(response(&v).as_bytes()).unwrap(), Ok(v));
        assert_eq!(
            parse_response(fault(1, "no <such> client").as_bytes()).unwrap(),
            Err("no <such> client".into())
        );
    }

    #[test]
    fn other_hubs_values() {
        // Untyped strings, ints, booleans, character references, CDATA, whitespace.
        let xml = br#"<?xml version="1.0"?>
<methodResponse>
  <params>
    <param>
      <value><struct>
        <member><name>a</name><value>plain &amp; simple</value></member>
        <member><name>b</name><value><int>42</int></value></member>
        <member><name>c</name><value><boolean>1</boolean></value></member>
        <member><name>d</name><value><string>&#65;<![CDATA[<b>]]></string></value></member>
        <member><name>e</name><value><array><data></data></array></value></member>
      </struct></value>
    </param>
  </params>
</methodResponse>"#;
        let v = parse_response(xml).unwrap().unwrap();
        assert_eq!(v.get_str("a"), Some("plain & simple"));
        assert_eq!(v.get_str("b"), Some("42"));
        assert_eq!(v.get_str("c"), Some("1"));
        assert_eq!(v.get_str("d"), Some("A<b>"));
        assert_eq!(v.get("e"), Some(&Value::List(vec![])));
        // A response without a value (samp.hub.reply in some hubs).
        assert_eq!(
            parse_response(b"<methodResponse><params/></methodResponse>").unwrap(),
            Ok(Value::str(""))
        );
    }

    #[test]
    fn hostile_documents() {
        let deep = "<value><array><data>".repeat(100);
        for bad in [
            &b""[..],
            b"<methodCall>",
            b"<methodCall><methodName>x</methodName><params><param><value><what/></value></param></params></methodCall>",
            b"<notxml",
            b"<methodResponse><fault/></methodResponse>",
            deep.as_bytes(),
        ] {
            assert!(parse_call(bad).is_err() || parse_response(bad).is_err());
        }
    }
}
