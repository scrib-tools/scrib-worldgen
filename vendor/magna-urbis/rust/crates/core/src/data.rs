//! The data documents of the generator (cultures, presets, services, styles, ...), one
//! registry for all of them. Each document is the parsed JSON of its built-in file under a
//! stable key (`DOCUMENTS`); a reader takes `doc(key)` and keeps its own typed view.
//!
//! An application may change the documents once, before the first read, with overlay
//! packs (`install_packs`). A pack is `{ "<document key>": <patch>, ... }`; packs apply in
//! order. A patch is a JSON Merge Patch (RFC 7396: objects merge recursively, `null`
//! removes a key, any other value replaces) with two directives, read in object position:
//! - `{"$from": "<JSON pointer>", ...rest}`: a deep copy of that node of the same
//!   document (as it stands before this patch: built-in plus earlier packs), then `rest`
//!   merged onto it ("like X, with these changes");
//! - `{"$append": [...]}` on an array: appends the items it does not already hold.
//!
//! Any other `$` key is an error. Arrays and scalars of a patch are taken literally.
//! Without `install_packs` the built-in documents are used as they are.
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::OnceLock;

/// Every document: stable key, built-in text.
const DOCUMENTS: &[(&str, &str)] = &[
    ("catalogue", include_str!("urban/catalogue.json")),
    ("services", include_str!("urban/services.json")),
    ("activities", include_str!("urban/activities.json")),
    ("enclosures", include_str!("urban/enclosures.json")),
    ("harbours", include_str!("urban/harbours.json")),
    ("styles", include_str!("urban/forms/styles.json")),
    ("styles_modern", include_str!("urban/forms/styles_modern.json")),
    ("roles_core", include_str!("urban/forms/roles_core.json")),
    ("roles_monuments_a", include_str!("urban/forms/roles_monuments_a.json")),
    ("roles_monuments_b", include_str!("urban/forms/roles_monuments_b.json")),
    ("roles_rural", include_str!("urban/forms/roles_rural.json")),
    ("roles_modern", include_str!("urban/forms/roles_modern.json")),
    ("fortifications", include_str!("urban/fortifications.json")),
    ("urban_admission", include_str!("../../houses/data/urban-admission.json")),
    ("programme_tokens", include_str!("urban/programme_tokens.json")),
    ("spring_water", include_str!("urban/spring_water.json")),
    ("tunnelling", include_str!("settlements/roads/tunnelling.json")),
    ("demographic_classes", include_str!("settlements/demographic_classes.json")),
    ("native_witnesses", include_str!("urban/native_witnesses.json")),
    ("functional_source_coverage", include_str!("urban/functional-source-coverage.json")),
    ("culture_mappings", include_str!("urban/culture_mappings.json")),
];

/// Why packs were refused. Nothing is installed when any pack fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataError {
    /// A document was read, or packs were installed, before this call.
    AlreadyRead,
    /// A pack is not a JSON object.
    PackNotObject { pack: usize },
    /// A pack names a document the registry does not hold.
    UnknownDocument { pack: usize, key: String },
    /// A document's patch is not an object (a document stays an object).
    PatchNotObject { pack: usize, key: String },
    /// A `$` key other than `$from` / `$append`, or a directive in the wrong shape.
    BadDirective { pack: usize, key: String, at: String, reason: String },
    /// `$from` points at no node of the document.
    MissingSource { pack: usize, key: String, at: String, pointer: String },
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRead => write!(f, "data packs must be installed once, before the first read"),
            Self::PackNotObject { pack } => write!(f, "pack {pack}: not a JSON object"),
            Self::UnknownDocument { pack, key } => write!(f, "pack {pack}: unknown document `{key}`"),
            Self::PatchNotObject { pack, key } => write!(f, "pack {pack}: patch of `{key}` is not an object"),
            Self::BadDirective { pack, key, at, reason } => write!(f, "pack {pack}: {key}{at}: {reason}"),
            Self::MissingSource { pack, key, at, pointer } => {
                write!(f, "pack {pack}: {key}{at}: `$from` {pointer:?} points at nothing")
            }
        }
    }
}

