//! Block data for budgets (ADR 0027): a block definition's category, type,
//! material, supplier and cost, and the drawing's register of currencies
//! the costs use. Both live in the DWG as XRECORDs: the data in the block
//! record's extension dictionary, the register in the drawing's root
//! dictionary. Each XRECORD holds a format version, then name–value pairs;
//! pairs a version does not know are kept when it rewrites one.

use std::collections::{BTreeMap, BTreeSet};

use acadrust::Handle;
use acadrust::objects::{ObjectType, XRecord, XRecordEntry, XRecordValue};
use acadrust::tables::TableEntry;

use crate::document::Document;
use crate::error::{DocumentError, DocumentResult};

/// The key of a block's data in its record's extension dictionary.
const DATA_KEY: &str = "ARQSTUDIOCAD_BLOCKDATA";
/// The key of the currency register in the drawing's root dictionary.
const CURRENCIES_KEY: &str = "ARQSTUDIOCAD_CURRENCIES";
/// The format version this code writes.
const VERSION: i16 = 1;
/// The group codes of the version, a pair's name and its value.
const VERSION_CODE: i32 = 70;
const NAME_CODE: i32 = 1;
const VALUE_CODE: i32 = 300;
/// The longest text a field holds, in characters.
const MAX_TEXT: usize = 255;
/// The most decimals a currency has.
const MAX_DECIMALS: u8 = 4;

/// An exact amount of money: `units` of the currency's smallest part, so
/// 120.50 USD is 12050 units with 2 decimals (ADR 0027 D2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Money {
    /// The amount in the currency's smallest part; zero or more.
    pub units: i64,
    /// How many decimals the amount has.
    pub decimals: u8,
    /// The currency's code, e.g. `USD`.
    pub currency: String,
}

impl Money {
    /// Reads `text`, digits with an optional point and at most `decimals`
    /// decimals ("120", "120.5"), as an amount of `currency`.
    pub fn parse(text: &str, decimals: u8, currency: &str) -> DocumentResult<Self> {
        let invalid = || DocumentError::InvalidBlockData(format!("{text:?} is not an amount"));
        if decimals > MAX_DECIMALS {
            return Err(DocumentError::InvalidBlockData(format!(
                "a currency has at most {MAX_DECIMALS} decimals"
            )));
        }
        let (whole, fraction) = text.trim().split_once('.').unwrap_or((text.trim(), ""));
        let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
        if whole.is_empty() || !digits(whole) || !digits(fraction) {
            return Err(invalid());
        }
        if fraction.len() > usize::from(decimals) {
            return Err(DocumentError::InvalidBlockData(format!(
                "{text:?} has more than {decimals} decimals"
            )));
        }
        let padded = format!("{whole}{fraction:0<width$}", width = usize::from(decimals));
        let units = padded.parse::<i64>().map_err(|_| invalid())?;
        Ok(Self {
            units,
            decimals,
            currency: currency.to_owned(),
        })
    }

    /// The amount as decimal text with its decimals, e.g. `"120.50"`.
    pub fn amount(&self) -> String {
        let scale = 10_i64.pow(u32::from(self.decimals));
        let (whole, fraction) = (self.units / scale, self.units % scale);
        if self.decimals == 0 {
            whole.to_string()
        } else {
            format!(
                "{whole}.{fraction:0width$}",
                width = usize::from(self.decimals)
            )
        }
    }
}

/// What a block definition carries for budgets (ADR 0027 D1); every field
/// is optional, empty when not given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockData {
    /// Its category, free text.
    pub category: String,
    /// Its type, free text.
    pub kind: String,
    /// Its material.
    pub material: String,
    /// Its supplier.
    pub supplier: String,
    /// What one of it costs.
    pub cost: Option<Money>,
}

impl BlockData {
    /// Whether it holds nothing.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A currency of the drawing's register (ADR 0027 D3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Currency {
    /// ISO 4217 or the user's own: 3 to 10 capital letters or digits.
    pub code: String,
    /// Its name, e.g. "US dollar".
    pub name: String,
    /// Its symbol, e.g. `$`; may be empty.
    pub symbol: String,
    /// Its decimals, 0 to 4.
    pub decimals: u8,
    /// Whether a block's cost may use it.
    pub active: bool,
}

