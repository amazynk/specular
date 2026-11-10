//! buildingSMART Information Delivery Specification (IDS 1.0): `.ids` files read, and an
//! IFC model checked against them.
//!
//! An IDS holds specifications. Each one names what it applies to (its applicability: facets
//! every applicable object meets) and what those objects must have (its requirements). The
//! six facets are entity (class and predefined type), attribute, property (in a property or
//! quantity set, with a data type), classification, material and part of (aggregate,
//! group, container, nest, voids and fills). Every value is a simple value or an XML Schema
//! restriction: enumeration, pattern (an [`xsd::Pattern`]), bounds and lengths. A
//! requirement is required, optional (it may be absent but not wrong) or prohibited, and a
//! specification is required (at least one object applies), optional or prohibited (none
//! may).
//!
//! The checker follows the buildingSMART implementers' test cases: exact classes (no
//! subtypes; IFC2X3 occurrences also by their type's class), property values in SI units,
//! real numbers equal within 1e-6 relative plus 1e-6, bounds exact, the occurrence's
//! properties over its type's, materials and classifications from the type when the
//! occurrence has none.

use std::collections::HashMap;

use specular_mesh::ifc::data::{ClassRef, IfcData, Object, PropKind, Value, Version};

use crate::xsd::Pattern;

/// Why an IDS file could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdsError {
    /// Not XML, or not an IDS document.
    NotIds(String),
    /// An IDS that breaks the schema's rules: the element and what is wrong.
    Invalid(String),
}

impl std::fmt::Display for IdsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdsError::NotIds(m) => write!(f, "not an IDS file: {m}"),
            IdsError::Invalid(m) => write!(f, "invalid IDS: {m}"),
        }
    }
}

impl std::error::Error for IdsError {}

/// A value as IDS gives it: a simple value or a restriction.
#[derive(Clone, Debug)]
pub enum IdsValue {
    Simple(String),
    Restriction(Box<Restriction>),
}

/// An `xs:restriction`: every constraint given must hold; enumerations and patterns hold
/// when any one of them does.
#[derive(Clone, Debug, Default)]
pub struct Restriction {
    pub base: Option<String>,
    pub enumeration: Vec<String>,
    pub patterns: Vec<(String, Pattern)>,
    pub min_inclusive: Option<f64>,
    pub max_inclusive: Option<f64>,
    pub min_exclusive: Option<f64>,
    pub max_exclusive: Option<f64>,
    pub length: Option<usize>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
}

/// A required, optional or prohibited facet or specification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Cardinality {
    #[default]
    Required,
    Optional,
    Prohibited,
}

/// How a part-of facet relates the object to the whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    Aggregates,
    Group,
    Container,
    Nests,
    VoidsFills,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FacetKind {
    Entity,
    Attribute,
    Property,
    Classification,
    Material,
    PartOf,
}

#[derive(Clone, Debug)]
pub enum Facet {
    Entity {
        name: IdsValue,
        predefined: Option<IdsValue>,
    },
    Attribute {
        name: IdsValue,
        value: Option<IdsValue>,
        card: Cardinality,
    },
    Property {
        pset: IdsValue,
        name: IdsValue,
        value: Option<IdsValue>,
        data_type: Option<String>,
        card: Cardinality,
    },
    Classification {
        value: Option<IdsValue>,
        system: Option<IdsValue>,
        card: Cardinality,
    },
    Material {
        value: Option<IdsValue>,
        card: Cardinality,
    },
    PartOf {
        relation: Option<Relation>,
        name: IdsValue,
        predefined: Option<IdsValue>,
        card: Cardinality,
    },
}

impl Facet {
    pub fn kind(&self) -> FacetKind {
        match self {
            Facet::Entity { .. } => FacetKind::Entity,
            Facet::Attribute { .. } => FacetKind::Attribute,
            Facet::Property { .. } => FacetKind::Property,
            Facet::Classification { .. } => FacetKind::Classification,
            Facet::Material { .. } => FacetKind::Material,
            Facet::PartOf { .. } => FacetKind::PartOf,
        }
    }

    pub fn cardinality(&self) -> Cardinality {
        match self {
            Facet::Entity { .. } => Cardinality::Required,
            Facet::Attribute { card, .. }
            | Facet::Property { card, .. }
            | Facet::Classification { card, .. }
            | Facet::Material { card, .. }
            | Facet::PartOf { card, .. } => *card,
        }
    }
}