impl std::error::Error for DataError {}

/// The patched documents, set once: by `install_packs`, else empty at the first read.
static INSTALLED: OnceLock<BTreeMap<&'static str, Value>> = OnceLock::new();

fn index(key: &str) -> Option<usize> {
    DOCUMENTS.iter().position(|(k, _)| *k == key)
}

fn builtin(i: usize) -> Value {
    let (key, text) = DOCUMENTS[i];
    serde_json::from_str(text).unwrap_or_else(|e| panic!("built-in data document `{key}`: {e}"))
}

/// The document under `key`. The first read fixes the registry. Panics on an unknown key
/// (a programming error: keys are this module's).
pub(crate) fn doc(key: &str) -> &'static Value {
    const CELL: OnceLock<Value> = OnceLock::new();
    static CELLS: [OnceLock<Value>; DOCUMENTS.len()] = [CELL; DOCUMENTS.len()];
    let i = index(key).unwrap_or_else(|| panic!("unknown data document `{key}`"));
    CELLS[i].get_or_init(|| {
        let installed = INSTALLED.get_or_init(BTreeMap::new);
        installed.get(DOCUMENTS[i].0).cloned().unwrap_or_else(|| builtin(i))
    })
}

/// A typed view of a document (`T::deserialize` of `doc(key)`).
pub(crate) fn typed<T: serde::de::DeserializeOwned>(key: &str) -> Result<T, serde_json::Error> {
    T::deserialize(doc(key))
}

/// Merges `packs` onto the built-in documents, in order. Call once, before anything reads
/// the data; afterwards (or on a second call) it returns `AlreadyRead` and changes nothing.
pub fn install_packs(packs: &[Value]) -> Result<(), DataError> {
    if INSTALLED.get().is_some() {
        return Err(DataError::AlreadyRead);
    }
    let merged = merge_packs(packs)?;
    INSTALLED.set(merged).map_err(|_| DataError::AlreadyRead)
}

/// The documents `packs` change, after merging them onto the built-in ones (pure).
pub(crate) fn merge_packs(packs: &[Value]) -> Result<BTreeMap<&'static str, Value>, DataError> {
    let mut docs: BTreeMap<&'static str, Value> = BTreeMap::new();
    for (pack, value) in packs.iter().enumerate() {
        let Some(patches) = value.as_object() else { return Err(DataError::PackNotObject { pack }) };
        for (key, patch) in patches {
            let Some(i) = index(key) else {
                return Err(DataError::UnknownDocument { pack, key: key.clone() });
            };
            if !patch.is_object() {
                return Err(DataError::PatchNotObject { pack, key: key.clone() });
            }
            let doc = docs.entry(DOCUMENTS[i].0).or_insert_with(|| builtin(i));
            let base = doc.clone();
            let ctx = Ctx { pack, key, base: &base };
            merge(&ctx, doc, patch, &mut String::new())?;
        }
    }
    Ok(docs)
}

struct Ctx<'a> {
    pack: usize,
    key: &'a str,
    base: &'a Value,
}

impl Ctx<'_> {
    fn bad(&self, at: &str, reason: impl Into<String>) -> DataError {
        DataError::BadDirective { pack: self.pack, key: self.key.into(), at: at.into(), reason: reason.into() }
    }
}