impl Currency {
    /// The US dollar, every drawing's currency until it says otherwise.
    pub fn us_dollar() -> Self {
        Self {
            code: "USD".to_owned(),
            name: "US dollar".to_owned(),
            symbol: "$".to_owned(),
            decimals: 2,
            active: true,
        }
    }
}

/// The drawing's currencies, in the register's order, and its principal
/// one, which a new cost proposes first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Currencies {
    /// The currencies.
    pub list: Vec<Currency>,
    /// The principal currency's code: one of `list`, active.
    pub principal: String,
}

impl Default for Currencies {
    /// US dollars alone, principal.
    fn default() -> Self {
        Self {
            list: vec![Currency::us_dollar()],
            principal: "USD".to_owned(),
        }
    }
}

impl Currencies {
    /// The currency with code `code`.
    pub fn get(&self, code: &str) -> Option<&Currency> {
        self.list.iter().find(|c| c.code == code)
    }
}

impl Document {
    /// Block `name`'s data, found ignoring case; `None` when it has none.
    /// Only blocks INSERT can place carry data. Data that does not read is
    /// an error, and stays in the file as it is.
    pub fn block_data(&self, name: &str) -> DocumentResult<Option<BlockData>> {
        let record = self.data_record(name)?;
        let Some(xrecord) = self.cad().xrecord(record, DATA_KEY) else {
            return Ok(None);
        };
        let (_, pairs) =
            read_pairs(xrecord).ok_or_else(|| unreadable(format!("block {name}'s data")))?;
        let field = |key: &str| {
            pairs
                .iter()
                .find(|(n, _)| n == key)
                .map_or(String::new(), |(_, v)| v.clone())
        };
        let cost = match (field("cost"), field("currency")) {
            (amount, _) if amount.is_empty() => None,
            (amount, currency) => {
                let decimals = amount.split_once('.').map_or(0, |(_, f)| f.len());
                let decimals = u8::try_from(decimals)
                    .ok()
                    .filter(|&d| d <= MAX_DECIMALS && !currency.is_empty())
                    .ok_or_else(|| unreadable(format!("block {name}'s cost")))?;
                Some(
                    Money::parse(&amount, decimals, &currency)
                        .map_err(|_| unreadable(format!("block {name}'s cost")))?,
                )
            }
        };
        let data = BlockData {
            category: field("category"),
            kind: field("type"),
            material: field("material"),
            supplier: field("supplier"),
            cost,
        };
        Ok((!data.is_empty()).then_some(data))
    }

    /// Every block's data that reads, by the block's name: what budgets and
    /// the editors' suggestions use.
    pub fn blocks_data(&self) -> BTreeMap<String, BlockData> {
        self.block_names()
            .into_iter()
            .filter_map(|n| Some((n.clone(), self.block_data(&n).ok()??)))
            .collect()
    }

    /// Sets block `name`'s data, found ignoring case; empty data removes
    /// it. The texts are one line of at most 255 characters; a cost is in
    /// an active currency of the register, with its decimals. Returns the
    /// block's name as the drawing spells it.
    pub fn set_block_data(&mut self, name: &str, data: &BlockData) -> DocumentResult<String> {
        let block = self.data_block(name)?;
        for (field, text) in [
            ("category", &data.category),
            ("type", &data.kind),
            ("material", &data.material),
            ("supplier", &data.supplier),
        ] {
            check_text(field, text)?;
        }
        if let Some(cost) = &data.cost {
            let currencies = self.currencies()?;
            let currency = currencies
                .get(&cost.currency)
                .filter(|c| c.active)
                .ok_or_else(|| {
                    DocumentError::InvalidBlockData(format!(
                        "{} is not an active currency of the drawing",
                        cost.currency
                    ))
                })?;
            if cost.decimals != currency.decimals || cost.units < 0 {
                return Err(DocumentError::InvalidBlockData(format!(
                    "a cost in {} has {} decimals and is zero or more",
                    currency.code, currency.decimals
                )));
            }
        }
        if self.block_data(&block).ok().flatten().unwrap_or_default() == *data {
            return Ok(block);
        }
        self.touch_structure();
        self.note_block(&block);
        self.write_block_data(&block, data);
        Ok(block)
    }