/// One specification.
#[derive(Clone, Debug, Default)]
pub struct Spec {
    pub name: String,
    pub identifier: Option<String>,
    pub description: Option<String>,
    pub instructions: Option<String>,
    /// The IFC versions it is written for (metadata: it never changes the result).
    pub ifc_versions: Vec<String>,
    pub usage: Cardinality,
    pub applicability: Vec<Facet>,
    pub requirements: Vec<Facet>,
}

/// An IDS document.
#[derive(Clone, Debug, Default)]
pub struct Ids {
    pub title: Option<String>,
    pub author: Option<String>,
    pub specs: Vec<Spec>,
}

// --- Reading -------------------------------------------------------------------------

fn invalid<T>(m: impl Into<String>) -> Result<T, IdsError> {
    Err(IdsError::Invalid(m.into()))
}

fn child<'a>(n: roxmltree::Node<'a, 'a>, name: &str) -> Option<roxmltree::Node<'a, 'a>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

fn text_of(n: roxmltree::Node) -> String {
    n.text().unwrap_or("").to_owned()
}

fn int_attr(n: roxmltree::Node, a: &str) -> Result<Option<usize>, IdsError> {
    match n.attribute(a) {
        None => Ok(None),
        Some(v) => v
            .trim()
            .parse()
            .map(Some)
            .or_else(|_| invalid(format!("{a}=\"{v}\" is not a whole number"))),
    }
}

fn num(s: &str) -> Option<f64> {
    if is_xsd_double(s.trim()) {
        s.trim().parse().ok().or(match s.trim() {
            "INF" => Some(f64::INFINITY),
            "-INF" => Some(f64::NEG_INFINITY),
            _ => None,
        })
    } else {
        None
    }
}

fn value_of(n: roxmltree::Node) -> Result<IdsValue, IdsError> {
    if let Some(s) = child(n, "simpleValue") {
        return Ok(IdsValue::Simple(text_of(s)));
    }
    let Some(r) = child(n, "restriction") else {
        return invalid(format!("<{}> has no value", n.tag_name().name()));
    };
    let mut out = Restriction {
        base: r.attribute("base").map(str::to_owned),
        ..Restriction::default()
    };
    for c in r.children().filter(|c| c.is_element()) {
        let v = c.attribute("value").unwrap_or("").to_owned();
        let bound = |v: &str| {
            num(v).map_or_else(
                || invalid(format!("bound \"{v}\" is not a number")),
                |x| Ok(Some(x)),
            )
        };
        let len = |v: &str| {
            v.trim()
                .parse::<usize>()
                .map(Some)
                .or_else(|_| invalid(format!("length \"{v}\" is not a whole number")))
        };
        match c.tag_name().name() {
            "enumeration" => out.enumeration.push(v),
            "pattern" => match Pattern::new(&v) {
                Ok(p) => out.patterns.push((v, p)),
                Err(e) => return invalid(format!("pattern \"{v}\": {}", e.0)),
            },
            "minInclusive" => out.min_inclusive = bound(&v)?,
            "maxInclusive" => out.max_inclusive = bound(&v)?,
            "minExclusive" => out.min_exclusive = bound(&v)?,
            "maxExclusive" => out.max_exclusive = bound(&v)?,
            "length" => out.length = len(&v)?,
            "minLength" => out.min_length = len(&v)?,
            "maxLength" => out.max_length = len(&v)?,
            "annotation" | "totalDigits" | "fractionDigits" | "whiteSpace" => {}
            other => return invalid(format!("unknown restriction <{other}>")),
        }
    }
    Ok(IdsValue::Restriction(Box::new(out)))
}

fn opt_value(n: roxmltree::Node, name: &str) -> Result<Option<IdsValue>, IdsError> {
    child(n, name).map(value_of).transpose()
}

fn cardinality(n: roxmltree::Node, in_applicability: bool) -> Result<Cardinality, IdsError> {
    Ok(match n.attribute("cardinality") {
        None | Some("required") => Cardinality::Required,
        Some("optional") if !in_applicability => Cardinality::Optional,
        Some("prohibited") if !in_applicability => Cardinality::Prohibited,
        Some(v) => return invalid(format!("cardinality \"{v}\"")),
    })
}