/// Applies `patch` to `target` (at pointer `at`, for errors).
fn merge(ctx: &Ctx, target: &mut Value, patch: &Value, at: &mut String) -> Result<(), DataError> {
    let Some(obj) = patch.as_object() else {
        *target = patch.clone();
        return Ok(());
    };
    for k in obj.keys().filter(|k| k.starts_with('$')) {
        if k != "$from" && k != "$append" {
            return Err(ctx.bad(at, format!("unknown directive `{k}`")));
        }
    }
    if let Some(items) = obj.get("$append") {
        if obj.len() != 1 {
            return Err(ctx.bad(at, "`$append` stands alone"));
        }
        let Some(items) = items.as_array() else { return Err(ctx.bad(at, "`$append` takes an array")) };
        if target.is_null() {
            *target = Value::Array(Vec::new());
        }
        let Some(list) = target.as_array_mut() else {
            return Err(ctx.bad(at, "`$append` on a value that is not an array"));
        };
        for item in items {
            if !list.contains(item) {
                list.push(item.clone());
            }
        }
        return Ok(());
    }
    if let Some(pointer) = obj.get("$from") {
        let Some(pointer) = pointer.as_str() else { return Err(ctx.bad(at, "`$from` takes a JSON pointer string")) };
        let Some(source) = ctx.base.pointer(pointer) else {
            return Err(DataError::MissingSource {
                pack: ctx.pack,
                key: ctx.key.into(),
                at: at.clone(),
                pointer: pointer.into(),
            });
        };
        *target = source.clone();
    }
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    let map = target.as_object_mut().expect("object");
    for (k, v) in obj.iter().filter(|(k, _)| *k != "$from") {
        if v.is_null() {
            map.remove(k);
            continue;
        }
        let len = at.len();
        at.push('/');
        at.push_str(&k.replace('~', "~0").replace('/', "~1"));
        merge(ctx, map.entry(k.clone()).or_insert(Value::Null), v, at)?;
        at.truncate(len);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc_after(packs: &[Value], key: &str) -> Result<Value, DataError> {
        let mut docs = merge_packs(packs)?;
        Ok(docs.remove(key).unwrap_or_else(|| builtin(index(key).unwrap())))
    }

    #[test]
    fn data_every_document_parses_and_keys_are_unique() {
        for (i, (key, _)) in DOCUMENTS.iter().enumerate() {
            assert!(builtin(i).is_object(), "{key}");
            assert_eq!(index(key), Some(i), "duplicate key {key}");
        }
    }

    /// The farmstead readers used to cut the first `"farmstead"` block out of the
    /// catalogue text; it is the top-level `farmstead` table.
    #[test]
    fn data_farmstead_block_is_the_top_level_table() {
        let s = include_str!("urban/catalogue.json");
        let start = s.find("\"farmstead\"").and_then(|k| s[k..].find('{').map(|i| k + i)).unwrap();
        let mut depth = 0;
        let end = s[start..]
            .char_indices()
            .find_map(|(i, c)| {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(start + i + 1);
                        }
                    }
                    _ => {}
                }
                None
            })
            .unwrap();
        let cut: Value = serde_json::from_str(&s[start..end]).unwrap();
        assert_eq!(cut, doc("catalogue")["farmstead"]);
    }

    #[test]
    fn data_merge_replaces_merges_and_removes() {
        let pack = json!({"tunnelling": {"maxLength": 9.5, "cost": {"new-culture": 2.0}, "perMap": null}});
        let d = doc_after(&[pack], "tunnelling").unwrap();
        let base = builtin(index("tunnelling").unwrap());
        assert_eq!(d["maxLength"], json!(9.5));
        assert_eq!(d["cost"]["new-culture"], json!(2.0));
        assert!(d.get("perMap").is_none());
        // untouched keys stay
        assert_eq!(d["maxGrade"], base["maxGrade"]);
        let n = base["cost"].as_object().unwrap().len();
        assert_eq!(d["cost"].as_object().unwrap().len(), n + 1);
        // arrays replace
        let pack = json!({"spring_water": {"default": ["a"]}});
        assert_eq!(doc_after(&[pack], "spring_water").unwrap()["default"], json!(["a"]));
    }

    #[test]
    fn data_from_copies_a_node_then_applies_the_rest() {
        let pack = json!({"catalogue": {"cultures": {"copy": {
            "$from": "/cultures/mediterranean-modern", "label": "Copy", "plan": {"pattern": "organic"}, "nucleus": null
        }}}});
        let d = doc_after(&[pack], "catalogue").unwrap();
        let (copy, src) = (&d["cultures"]["copy"], &d["cultures"]["mediterranean-modern"]);
        assert_eq!(copy["label"], json!("Copy"));
        assert_eq!(copy["plan"]["pattern"], json!("organic"));
        assert_eq!(copy["plan"]["sources"], src["plan"]["sources"]);
        assert_eq!(copy["recipes"], src["recipes"]);
        assert!(copy.get("nucleus").is_none() && src.get("nucleus").is_some());
        // pointer escapes: `/` in a key is `~1`; packs apply in order and read earlier ones
        let a = json!({"catalogue": {"presets": {"a/core": {"$from": "/presets/mediterranean-modern~1core"}}}});
        let b = json!({"catalogue": {"presets": {"b/core": {"$from": "/presets/a~1core", "label": "B"}}}});
        let d = doc_after(&[a, b], "catalogue").unwrap();
        assert_eq!(d["presets"]["b/core"]["params"], d["presets"]["mediterranean-modern/core"]["params"]);
        assert_eq!(d["presets"]["b/core"]["label"], json!("B"));
    }

    #[test]
    fn data_append_adds_missing_items_only() {
        let at = "/cultures/families/industrial/cultures";
        let pack = json!({"activities": {"cultures": {"families": {"industrial": {"cultures": {
            "$append": ["mediterranean-modern", "copy", "copy"]
        }}}}}});
        let d = doc_after(&[pack], "activities").unwrap();
        let before = builtin(index("activities").unwrap());
        let (after, before) = (d.pointer(at).unwrap().as_array().unwrap(), before.pointer(at).unwrap().as_array().unwrap());
        assert_eq!(after.len(), before.len() + 1);
        assert_eq!(after.last(), Some(&json!("copy")));
        assert_eq!(&after[..before.len()], &before[..]);
        // on a missing key it makes the list
        let pack = json!({"spring_water": {"families": {"new": {"$append": ["x"]}}}});
        assert_eq!(doc_after(&[pack], "spring_water").unwrap()["families"]["new"], json!(["x"]));
    }

    #[test]
    fn data_errors() {
        let err = |p: Value| doc_after(&[p], "catalogue").unwrap_err();
        assert_eq!(err(json!([1])), DataError::PackNotObject { pack: 0 });
        assert!(matches!(err(json!({"nope": {}})), DataError::UnknownDocument { .. }));
        assert!(matches!(err(json!({"tunnelling": 3})), DataError::PatchNotObject { .. }));
        assert!(matches!(err(json!({"tunnelling": null})), DataError::PatchNotObject { .. }));
        let bad = |p: Value| match err(p) {
            DataError::BadDirective { at, reason, .. } => format!("{at}: {reason}"),
            e => panic!("{e}"),
        };
        assert_eq!(bad(json!({"tunnelling": {"cost": {"$merge": 1}}})), "/cost: unknown directive `$merge`");
        assert!(bad(json!({"tunnelling": {"cost": {"$append": [1], "x": 1}}})).contains("stands alone"));
        assert!(bad(json!({"tunnelling": {"cost": {"$append": 1}}})).contains("takes an array"));
        assert!(bad(json!({"tunnelling": {"cost": {"$append": [1]}}})).contains("not an array"));
        assert!(bad(json!({"tunnelling": {"cost": {"$from": 3}}})).contains("pointer string"));
        assert!(matches!(
            err(json!({"tunnelling": {"cost": {"$from": "/nothing"}}})),
            DataError::MissingSource { .. }
        ));
        // the registry of this test process was read: installing now is refused
        let _ = doc("tunnelling");
        assert_eq!(install_packs(&[]), Err(DataError::AlreadyRead));
    }
}