    /// The drawing's currency register; US dollars alone when it has none.
    pub fn currencies(&self) -> DocumentResult<Currencies> {
        let Some(xrecord) = self.register() else {
            return Ok(Currencies::default());
        };
        let bad = || unreadable("the drawing's currencies".to_owned());
        let (_, pairs) = read_pairs(xrecord).ok_or_else(bad)?;
        let mut register = Currencies {
            list: Vec::new(),
            principal: String::new(),
        };
        for (key, value) in pairs {
            let current = register.list.last_mut();
            match (key.as_str(), current) {
                ("principal", _) => register.principal = value,
                ("code", _) => register.list.push(Currency {
                    code: value,
                    name: String::new(),
                    symbol: String::new(),
                    decimals: 0,
                    active: false,
                }),
                ("name", Some(c)) => c.name = value,
                ("symbol", Some(c)) => c.symbol = value,
                ("decimals", Some(c)) => c.decimals = value.parse().map_err(|_| bad())?,
                ("active", Some(c)) => c.active = value == "1",
                _ => {}
            }
        }
        check_register(&register).map_err(|_| bad())?;
        Ok(register)
    }

    /// How many blocks' costs use each currency, by code.
    pub fn currency_uses(&self) -> BTreeMap<String, usize> {
        let mut uses = BTreeMap::new();
        for cost in self.blocks_data().into_values().filter_map(|d| d.cost) {
            *uses.entry(cost.currency).or_insert(0) += 1;
        }
        uses
    }

    /// Replaces the drawing's currency register (ADR 0027 D3). Codes are 3
    /// to 10 capital letters or digits, distinct; names are not empty;
    /// decimals are 0 to 4; the principal currency is active. A currency
    /// a block's cost uses cannot be removed, deactivated, or given other
    /// decimals.
    pub fn set_currencies(&mut self, currencies: &Currencies) -> DocumentResult<()> {
        check_register(currencies)?;
        let mut costs: BTreeMap<String, (usize, BTreeSet<u8>)> = BTreeMap::new();
        for cost in self.blocks_data().into_values().filter_map(|d| d.cost) {
            let entry = costs.entry(cost.currency).or_default();
            entry.0 += 1;
            entry.1.insert(cost.decimals);
        }
        let known = self.currencies()?;
        for (code, (count, decimals)) in &costs {
            // A cost in a currency the register never held stays as it is.
            if known.get(code).is_none() {
                continue;
            }
            let kept = currencies
                .get(code)
                .is_some_and(|c| c.active && decimals.iter().all(|&d| d == c.decimals));
            if !kept {
                let blocks = if *count == 1 { "block" } else { "blocks" };
                return Err(DocumentError::InvalidBlockData(format!(
                    "{code} is used by {count} {blocks}: it must stay active, with its decimals"
                )));
            }
        }
        if known == *currencies {
            return Ok(());
        }
        self.touch_structure();
        self.note_document();
        self.write_currencies(currencies);
        Ok(())
    }

    /// Where `written`, this drawing read back, holds other block data or
    /// another currency register (ADR 0027 D4), for `validate_roundtrip`.
    pub(crate) fn compare_block_data(&self, written: &Document, differences: &mut Vec<String>) {
        if self.currencies().ok() != written.currencies().ok() {
            differences.push("the currency register changed".to_owned());
        }
        for name in self.block_names() {
            let data = |doc: &Document| doc.block_data(&name).ok().flatten();
            if data(self) != data(written) {
                differences.push(format!("block {name}'s data changed"));
            }
        }
    }