/// Entity names must be upper case.
fn check_entity_name(v: &IdsValue) -> Result<(), IdsError> {
    match v {
        IdsValue::Simple(s) if s.chars().any(|c| c.is_lowercase()) => {
            invalid(format!("entity \"{s}\" is not upper case"))
        }
        IdsValue::Restriction(r) => {
            for e in &r.enumeration {
                if e.chars().any(|c| c.is_lowercase()) {
                    return invalid(format!("entity \"{e}\" is not upper case"));
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn facet(n: roxmltree::Node, in_applicability: bool) -> Result<Facet, IdsError> {
    let card = cardinality(n, in_applicability)?;
    Ok(match n.tag_name().name() {
        "entity" => {
            let Some(name) = opt_value(n, "name")? else {
                return invalid("entity facet without a name");
            };
            check_entity_name(&name)?;
            Facet::Entity {
                name,
                predefined: opt_value(n, "predefinedType")?,
            }
        }
        "attribute" => {
            let Some(name) = opt_value(n, "name")? else {
                return invalid("attribute facet without a name");
            };
            Facet::Attribute {
                name,
                value: opt_value(n, "value")?,
                card,
            }
        }
        "property" => {
            let (Some(pset), Some(name)) =
                (opt_value(n, "propertySet")?, opt_value(n, "baseName")?)
            else {
                return invalid("property facet without a property set or name");
            };
            Facet::Property {
                pset,
                name,
                value: opt_value(n, "value")?,
                data_type: n.attribute("dataType").map(|s| s.to_ascii_uppercase()),
                card,
            }
        }
        "classification" => Facet::Classification {
            value: opt_value(n, "value")?,
            system: opt_value(n, "system")?,
            card,
        },
        "material" => Facet::Material {
            value: opt_value(n, "value")?,
            card,
        },
        "partOf" => {
            let relation = match n.attribute("relation") {
                None => None,
                Some("IFCRELAGGREGATES") => Some(Relation::Aggregates),
                Some("IFCRELASSIGNSTOGROUP") => Some(Relation::Group),
                Some("IFCRELCONTAINEDINSPATIALSTRUCTURE") => Some(Relation::Container),
                Some("IFCRELNESTS") => Some(Relation::Nests),
                Some("IFCRELVOIDSELEMENT IFCRELFILLSELEMENT") => Some(Relation::VoidsFills),
                Some(r) => return invalid(format!("partOf relation \"{r}\"")),
            };
            let Some(e) = child(n, "entity") else {
                return invalid("partOf facet without an entity");
            };
            let Some(name) = opt_value(e, "name")? else {
                return invalid("partOf entity without a name");
            };
            check_entity_name(&name)?;
            Facet::PartOf {
                relation,
                name,
                predefined: opt_value(e, "predefinedType")?,
                card,
            }
        }
        other => return invalid(format!("unknown facet <{other}>")),
    })
}

/// Reads an IDS document.
pub fn parse(xml: &str) -> Result<Ids, IdsError> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| IdsError::NotIds(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "ids" {
        return Err(IdsError::NotIds(format!(
            "the root element is <{}>",
            root.tag_name().name()
        )));
    }
    let mut ids = Ids::default();
    if let Some(info) = child(root, "info") {
        ids.title = child(info, "title").map(text_of);
        ids.author = child(info, "author").map(text_of);
    }
    let Some(specs) = child(root, "specifications") else {
        return invalid("no <specifications>");
    };
    for s in specs.children().filter(|c| c.is_element()) {
        if s.tag_name().name() != "specification" {
            return invalid(format!("unknown element <{}>", s.tag_name().name()));
        }
        let mut spec = Spec {
            name: s.attribute("name").unwrap_or("").to_owned(),
            identifier: s.attribute("identifier").map(str::to_owned),
            description: s.attribute("description").map(str::to_owned),
            instructions: s.attribute("instructions").map(str::to_owned),
            ifc_versions: s
                .attribute("ifcVersion")
                .unwrap_or("")
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
            ..Spec::default()
        };
        let Some(app) = child(s, "applicability") else {
            return invalid(format!(
                "specification \"{}\" has no applicability",
                spec.name
            ));
        };
        let min = int_attr(app, "minOccurs")?.unwrap_or(1);
        let max = match app.attribute("maxOccurs") {
            None | Some("unbounded") => None,
            Some(_) => int_attr(app, "maxOccurs")?,
        };
        spec.usage = if max == Some(0) {
            Cardinality::Prohibited
        } else if min == 0 {
            Cardinality::Optional
        } else {
            Cardinality::Required
        };
        for f in app.children().filter(|c| c.is_element()) {
            spec.applicability.push(facet(f, true)?);
        }
        if spec.applicability.is_empty() {
            return invalid(format!(
                "specification \"{}\" applies to nothing",
                spec.name
            ));
        }
        if let Some(req) = child(s, "requirements") {
            for f in req.children().filter(|c| c.is_element()) {
                spec.requirements.push(facet(f, false)?);
            }
        }
        if spec.usage == Cardinality::Prohibited && !spec.requirements.is_empty() {
            return invalid(format!(
                "prohibited specification \"{}\" has requirements",
                spec.name
            ));
        }
        ids.specs.push(spec);
    }
    Ok(ids)
}

// --- Values --------------------------------------------------------------------------

/// XSD's double lexical form (also what decimal and integer accept).
fn is_xsd_double(s: &str) -> bool {
    if matches!(s, "INF" | "-INF" | "NaN") {
        return true;
    }
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let d0 = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i > d0;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let f0 = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        digits |= i > f0;
    }
    if !digits {
        return false;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let e0 = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == e0 {
            return false;
        }
    }
    i == b.len()
}

fn is_xsd_integer(s: &str) -> bool {
    let t = s.strip_prefix(['+', '-']).unwrap_or(s);
    !t.is_empty() && t.bytes().all(|c| c.is_ascii_digit())
}

/// A value as IDS compares it.
#[derive(Clone, Debug, PartialEq)]
enum Actual {
    Str(String),
    Int(i64),
    Real(f64),
    Bool(bool),
}

impl Actual {
    fn of(v: &Value) -> Option<Actual> {
        Some(match v.plain() {
            Value::Str(s) | Value::Enum(s) => Actual::Str(s.clone()),
            Value::Int(i) => Actual::Int(*i),
            Value::Real(r) => Actual::Real(*r),
            Value::Bool(b) | Value::Logical(Some(b)) => Actual::Bool(*b),
            _ => return None,
        })
    }

    fn number(&self) -> Option<f64> {
        match self {
            Actual::Int(i) => Some(*i as f64),
            Actual::Real(r) => Some(*r),
            _ => None,
        }
    }
}

/// Real numbers are equal within 1e-6 of the expected value's size, plus 1e-6.
fn near(actual: f64, expected: f64) -> bool {
    let tol = expected.abs() * 1e-6 + 1e-6;
    // A hair over, for the rounding of the tolerance itself.
    (actual - expected).abs() <= tol * (1.0 + 1e-9)
}

/// Whether the simple value `s` equals `a`.
fn simple_eq(s: &str, a: &Actual) -> bool {
    match a {
        Actual::Str(x) => x == s,
        Actual::Int(i) => is_xsd_integer(s.trim()) && s.trim().parse::<i64>().ok() == Some(*i),
        Actual::Real(r) => num(s).is_some_and(|e| near(*r, e)),
        Actual::Bool(b) => match s {
            "true" | "1" => *b,
            "false" | "0" => !*b,
            _ => false,
        },
    }
}

impl IdsValue {
    fn matches(&self, a: &Actual) -> bool {
        match self {
            IdsValue::Simple(s) => simple_eq(s, a),
            IdsValue::Restriction(r) => r.matches(a),
        }
    }

    fn matches_str(&self, s: &str) -> bool {
        self.matches(&Actual::Str(s.to_owned()))
    }

    /// The value as one line ("IFCWALL", "one of A, B", "pattern Foo.*").
    pub fn describe(&self) -> String {
        match self {
            IdsValue::Simple(s) => s.clone(),
            IdsValue::Restriction(r) => {
                let mut parts = Vec::new();
                if !r.enumeration.is_empty() {
                    parts.push(r.enumeration.join(" | "));
                }
                for (p, _) in &r.patterns {
                    parts.push(format!("/{p}/"));
                }
                let n = |x: f64| x.to_string();
                match (r.min_inclusive, r.min_exclusive) {
                    (Some(a), _) => parts.push(format!("≥ {}", n(a))),
                    (_, Some(a)) => parts.push(format!("> {}", n(a))),
                    _ => {}
                }
                match (r.max_inclusive, r.max_exclusive) {
                    (Some(a), _) => parts.push(format!("≤ {}", n(a))),
                    (_, Some(a)) => parts.push(format!("< {}", n(a))),
                    _ => {}
                }
                if let Some(l) = r.length {
                    parts.push(format!("len = {l}"));
                }
                if let Some(l) = r.min_length {
                    parts.push(format!("len ≥ {l}"));
                }
                if let Some(l) = r.max_length {
                    parts.push(format!("len ≤ {l}"));
                }
                parts.join(", ")
            }
        }
    }
}

impl Restriction {
    fn matches(&self, a: &Actual) -> bool {
        if !self.enumeration.is_empty() && !self.enumeration.iter().any(|e| simple_eq(e, a)) {
            return false;
        }
        if !self.patterns.is_empty() {
            let Actual::Str(s) = a else { return false };
            if !self.patterns.iter().any(|(_, p)| p.matches(s)) {
                return false;
            }
        }
        let bounded = self.min_inclusive.is_some()
            || self.max_inclusive.is_some()
            || self.min_exclusive.is_some()
            || self.max_exclusive.is_some();
        if bounded {
            let Some(x) = a.number() else { return false };
            if self.min_inclusive.is_some_and(|b| x < b)
                || self.max_inclusive.is_some_and(|b| x > b)
                || self.min_exclusive.is_some_and(|b| x <= b)
                || self.max_exclusive.is_some_and(|b| x >= b)
            {
                return false;
            }
        }
        if self.length.is_some() || self.min_length.is_some() || self.max_length.is_some() {
            let Actual::Str(s) = a else { return false };
            let n = s.chars().count();
            if self.length.is_some_and(|l| n != l)
                || self.min_length.is_some_and(|l| n < l)
                || self.max_length.is_some_and(|l| n > l)
            {
                return false;
            }
        }
        true
    }
}

// --- Checking ------------------------------------------------------------------------

/// Why a facet failed for an object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// The class is not the one wanted.
    Class,
    PredefinedType,
    /// No such attribute, property set, property, material, classification or relation.
    Missing,
    /// It is there but empty (null, empty text or list, unknown logical).
    Empty,
    /// Its value does not match.
    Value,
    /// The property's data type is not the one wanted.
    DataType,
    /// The classification system does not match.
    System,
    /// The whole it is part of is not the one wanted.
    Whole,
    /// A prohibited facet was met.
    Prohibited,
    /// A kind of property IDS cannot check (references, complex properties).
    Unsupported,
}

/// One failed requirement.
#[derive(Clone, Debug, PartialEq)]
pub struct Reason {
    /// Which requirement (index into the specification's requirements).
    pub requirement: usize,
    pub facet: FacetKind,
    pub why: Why,
    /// What the object has, when that helps ("IFCSLAB", "2HR").
    pub actual: Option<String>,
}

/// An object that failed a specification.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    /// The object's instance number.
    pub id: u32,
    pub global_id: String,
    pub reasons: Vec<Reason>,
}

/// A specification's result.
#[derive(Clone, Debug, PartialEq)]
pub struct SpecResult {
    /// Index into [`Ids::specs`].
    pub spec: usize,
    pub applicable: usize,
    pub passed: usize,
    pub failed: Vec<Failure>,
    /// It holds: every applicable object passes, at least one applies when it is required,
    /// none applies when it is prohibited.
    pub ok: bool,
}

/// An IDS checked against a model.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Report {
    pub specs: Vec<SpecResult>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.specs.iter().all(|s| s.ok)
    }
}

struct Checker<'a> {
    d: &'a IfcData,
}

/// A facet's outcome: pass, or fail with why and what was found.
type Out = Result<(), (Why, Option<String>)>;

fn fail(why: Why, actual: Option<String>) -> Out {
    Err((why, actual))
}

/// The value `card` asks for: prohibited inverts, optional passes when nothing is there.
fn by_card(card: Cardinality, present: bool, r: Out) -> Out {
    match card {
        Cardinality::Required => r,
        Cardinality::Optional if !present => Ok(()),
        Cardinality::Optional => r,
        Cardinality::Prohibited => match r {
            Ok(()) => fail(Why::Prohibited, None),
            Err(_) => Ok(()),
        },
    }
}

impl Checker<'_> {
    fn class_matches(&self, o: &Object, name: &IdsValue) -> bool {
        if name.matches_str(&o.class) {
            return true;
        }
        // IFC2X3 had no class for many kinds of element: an air terminal is an
        // IfcFlowTerminal typed by an IfcAirTerminalType.
        if self.d.version() == Version::Ifc2x3
            && let Some(t) = self.d.type_of(o)
        {
            let c = t.class.trim_end_matches("TYPE").trim_end_matches("STYLE");
            let c = if c.len() < t.class.len() { c } else { "" };
            return !c.is_empty() && name.matches_str(c);
        }
        false
    }

    fn predefined_matches(&self, o: &Object, p: &IdsValue) -> Out {
        if let IdsValue::Simple(s) = p
            && s == "USERDEFINED"
        {
            return if self.d.is_user_defined(o) {
                Ok(())
            } else {
                fail(Why::PredefinedType, self.d.predefined_type(o))
            };
        }
        match self.d.predefined_type(o) {
            Some(t) if p.matches_str(&t) => Ok(()),
            t => fail(Why::PredefinedType, t),
        }
    }

    fn entity(&self, o: &Object, name: &IdsValue, predefined: Option<&IdsValue>) -> Out {
        if !self.class_matches(o, name) {
            return fail(Why::Class, Some(o.class.clone()));
        }
        match predefined {
            Some(p) => self.predefined_matches(o, p),
            None => Ok(()),
        }
    }

    fn attribute(
        &self,
        o: &Object,
        name: &IdsValue,
        value: Option<&IdsValue>,
        card: Cardinality,
    ) -> Out {
        let found: Vec<&(String, Value)> = o
            .attrs
            .iter()
            .filter(|(n, _)| name.matches_str(n))
            .collect();
        // Null counts as absent; empty text, empty lists and unknown logicals as empty.
        let present: Vec<&Value> = found
            .iter()
            .map(|(_, v)| v)
            .filter(|v| !matches!(v.plain(), Value::Null | Value::Derived))
            .collect();
        let r = (|| {
            if present.is_empty() {
                return fail(Why::Missing, None);
            }
            if let Some(e) = present.iter().find(|v| v.is_empty()) {
                return fail(Why::Empty, Some(e.text()));
            }
            if let Some(want) = value {
                for v in &present {
                    // Objects, lists and select values are never equal to a value.
                    if matches!(v, Value::Typed(..)) {
                        return fail(Why::Value, Some(v.text()));
                    }
                    match Actual::of(v) {
                        Some(a) if want.matches(&a) => {}
                        _ => return fail(Why::Value, Some(v.text())),
                    }
                }
            }
            Ok(())
        })();
        by_card(card, !present.is_empty(), r)
    }

    fn property(
        &self,
        o: &Object,
        pset: &IdsValue,
        name: &IdsValue,
        value: Option<&IdsValue>,
        data_type: Option<&str>,
        card: Cardinality,
    ) -> Out {
        let psets: Vec<_> = self
            .d
            .psets(o)
            .into_iter()
            .filter(|p| pset.matches_str(&p.name))
            .collect();
        if psets.is_empty() {
            return by_card(card, false, fail(Why::Missing, None));
        }
        let mut any = false;
        let r = (|| {
            for ps in &psets {
                let props: Vec<_> = ps
                    .props
                    .iter()
                    .filter(|p| name.matches_str(&p.name))
                    .filter(|p| {
                        // Null, empty and unknown values count as absent.
                        let vals: Vec<&Value> = p.groups.iter().flat_map(|g| &g.values).collect();
                        match p.kind {
                            PropKind::Single | PropKind::Predefined => {
                                vals.first().is_some_and(|v| !v.is_empty())
                            }
                            _ => true,
                        }
                    })
                    .collect();
                if props.is_empty() {
                    return fail(Why::Missing, None);
                }
                any = true;
                for p in props {
                    let dt = p.data_type.as_deref();
                    let mut values = p.si_values();
                    match p.kind {
                        PropKind::Single | PropKind::Quantity => {}
                        PropKind::Predefined => {}
                        PropKind::Enumerated | PropKind::List | PropKind::Bounded => {
                            if values.is_empty() {
                                return fail(Why::Empty, None);
                            }
                        }
                        PropKind::Table => {
                            // The columns of the wanted data type.
                            values = Vec::new();
                            for g in &p.groups {
                                let gdt = g.values.first().and_then(Value::type_name);
                                if data_type
                                    .is_some_and(|w| gdt.is_some_and(|t| t.eq_ignore_ascii_case(w)))
                                {
                                    let tmp = specular_mesh::ifc::data::Property {
                                        groups: vec![g.clone()],
                                        ..p.clone()
                                    };
                                    values.extend(tmp.si_values());
                                }
                            }
                            if values.is_empty() {
                                return fail(Why::DataType, dt.map(str::to_owned));
                            }
                        }
                        PropKind::Reference | PropKind::Complex | PropKind::Other => {
                            return fail(Why::Unsupported, None);
                        }
                    }
                    if p.kind != PropKind::Predefined
                        && p.kind != PropKind::Table
                        && let Some(w) = data_type
                        && !dt.is_some_and(|t| t.eq_ignore_ascii_case(w))
                    {
                        return fail(Why::DataType, dt.map(str::to_owned));
                    }
                    if let Some(want) = value {
                        let ok = values
                            .iter()
                            .filter_map(Actual::of)
                            .any(|a| want.matches(&a));
                        if !ok {
                            let shown = values
                                .iter()
                                .map(Value::text)
                                .collect::<Vec<_>>()
                                .join(", ");
                            return fail(Why::Value, Some(shown));
                        }
                    }
                }
            }
            Ok(())
        })();
        by_card(card, any || card != Cardinality::Optional, r)
    }

    fn classification(
        &self,
        o: &Object,
        value: Option<&IdsValue>,
        system: Option<&IdsValue>,
        card: Cardinality,
    ) -> Out {
        let refs: Vec<&ClassRef> = self.d.classifications(o);
        let r = (|| {
            if refs.is_empty() {
                return fail(Why::Missing, None);
            }
            if let Some(v) = value
                && !refs
                    .iter()
                    .any(|r| r.codes.iter().any(|c| v.matches_str(c)))
            {
                let codes: Vec<&str> = refs
                    .iter()
                    .flat_map(|r| r.codes.iter().map(String::as_str))
                    .collect();
                return fail(Why::Value, Some(codes.join(", ")));
            }
            if let Some(s) = system
                && !refs
                    .iter()
                    .any(|r| r.system.as_deref().is_some_and(|x| s.matches_str(x)))
            {
                let sys: Vec<&str> = refs.iter().filter_map(|r| r.system.as_deref()).collect();
                return fail(Why::System, Some(sys.join(", ")));
            }
            Ok(())
        })();
        by_card(card, !refs.is_empty(), r)
    }

    fn material(&self, o: &Object, value: Option<&IdsValue>, card: Cardinality) -> Out {
        let m = self.d.material(o);
        let r = (|| {
            let Some(m) = m else {
                return fail(Why::Missing, None);
            };
            if let Some(v) = value {
                let vals = m.values();
                if !vals.iter().any(|x| v.matches_str(x)) {
                    return fail(Why::Value, Some(vals.join(", ")));
                }
            }
            Ok(())
        })();
        by_card(card, m.is_some(), r)
    }

    fn whole_matches(&self, w: &Object, name: &IdsValue, predefined: Option<&IdsValue>) -> bool {
        name.matches_str(&w.class)
            && predefined.is_none_or(|p| self.predefined_matches(w, p).is_ok())
    }

    /// Walks up from `o` by `next`, passing when a whole matches.
    fn walk(
        &self,
        o: &Object,
        next: impl Fn(&Object) -> Option<u32>,
        name: &IdsValue,
        predefined: Option<&IdsValue>,
    ) -> Out {
        let mut cur = next(o).and_then(|i| self.d.get(i));
        if cur.is_none() {
            return fail(Why::Missing, None);
        }
        let mut seen = Vec::new();
        let mut n = 0;
        while let Some(w) = cur {
            n += 1;
            if n > 256 {
                break;
            }
            seen.push(w.class.clone());
            if self.whole_matches(w, name, predefined) {
                return Ok(());
            }
            cur = next(w).and_then(|i| self.d.get(i));
        }
        fail(Why::Whole, Some(seen.join(" → ")))
    }

    /// The container: the object's own, or that of a whole it is part of.
    fn container(&self, o: &Object) -> Option<u32> {
        self.d.container(o).map(|c| c.id)
    }

    fn part_of(
        &self,
        o: &Object,
        relation: Option<Relation>,
        name: &IdsValue,
        predefined: Option<&IdsValue>,
        card: Cardinality,
    ) -> Out {
        let one = |w: Option<u32>| -> Out {
            let Some(w) = w.and_then(|w| self.d.get(w)) else {
                return fail(Why::Missing, None);
            };
            if self.whole_matches(w, name, predefined) {
                Ok(())
            } else {
                fail(Why::Whole, Some(w.class.clone()))
            }
        };
        let r = match relation {
            None => self.walk(o, |x| self.d.parent(x).map(|p| p.id), name, predefined),
            Some(Relation::Aggregates) => self.walk(o, |x| x.aggregate, name, predefined),
            Some(Relation::Nests) => self.walk(o, |x| x.nest, name, predefined),
            Some(Relation::Group) => {
                if o.groups.is_empty() {
                    fail(Why::Missing, None)
                } else if o.groups.iter().any(|&g| one(Some(g)).is_ok()) {
                    Ok(())
                } else {
                    one(o.groups.first().copied())
                }
            }
            Some(Relation::Container) => one(self.container(o)),
            Some(Relation::VoidsFills) => {
                let element = if o.voids.is_some() {
                    o.voids
                } else {
                    o.fills
                        .and_then(|op| self.d.get(op))
                        .and_then(|op| op.voids)
                };
                one(element)
            }
        };
        let present = !matches!(r, Err((Why::Missing, _)));
        by_card(card, present, r)
    }

    fn facet(&self, o: &Object, f: &Facet) -> Out {
        match f {
            Facet::Entity { name, predefined } => self.entity(o, name, predefined.as_ref()),
            Facet::Attribute { name, value, card } => {
                self.attribute(o, name, value.as_ref(), *card)
            }
            Facet::Property {
                pset,
                name,
                value,
                data_type,
                card,
            } => self.property(o, pset, name, value.as_ref(), data_type.as_deref(), *card),
            Facet::Classification {
                value,
                system,
                card,
            } => self.classification(o, value.as_ref(), system.as_ref(), *card),
            Facet::Material { value, card } => self.material(o, value.as_ref(), *card),
            Facet::PartOf {
                relation,
                name,
                predefined,
                card,
            } => self.part_of(o, *relation, name, predefined.as_ref(), *card),
        }
    }
}