    /// Writes block `block`'s data as given, unchecked: empty data removes
    /// it. The caller has touched the structure.
    pub(crate) fn write_block_data(&mut self, block: &str, data: &BlockData) {
        let Some(record) = self.cad().block_records.get(block).map(|r| r.handle()) else {
            return;
        };
        if data.is_empty() {
            self.remove_block_data(record);
            return;
        }
        let mut pairs = vec![
            ("category", data.category.clone()),
            ("type", data.kind.clone()),
            ("material", data.material.clone()),
            ("supplier", data.supplier.clone()),
        ];
        if let Some(cost) = &data.cost {
            pairs.push(("cost", cost.amount()));
            pairs.push(("currency", cost.currency.clone()));
        }
        let pairs: Vec<(String, String)> = pairs
            .into_iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(n, v)| (n.to_owned(), v))
            .collect();
        let known = [
            "category", "type", "material", "supplier", "cost", "currency",
        ];
        let cad = self.cad_mut();
        let handle = cad.ensure_xrecord(record, DATA_KEY);
        if let Some(ObjectType::XRecord(xrecord)) = cad.objects.get_mut(&handle) {
            let kept = keep_unknown(xrecord, &known);
            write_pairs(xrecord, pairs.into_iter().chain(kept));
        }
    }

    /// Adds `currency` to the register, active, unless it holds its code;
    /// a copied block's cost brings it (ADR 0027 D6). The caller has
    /// touched the structure.
    pub(crate) fn add_currency(&mut self, mut currency: Currency) {
        let mut register = self.currencies().unwrap_or_default();
        if register.get(&currency.code).is_some() {
            return;
        }
        currency.active = true;
        register.list.push(currency);
        self.note_document();
        self.write_currencies(&register);
    }

    fn write_currencies(&mut self, currencies: &Currencies) {
        let mut pairs = vec![("principal".to_owned(), currencies.principal.clone())];
        for c in &currencies.list {
            pairs.extend([
                ("code".to_owned(), c.code.clone()),
                ("name".to_owned(), c.name.clone()),
                ("symbol".to_owned(), c.symbol.clone()),
                ("decimals".to_owned(), c.decimals.to_string()),
                (
                    "active".to_owned(),
                    if c.active { "1" } else { "0" }.to_owned(),
                ),
            ]);
        }
        let handle = self.ensure_register();
        let known = ["principal", "code", "name", "symbol", "decimals", "active"];
        if let Some(ObjectType::XRecord(xrecord)) = self.cad_mut().objects.get_mut(&handle) {
            let kept = keep_unknown(xrecord, &known);
            write_pairs(xrecord, pairs.into_iter().chain(kept));
        }
    }

    /// The register's XRECORD, made in the root dictionary when missing.
    fn ensure_register(&mut self) -> Handle {
        if let Some(found) = self.register() {
            return found.handle;
        }
        let cad = self.cad_mut();
        let root = cad.header.named_objects_dict_handle;
        let handle = cad.allocate_handle();
        let mut xrecord = XRecord::named(CURRENCIES_KEY);
        xrecord.handle = handle;
        xrecord.owner = root;
        cad.objects.insert(handle, ObjectType::XRecord(xrecord));
        if let Some(ObjectType::Dictionary(d)) = cad.objects.get_mut(&root) {
            d.entries
                .retain(|(k, _)| !k.eq_ignore_ascii_case(CURRENCIES_KEY));
            d.add_entry(CURRENCIES_KEY, handle);
        }
        handle
    }

    fn register(&self) -> Option<&XRecord> {
        let cad = self.cad();
        let root = cad.header.named_objects_dict_handle;
        let Some(ObjectType::Dictionary(d)) = cad.objects.get(&root) else {
            return None;
        };
        match cad.objects.get(&d.get(CURRENCIES_KEY)?) {
            Some(ObjectType::XRecord(x)) => Some(x),
            _ => None,
        }
    }

    fn remove_block_data(&mut self, record: Handle) {
        let cad = self.cad_mut();
        let Some(dictionary) = cad.extension_dictionary_handle(record) else {
            return;
        };
        let Some(ObjectType::Dictionary(d)) = cad.objects.get_mut(&dictionary) else {
            return;
        };
        let Some(handle) = d.get(DATA_KEY) else {
            return;
        };
        d.entries.retain(|(k, _)| !k.eq_ignore_ascii_case(DATA_KEY));
        cad.objects.remove(&handle);
    }

    /// The record of block `name`, which INSERT can place.
    fn data_record(&self, name: &str) -> DocumentResult<Handle> {
        let block = self.data_block(name)?;
        Ok(self
            .cad()
            .block_records
            .get(&block)
            .map_or(Handle::NULL, |r| r.handle()))
    }

    /// Block `name` as the drawing spells it, if INSERT can place it.
    fn data_block(&self, name: &str) -> DocumentResult<String> {
        self.block_names()
            .into_iter()
            .find(|n| n.eq_ignore_ascii_case(name))
            .ok_or_else(|| DocumentError::InvalidBlockName(format!("no block named {name:?}")))
    }

    /// Currency `code` as this drawing's register describes it, or made up
    /// from the code and `decimals` when the register lacks it: what a
    /// block copied from here brings to another drawing's register.
    pub(crate) fn currency_of(&self, code: &str, decimals: u8) -> Currency {
        self.currencies()
            .ok()
            .and_then(|r| r.get(code).cloned())
            .unwrap_or_else(|| Currency {
                code: code.to_owned(),
                name: code.to_owned(),
                symbol: String::new(),
                decimals,
                active: true,
            })
    }
}

fn unreadable(what: String) -> DocumentError {
    DocumentError::InvalidBlockData(format!("{what} could not be read"))
}

fn check_text(field: &str, text: &str) -> DocumentResult<()> {
    if text.chars().any(char::is_control) || text.chars().count() > MAX_TEXT {
        return Err(DocumentError::InvalidBlockData(format!(
            "the {field} is one line of at most {MAX_TEXT} characters"
        )));
    }
    Ok(())
}

fn check_register(currencies: &Currencies) -> DocumentResult<()> {
    let invalid = |why: String| Err(DocumentError::InvalidBlockData(why));
    let mut codes = BTreeSet::new();
    for c in &currencies.list {
        let valid = (3..=10).contains(&c.code.len())
            && c.code
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
        if !valid {
            return invalid(format!(
                "{:?} is not a currency code: 3 to 10 capital letters or digits",
                c.code
            ));
        }
        if !codes.insert(c.code.as_str()) {
            return invalid(format!("{} is in the register twice", c.code));
        }
        if c.name.trim().is_empty() {
            return invalid(format!("{} has no name", c.code));
        }
        check_text("name", &c.name)?;
        check_text("symbol", &c.symbol)?;
        if c.decimals > MAX_DECIMALS {
            return invalid(format!("{} has more than {MAX_DECIMALS} decimals", c.code));
        }
    }
    if !currencies
        .get(&currencies.principal)
        .is_some_and(|c| c.active)
    {
        return invalid("the principal currency is one of the register, active".to_owned());
    }
    Ok(())
}

/// An XRECORD's version and name–value pairs; `None` unless it is a
/// version followed by pairs.
fn read_pairs(xrecord: &XRecord) -> Option<(i16, Vec<(String, String)>)> {
    let mut entries = xrecord.entries.iter();
    let version = match entries.next()? {
        XRecordEntry {
            code: VERSION_CODE,
            value: XRecordValue::Int16(v),
        } => *v,
        _ => return None,
    };
    let mut pairs = Vec::new();
    while let Some(name) = entries.next() {
        let value = entries.next()?;
        match (name, value) {
            (
                XRecordEntry {
                    code: NAME_CODE,
                    value: XRecordValue::String(n),
                },
                XRecordEntry {
                    code: VALUE_CODE,
                    value: XRecordValue::String(v),
                },
            ) => pairs.push((n.clone(), v.clone())),
            _ => return None,
        }
    }
    (version >= 1).then_some((version, pairs))
}

/// The pairs of `xrecord` whose names are not `known`, to keep them.
fn keep_unknown(xrecord: &XRecord, known: &[&str]) -> Vec<(String, String)> {
    read_pairs(xrecord)
        .map(|(_, pairs)| {
            pairs
                .into_iter()
                .filter(|(n, _)| !known.contains(&n.as_str()))
                .collect()
        })
        .unwrap_or_default()
}

fn write_pairs(xrecord: &mut XRecord, pairs: impl IntoIterator<Item = (String, String)>) {
    xrecord.clear();
    xrecord.add_int16(VERSION_CODE, VERSION);
    for (name, value) in pairs {
        xrecord.add_string(NAME_CODE, name);
        xrecord.add_string(VALUE_CODE, value);
    }
}

#[cfg(test)]
#[path = "block_data_tests.rs"]
mod tests;