/// Checks `data` against every specification of `ids`.
pub fn check(ids: &Ids, data: &IfcData) -> Report {
    let c = Checker { d: data };
    let mut report = Report::default();
    for (si, spec) in ids.specs.iter().enumerate() {
        let mut res = SpecResult {
            spec: si,
            applicable: 0,
            passed: 0,
            failed: Vec::new(),
            ok: true,
        };
        for o in &data.objects {
            if !spec.applicability.iter().all(|f| c.facet(o, f).is_ok()) {
                continue;
            }
            res.applicable += 1;
            if spec.usage == Cardinality::Prohibited {
                continue;
            }
            let mut reasons = Vec::new();
            for (ri, f) in spec.requirements.iter().enumerate() {
                if let Err((why, actual)) = c.facet(o, f) {
                    reasons.push(Reason {
                        requirement: ri,
                        facet: f.kind(),
                        why,
                        actual,
                    });
                }
            }
            if reasons.is_empty() {
                res.passed += 1;
            } else {
                res.failed.push(Failure {
                    id: o.id,
                    global_id: o.global_id.clone(),
                    reasons,
                });
            }
        }
        res.ok = match spec.usage {
            Cardinality::Prohibited => res.applicable == 0,
            Cardinality::Required => res.applicable > 0 && res.failed.is_empty(),
            Cardinality::Optional => res.failed.is_empty(),
        };
        report.specs.push(res);
    }
    report
}

/// How many objects of each class a report's failures name (for a summary).
pub fn failures_by_class(report: &Report, data: &IfcData) -> HashMap<String, usize> {
    let mut out = HashMap::new();
    for s in &report.specs {
        for f in &s.failed {
            if let Some(o) = data.get(f.id) {
                *out.entry(o.class.clone()).or_insert(0) += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_values_compare_as_ids_says() {
        assert!(is_xsd_double("42.3"));
        assert!(is_xsd_double("-1e3"));
        assert!(is_xsd_double(".5"));
        assert!(!is_xsd_double("42,3"));
        assert!(!is_xsd_double("1e"));
        assert!(simple_eq("42", &Actual::Int(42)));
        assert!(!simple_eq("42.0", &Actual::Int(42)));
        assert!(simple_eq("42", &Actual::Real(42.0)));
        assert!(simple_eq("-1000000.", &Actual::Real(-1_000_001.000_001)));
        assert!(!simple_eq("-1000000.", &Actual::Real(-1_000_001.000_001_1)));
        assert!(simple_eq("true", &Actual::Bool(true)));
        assert!(!simple_eq("TRUE", &Actual::Bool(true)));
        assert!(!simple_eq("Foo", &Actual::Str("foo".into())));
    }
}
