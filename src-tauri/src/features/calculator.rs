//! Arithmetic, unit conversion, and currency conversion through fend.
//! Phrases that fend does not read, such as `$30 + 20%` or `10 plus 5`, are
//! rewritten into its syntax first.

use std::{
    borrow::Cow,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use crate::{
    features::{currency::Rates, datetime},
    search::result::{Action, Icon, ResultAction, ResultKind, SearchResult, Symbol},
};

/// fend checks this deadline while it evaluates, but not while it parses:
/// `parses_fast` keeps the parse short. Tests get more time: a slow, busy
/// runner must not turn an answer into none.
const TIME_LIMIT: Duration = if cfg!(test) {
    Duration::from_secs(10)
} else {
    Duration::from_millis(100)
};
/// Longest query, in characters, that the calculator reads. fend's parser
/// recurses for each sign or operand, and a release build overflows a 2 MiB
/// stack near 2,000 (a debug build, with its larger stack frames, near 100).
const MAX_INPUT: usize = 500;
/// Most parentheses in a query. Each one nests fend's parser about 20 calls
/// deeper. In a release build, about 170 nested calls such as `sin(`
/// overflow a 2 MiB stack.
const MAX_PARENTHESES: usize = 32;
/// Deepest nesting of fend's input, after the rewrites that add parentheses.
/// fend adds a `(` at the start for each `)`, so this counts each `)` and
/// the deepest group. A release build overflows a 2 MiB stack near 550.
const MAX_DEPTH: usize = 64;
/// Most parse work, as `parses_fast` counts it: about 20 ms in a release
/// build for the slowest kind of input.
const MAX_PARSE_WORK: u64 = 50_000;
/// Most calculations kept in history.
pub const MAX_CALCULATIONS: usize = 50;
/// Longest expression, in characters, kept in history.
pub const MAX_EXPRESSION: usize = 500;
/// Most history rows one search shows.
const HISTORY_ROWS: usize = 20;

pub fn answer(query: &str, rates: Option<&Rates>) -> Option<SearchResult> {
    let (value, used_rates) = evaluate(query, rates)?;
    let subtitle = match rates {
        Some(rates) if used_rates => format!("{query} · ECB rates from {}", rates.date),
        _ => format!("= {query}"),
    };
    Some(SearchResult {
        id: format!("calc:{query}"),
        kind: ResultKind::Calculation,
        title: value.clone(),
        subtitle,
        icon: Icon::Symbol {
            name: Symbol::Calculator,
        },
        actions: vec![ResultAction::new(
            "Copy Result",
            Action::CopyCalculation {
                query: query.to_owned(),
            },
        )],
        pinned: false,
    })
}

/// The result of `query` as the calculator answers it, for copying.
pub fn result(query: &str, rates: Option<&Rates>) -> Option<String> {
    evaluate(query, rates).map(|(value, _)| value)
}

/// The expression in `query`: a spreadsheet user types `=5+3` for `5+3`.
/// A query that asks for the history stays as it is.
pub fn expression(query: &str) -> &str {
    match history_filter(query) {
        Some(_) => query,
        None => query.strip_prefix('=').unwrap_or(query),
    }
}

/// `=` alone, or `=` and a space, asks for the history: the words after it.
fn history_filter(query: &str) -> Option<&str> {
    let rest = query.trim().strip_prefix('=')?;
    (rest.is_empty() || rest.starts_with(char::is_whitespace)).then_some(rest)
}

/// A calculation the user copied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Calculation {
    pub id: i64,
    pub expression: String,
    pub result: String,
}

/// Copied calculations, newest first.
#[derive(Default)]
pub struct CalculationHistory {
    entries: Vec<Calculation>,
}

impl CalculationHistory {
    pub fn new(entries: Vec<Calculation>) -> Self {
        Self { entries }
    }

    pub fn get(&self, id: i64) -> Option<&Calculation> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// `=` lists the history; `= words` lists the calculations whose
    /// expression or result holds every word. `None` for any other query,
    /// such as `=5+3`, which a spreadsheet user means as a calculation.
    pub fn answer(&self, query: &str) -> Option<Vec<SearchResult>> {
        let filter = history_filter(query)?.to_lowercase();
        let words: Vec<&str> = filter.split_whitespace().collect();
        Some(
            self.entries
                .iter()
                .filter(|entry| {
                    let text = format!("{} {}", entry.expression, entry.result).to_lowercase();
                    words.iter().all(|word| text.contains(word))
                })
                .take(HISTORY_ROWS)
                .map(history_result)
                .collect(),
        )
    }
}

fn history_result(entry: &Calculation) -> SearchResult {
    SearchResult {
        id: format!("calc-history:{}", entry.id),
        kind: ResultKind::Calculation,
        title: entry.result.clone(),
        subtitle: format!("= {}", entry.expression),
        icon: Icon::Symbol {
            name: Symbol::Calculator,
        },
        // The saved result, as copied then; rates may have changed since.
        actions: vec![
            ResultAction::new("Copy Result", Action::CopySavedCalculation { id: entry.id }),
            ResultAction::new(
                "Copy Expression",
                Action::Copy {
                    text: entry.expression.clone(),
                },
            ),
            ResultAction::new(
                "Delete from History",
                Action::DeleteCalculation { id: entry.id },
            ),
        ],
        pinned: false,
    }
}

/// While currency rates are off, a query that needs them, such as
/// `100 usd to eur`, says so and offers to turn them on.
pub fn rates_off_answer(query: &str) -> Option<SearchResult> {
    if !needs_rates(query) {
        return None;
    }
    Some(SearchResult {
        id: format!("calc:{query}"),
        kind: ResultKind::Calculation,
        title: "Currency rates are off".into(),
        subtitle: "Turn them on to convert. TinyDash then downloads the daily ECB rate table."
            .into(),
        icon: Icon::Symbol {
            name: Symbol::Calculator,
        },
        actions: vec![ResultAction::new(
            "Turn On Currency Rates",
            Action::TurnOnCurrencyRates,
        )],
        pinned: false,
    })
}

/// Whether fend asks for an exchange rate to evaluate the query, found with
/// a stand-in rate of 1.
fn needs_rates(query: &str) -> bool {
    if !query.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    let Some(input) = fend_input(query) else {
        return false;
    };
    let asked = Arc::new(AtomicBool::new(false));
    let mut context = fend_core::Context::new();
    context.disable_rng();
    context.set_exchange_rate_handler_v2(Probe {
        asked: asked.clone(),
    });
    let deadline = Deadline(Instant::now() + TIME_LIMIT);
    let _ = fend_core::evaluate_preview_with_interrupt(&input, &context, &deadline);
    asked.load(Ordering::Relaxed)
}

/// The result text and whether it needed exchange rates.
fn evaluate(query: &str, rates: Option<&Rates>) -> Option<(String, bool)> {
    if !query.chars().any(|c| c.is_ascii_digit()) && !matches!(query, "pi" | "π") {
        return None;
    }
    if datetime::is_date_or_clock(query) {
        return None;
    }
    let input = fend_input(query)?;
    let mut context = fend_core::Context::new();
    context.disable_rng();
    // fend matches units case-insensitively and reads `Bps` as bits. Define
    // bytes per second so `8 Mbps to MBps` gives 1 MBps.
    context.define_custom_unit_v1(
        "Bps",
        "",
        "byte / second",
        &fend_core::CustomUnitAttribute::AllowShortPrefix,
    );
    let used_rates = Arc::new(AtomicBool::new(false));
    if let Some(rates) = rates {
        context.set_exchange_rate_handler_v2(RateLookup {
            rates: rates.clone(),
            used: used_rates.clone(),
        });
    }
    let deadline = Deadline(Instant::now() + TIME_LIMIT);
    let result = fend_core::evaluate_preview_with_interrupt(&input, &context, &deadline);
    let is_number = result
        .get_main_result_spans()
        .any(|span| span.kind() == fend_core::SpanKind::Number);
    let value = result.get_main_result().trim().to_owned();
    if !is_number || value.is_empty() {
        return None;
    }
    // A bare number echoes itself, also as fend writes it after the
    // rewrites (`30 percent` as `30%`); that is not an answer.
    let bare = |text: &str| text.split_whitespace().collect::<String>();
    let echo = bare(&value);
    if echo == bare(query) || echo == bare(&input) {
        return None;
    }
    if is_f64_artifact(query, &value) {
        return None;
    }
    if !used_rates.load(Ordering::Relaxed) {
        return Some((value, false));
    }
    Some((money(&value).unwrap_or(value), true))
}

/// Whether fend 1.5.8 computed the value through an f64 and got it wrong. A
/// wrong answer is worse than none.
///
/// `floor`, `ceil`, and `round` lose digits above 2^53 and cap at 2^64, so
/// `round(9007199254740993)` gives 9007199254740992 and `floor(1e20)` gives
/// 18446744073709551616. A run of 14 or more digits holds every such value
/// in decimal, octal, binary, and hex (2^52 has 14 hex digits). This also
/// drops correct answers from 10^13 up in decimal, from 8^13 (about 5.5e11)
/// up in octal, and from 8192 up in binary. A looser limit for runs of only
/// 0 and 1 would keep small binary answers, but it would also keep wrong
/// decimal ones: `round(10000000000000001)` gives 10000000000000000.
///
/// `sinh` and `cosh` cap at 2^64 too: `sinh(50)` gives 18446744073709551616.
fn is_f64_artifact(query: &str, value: &str) -> bool {
    let query = query.to_ascii_lowercase();
    let names = |names: &[&str]| names.iter().any(|name| query.contains(name));
    if names(&["floor", "ceil", "round"]) {
        let longest_run = value
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|run| run.chars().any(|c| c.is_ascii_digit()))
            .map(str::len)
            .max();
        return longest_run.is_some_and(|len| len >= 14);
    }
    names(&["sinh", "cosh"]) && value.contains("18446744073709551616")
}

/// `approx. 401.7094 MYR` as `401.71 MYR`. The subtitle already says the
/// value comes from daily rates. (fend's own `to 2 dp` rounds incorrectly.)
/// An amount below 1 keeps 3 significant digits, so `1 IDR to EUR` is not
/// `0.00 EUR`. A fraction, such as `401 83/117 MYR` or `47000/117 MYR`,
/// stays as fend wrote it. A unit with a `/`, such as `MYR / kg`, is rounded.
fn money(value: &str) -> Option<String> {
    let (amount, currency) = value.trim_start_matches("approx. ").split_once(' ')?;
    if currency.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let amount: f64 = amount.parse().ok()?;
    let decimals = if amount == 0.0 || amount.abs() >= 1.0 {
        2
    } else {
        // log10 of a finite amount below 1 is at least -324, so this fits.
        (2.0 - amount.abs().log10().floor()) as usize
    };
    Some(format!("{amount:.decimals$} {currency}"))
}

/// `100 USD MYR` means `100 USD to MYR`.
fn currency_shorthand(query: &str) -> Option<String> {
    let [amount, from, to] =
        <[&str; 3]>::try_from(query.split_whitespace().collect::<Vec<_>>()).ok()?;
    let is_code = |code: &str| code.len() == 3 && code.bytes().all(|b| b.is_ascii_uppercase());
    (amount.parse::<f64>().is_ok() && is_code(from) && is_code(to))
        .then(|| format!("{amount} {from} to {to}"))
}

/// The query in fend's syntax, or `None` when fend would read a percentage
/// in it differently from people or could not parse it fast.
fn fend_input(query: &str) -> Option<String> {
    let ascii = ascii_signs(query);
    // The limits that the user sees, on what they typed.
    if query.chars().nth(MAX_INPUT).is_some()
        || query.matches(['(', ')']).count() > MAX_PARENTHESES
        || has_lambda(&ascii)
    {
        return None;
    }
    let input = match currency_shorthand(&ascii) {
        Some(input) => input,
        None => percentage(&word_operators(&euro_amounts(&ascii)))?,
    };
    parses_fast(&input).then_some(input)
}

/// The query with fend's other spellings of its signs (fend-core 1.5.8,
/// `lexer.rs`) in ASCII, so that the checks here see every sign. One
/// difference: fend reads `1e−2` as `1 e − 2`, and this as 0.01.
fn ascii_signs(query: &str) -> String {
    let mut input = String::with_capacity(query.len());
    for c in query.chars() {
        match c {
            '\u{2212}' => input.push('-'),
            // fend reads `×*` as a power, like `**`, but `*×` and `××` as
            // two products. The space keeps them apart.
            '×' | '✕' => input.push_str(" *"),
            '÷' | '∕' => input.push('/'),
            '≠' => input.push_str("!="),
            'λ' => input.push('\\'),
            c => input.push(c),
        }
    }
    input
}

/// Whether `input` has a lambda in any of fend's spellings: `\x.x + 1`,
/// `x: x + 1`, or `x => x + 1` (`λ` is `\` by now). A launcher needs none.
/// fend parses the body of a `\` lambda again for each reading around it,
/// and a lambda that calls itself, such as `(x: x x)(x: x x) 1`, recurses
/// until it overflows the stack, which crashes TinyDash.
fn has_lambda(input: &str) -> bool {
    input.contains(['\\', ':']) || input.contains("=>")
}

/// Whether fend parses `input` fast and within the stack. fend cannot stop
/// its parser, which tries one reading after another: it parses a group in
/// parentheses once for each `+` or `-` term before it, up to three times
/// more where other readings start (see `group_reads`), and all of that
/// again for each parse of the group around it. The count is an upper
/// bound, also for input that fails to parse, as it does while the user
/// types.
fn parses_fast(input: &str) -> bool {
    // How often fend parses the current group, and its terms so far.
    let (mut parses, mut terms) = (1_u64, 0_u64);
    let mut outer = Vec::new();
    let (mut work, mut closed, mut deepest) = (0_u64, 0, 0);
    for (at, c) in input.char_indices() {
        let reads = parses.saturating_mul(terms + 1);
        work = work.saturating_add(reads);
        match c {
            '(' => {
                outer.push((parses, terms));
                deepest = deepest.max(outer.len());
                parses = reads.saturating_mul(group_reads(&input[..at]));
                terms = 0;
            }
            ')' => {
                closed += 1;
                if let Some(group) = outer.pop() {
                    (parses, terms) = group;
                }
            }
            '+' | '-' => terms += 1,
            _ => {}
        }
    }
    closed + deepest <= MAX_DEPTH && work <= MAX_PARSE_WORK
}

/// How often fend parses a group that follows `text`, for each parse of the
/// term around it. Once after `(`, a sign, or `*`. Twice after `/`: when the
/// group fails, fend reads it again as `1/x`. Up to three times where it
/// also tries a mixed fraction or an implied product: after a value (`2(`),
/// in a power (`1 2^-(`), and in a mixed fraction (`1 2/(`).
fn group_reads(text: &str) -> u64 {
    let unsigned = text.trim_end_matches(|c: char| c.is_whitespace() || matches!(c, '+' | '-'));
    let signed = unsigned.len() < text.trim_end().len();
    let mut operand = unsigned.chars();
    match operand.next_back() {
        None | Some('(') => 1,
        Some('*') if !ends_with_power(unsigned) => 1,
        Some('/') if !ends_mixed_fraction_top(operand.as_str()) => 2,
        Some('^' | '*' | '/') => 3,
        // A sign after a value adds or subtracts: the terms count it.
        Some(_) if signed => 1,
        Some(_) => 3,
    }
}

/// Whether `text` ends with fend's power sign, `^` or `**`.
fn ends_with_power(text: &str) -> bool {
    text.ends_with('^') || text.ends_with("**")
}

/// Whether `text` may end the top of a mixed fraction, as `1 2` does in
/// `1 2/(`: a number right after a value.
fn ends_mixed_fraction_top(text: &str) -> bool {
    let text = text.trim_end();
    let token = |c: char| c.is_alphanumeric() || matches!(c, '.' | ',');
    let mut before = text.trim_end_matches(token);
    // In `2e-3`, the sign belongs to the exponent.
    if let Some(mantissa) = before.strip_suffix(['+', '-'])
        && mantissa.ends_with(['e', 'E'])
    {
        before = mantissa.trim_end_matches(token);
    }
    let number = &text[before.len()..];
    if number.is_empty() {
        // `(1 + 2)/(` divides. After an operator, `/` is `1/x`, which can
        // belong to a power, as in `1 2^/(`.
        return !text.ends_with(')');
    }
    number.starts_with(|c: char| c.is_ascii_digit() || c == '.')
        && before.trim_end().ends_with(|c: char| !"(+-*/^".contains(c))
}

/// `€20` as `20 EUR`: fend reads `$20` and `£20`, but not `€20`.
fn euro_amounts(query: &str) -> Cow<'_, str> {
    if !query.contains('€') {
        return Cow::Borrowed(query);
    }
    let mut input = String::with_capacity(query.len() + 4);
    let mut rest = query;
    while let Some(at) = rest.find('€') {
        input.push_str(&rest[..at]);
        let after = &rest[at + '€'.len_utf8()..];
        let amount = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | ',')))
            .unwrap_or(after.len());
        if after.starts_with(|c: char| c.is_ascii_digit()) {
            input.push_str(&after[..amount]);
            input.push_str(" EUR");
            rest = &after[amount..];
        } else {
            input.push('€');
            rest = after;
        }
    }
    input.push_str(rest);
    Cow::Owned(input)
}

/// `plus`, `minus`, `times`, `multiplied by`, and `divided by` as signs, and
/// `percent` as `%`. Also `per` as `/`, which fend reads the same, so that
/// the checks here see the division.
fn word_operators(query: &str) -> Cow<'_, str> {
    let words: Vec<&str> = query.split_whitespace().collect();
    let is = |i: usize, word: &str| words.get(i).is_some_and(|w| w.eq_ignore_ascii_case(word));
    let mut input = Vec::with_capacity(words.len());
    let mut changed = false;
    let mut i = 0;
    while i < words.len() {
        let (sign, used) = if is(i, "plus") {
            ("+", 1)
        } else if is(i, "minus") {
            ("-", 1)
        } else if is(i, "times") {
            ("*", 1)
        } else if is(i, "multiplied") && is(i + 1, "by") {
            ("*", 2)
        } else if is(i, "divided") && is(i + 1, "by") {
            ("/", 2)
        } else if words[i] == "per" {
            // Only this spelling: fend reads `Per` as a name.
            ("/", 1)
        } else if is(i, "percent") {
            ("%", 1)
        } else if let Some(number) = strip_suffix(words[i], "percent").filter(|n| is_number(n)) {
            // `20percent`
            input.push(number);
            ("%", 1)
        } else {
            (words[i], 0)
        };
        changed |= used > 0;
        input.push(sign);
        i += used.max(1);
    }
    if changed {
        Cow::Owned(input.join(" "))
    } else {
        Cow::Borrowed(query)
    }
}

/// fend's input for two parts of a question.
type Formula = fn(&str, &str) -> String;

/// `10% of`, `10% on`, and `10% off` as a factor: `10 / 100`, `1 + 10 / 100`,
/// and `1 - 10 / 100`.
const FACTORS: [(&str, &str); 3] = [("of", ""), ("on", "1 + "), ("off", "1 - ")];

/// Percentages as people say them, in fend's syntax, or `None` for a
/// percentage in any other shape. fend reads `30 + 20%` as 30.2 and
/// `200 / 10%` as 20%, cannot add a percentage to money, and has no `off` or
/// `is what %`. A wrong answer is worse than none.
fn percentage(query: &str) -> Option<String> {
    if let Some(input) = percentage_question(query).or_else(|| percentage_steps(query)) {
        return Some(input);
    }
    let input = percentage_phrases(query);
    (!mixes_percentage(&input)).then(|| input.into_owned())
}

/// A question about a percentage, as a whole: `20 is what % of 200`,
/// `50 to 75 is what %`, `3/20 is what %`, or `20 is 10% of what`. Each part
/// is a plain amount, or a ratio of two in `3/20 is what %`.
fn percentage_question(query: &str) -> Option<String> {
    let query = query.trim();
    for ending in [" is what %", " as %", " as a %"] {
        if let Some(left) = strip_suffix(query, ending) {
            return match around(left, " to ") {
                Some((from, to)) => (is_amount(from) && is_amount(to))
                    .then(|| format!("(({to}) - ({from})) / ({from}) to %")),
                None => left
                    .split_once('/')
                    .map_or(is_amount(left), |(a, b)| is_amount(a) && is_amount(b))
                    .then(|| format!("({left}) to %")),
            };
        }
    }
    let ratios: [(&str, Formula); 5] = [
        (" is what % of ", |a, b| format!("({a}) / ({b}) to %")),
        (" as a % of ", |a, b| format!("({a}) / ({b}) to %")),
        (" as % of ", |a, b| format!("({a}) / ({b}) to %")),
        (" is what % off ", |a, b| {
            format!("(1 - ({a}) / ({b})) to %")
        }),
        (" is what % on ", |a, b| format!("(({a}) / ({b}) - 1) to %")),
    ];
    for (separator, formula) in ratios {
        if let Some((a, b)) = around(query, separator) {
            return (is_amount(a) && is_amount(b)).then(|| formula(a, b));
        }
    }
    // `20 is 10% of what`: the number that 20 is 10% of.
    let left = strip_suffix(query, " what")?;
    FACTORS.iter().find_map(|(word, factor)| {
        let (value, percent) = around(strip_suffix(left, word)?.strip_suffix('%')?, " is ")?;
        (is_amount(value) && is_number(percent))
            .then(|| format!("({value}) / ({factor}{percent} / 100)"))
    })
}

/// `$30 + 20%` adds 20% of $30, and `200 - 10% - 5%` takes 5% off 180: each
/// percentage is of the value before it. The whole query is a plain amount
/// and such steps.
fn percentage_steps(query: &str) -> Option<String> {
    let query = query.trim();
    let amount = &query[..amount_len(query)?];
    let mut rest = query[amount.len()..].trim_start();
    if rest.is_empty() {
        return None;
    }
    // Each step multiplies the one before it, so a chain adds no nesting.
    let mut input = format!("({amount})");
    while !rest.is_empty() {
        let step = rest.strip_prefix(['+', '-'])?.trim_start();
        let sign = &rest[..1];
        let percent = &step[..number_len(step)?];
        rest = step[percent.len()..]
            .trim_start()
            .strip_prefix('%')?
            .trim_start();
        input.push_str(&format!(" * (1 {sign} {percent} / 100)"));
    }
    Some(input)
}

/// `10% of 200`, `10% on 200`, and `10% off 200` as `((10 / 100) * (200))`,
/// `((1 + 10 / 100) * (200))`, and `((1 - 10 / 100) * (200))`: one value,
/// where it stands, so `10% of 200 + 50` is 70. Only where that is the one
/// reading (see `starts_value` and `ends_value`). After `/` or a power, fend
/// takes a percentage of all that comes before it: `1/2% of 200` is 1. After
/// a sign alone, `-10% off 200` could be -180 or 220.
fn percentage_phrases(query: &str) -> Cow<'_, str> {
    let mut input = String::new();
    // `input` holds `query` up to `copied`, rewritten.
    let mut copied = 0;
    for (at, _) in query.match_indices('%') {
        // A `%` before `copied` is in a phrase already rewritten.
        let Some(before) = query.get(copied..=at).and_then(before_percentage) else {
            continue;
        };
        let percent = query[copied + before.len()..at].trim();
        let after = query[at + 1..].trim_start();
        let letters = after.find(|c: char| !c.is_alphabetic());
        let (word, rest) = after.split_at(letters.unwrap_or(after.len()));
        let Some((_, factor)) = FACTORS
            .iter()
            .find(|(name, _)| word.eq_ignore_ascii_case(name))
        else {
            continue;
        };
        let amount = rest.trim_start();
        let Some(len) = amount_len(amount) else {
            continue;
        };
        // What the user typed before the percentage, so that an amount in
        // a phrase already rewritten is a value too.
        if is_number(percent)
            && amount.len() < rest.len()
            && starts_value(&query[..copied + before.len()])
            && ends_value(&amount[len..])
        {
            input.push_str(before);
            input.push_str(&format!(
                "(({factor}{percent} / 100) * ({}))",
                &amount[..len]
            ));
            copied = query.len() - amount.len() + len;
        }
    }
    if copied == 0 {
        return Cow::Borrowed(query);
    }
    input.push_str(&query[copied..]);
    Cow::Owned(input)
}

/// Whether a value after `text` stands on its own: at the start, or after
/// `(` or a `+`, `-`, or `*` between two values. A word that fend reads as
/// an operator, such as `mod` in `400 mod -10% off 200`, is not a value.
fn starts_value(text: &str) -> bool {
    let text = text.trim_end();
    match text.strip_suffix(['+', '-', '*']) {
        Some(left) => {
            let left = left.trim_end();
            left.ends_with(|c: char| c.is_alphanumeric() || matches!(c, ')' | '.' | '!'))
                && !ends_with_operator_word(left)
        }
        None => text.is_empty() || text.ends_with('('),
    }
}

/// The words that fend-core 1.5.8 reads as operators (`lexer.rs`).
const OPERATOR_WORDS: [&str; 13] = [
    "to", "as", "in", "per", "of", "mod", "and", "or", "xor", "choose", "nCr", "permute", "nPr",
];

/// Whether `text` ends with one of fend's operator words, in any case. fend
/// starts a word after the digits of a number, so `5mod` ends with `mod`.
fn ends_with_operator_word(text: &str) -> bool {
    let before = text.trim_end_matches(char::is_alphanumeric);
    let word = text[before.len()..].trim_start_matches(|c: char| c.is_ascii_digit());
    OPERATOR_WORDS
        .iter()
        .any(|operator| word.eq_ignore_ascii_case(operator))
}

/// Whether a value before `text` stands on its own: at the end, or before
/// `)`, `+`, `-`, `/`, or a `*` that is not a power.
fn ends_value(text: &str) -> bool {
    let text = text.trim_start();
    text.is_empty()
        || text.starts_with([')', '+', '-', '/'])
        || (text.starts_with('*') && !text.starts_with("**"))
}

/// Whether `input` has a percentage with something else, which fend reads
/// differently from people: `100 + 10% + 5` as 105.1, `200 / 10%` as 20%. A
/// percentage alone, such as `2e-3%`, and a remainder, such as `10 % 3`, are
/// fine.
fn mixes_percentage(input: &str) -> bool {
    // `20 percent` is `20 %` by now, but `(20 percent)` is not.
    if input.to_ascii_lowercase().contains("percent") {
        return true;
    }
    let alone = before_percentage(input.trim())
        .is_some_and(|before| matches!(before.trim(), "" | "+" | "-"));
    // fend reads a `%` before a number or `(` as a remainder.
    !alone
        && input.match_indices('%').any(|(at, _)| {
            !input[at + 1..]
                .trim_start()
                .starts_with(|c: char| c.is_ascii_digit() || matches!(c, '.' | '('))
        })
}

/// The text before the percentage, such as `10%` or `1e-2 %`, that ends
/// `text`.
fn before_percentage(text: &str) -> Option<&str> {
    let number = text.strip_suffix('%')?.trim_end();
    let digit = |c: char| c.is_ascii_digit() || matches!(c, '.' | ',');
    let mut rest = number.trim_end_matches(digit);
    // In `1e-2`, the sign belongs to the exponent.
    if let Some(mantissa) = rest
        .strip_suffix(['+', '-'])
        .unwrap_or(rest)
        .strip_suffix(['e', 'E'])
        && mantissa.ends_with(|c: char| c.is_ascii_digit())
    {
        rest = mantissa.trim_end_matches(digit);
    }
    number[rest.len()..]
        .contains(|c: char| c.is_ascii_digit())
        .then_some(rest)
}

/// Whether `text` is a plain amount: a number with an optional sign,
/// currency sign, and unit, such as `200`, `-$5`, or `5 kg`.
fn is_amount(text: &str) -> bool {
    let text = text.trim();
    amount_len(text) == Some(text.len())
}

/// The length of the plain amount at the start of `text`.
fn amount_len(text: &str) -> Option<usize> {
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    let number = unsigned.strip_prefix(['$', '£', '¥']).unwrap_or(unsigned);
    let mut rest = &number[number_len(number)?..];
    // A unit is a word after a space, such as `kg` or `EUR`: in `2e-3`, `e`
    // is part of the number.
    let unit = rest.trim_start();
    let letters = unit
        .find(|c: char| !c.is_alphabetic())
        .unwrap_or(unit.len());
    if letters > 0 && unit.len() < rest.len() && ends_token(&unit[letters..]) {
        rest = &unit[letters..];
    }
    Some(text.len() - rest.len())
}

/// The length of the plain number at the start of `text`, such as `20`,
/// `12.5`, or `1,000`, when fend's number ends there too: not in `2e3`,
/// `2(`, or `2^3`.
fn number_len(text: &str) -> Option<usize> {
    let len = text
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | ',')))
        .unwrap_or(text.len());
    (is_number(&text[..len]) && !text.starts_with(',') && ends_token(&text[len..])).then_some(len)
}

/// Whether a number or a word ends before `rest`.
fn ends_token(rest: &str) -> bool {
    rest.chars()
        .next()
        .is_none_or(|c| c.is_whitespace() || "+-*/)%".contains(c))
}

/// The text before `suffix`, matched in any case, when some is left.
fn strip_suffix<'a>(text: &'a str, suffix: &str) -> Option<&'a str> {
    let at = text.len().checked_sub(suffix.len())?;
    if !text.is_char_boundary(at) || !text[at..].eq_ignore_ascii_case(suffix) {
        return None;
    }
    Some(text[..at].trim()).filter(|left| !left.is_empty())
}

/// The text before and after the first `separator`, matched in any case,
/// when both are there.
fn around<'a>(text: &'a str, separator: &str) -> Option<(&'a str, &'a str)> {
    // ASCII lowercase keeps every byte offset.
    let at = text.to_ascii_lowercase().find(separator)?;
    let (left, right) = (text[..at].trim(), text[at + separator.len()..].trim());
    (!left.is_empty() && !right.is_empty()).then_some((left, right))
}

/// A plain number, such as `20`, `12.5`, or `1,000`.
fn is_number(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_digit())
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | ','))
}

struct Deadline(Instant);

impl fend_core::Interrupt for Deadline {
    fn should_interrupt(&self) -> bool {
        Instant::now() >= self.0
    }
}

struct RateLookup {
    rates: Rates,
    used: Arc<AtomicBool>,
}

struct Probe {
    asked: Arc<AtomicBool>,
}

impl fend_core::ExchangeRateFnV2 for Probe {
    fn relative_to_base_currency(
        &self,
        _: &str,
        _: &fend_core::ExchangeRateFnV2Options,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        self.asked.store(true, Ordering::Relaxed);
        Ok(1.0)
    }
}

impl fend_core::ExchangeRateFnV2 for RateLookup {
    fn relative_to_base_currency(
        &self,
        currency: &str,
        _: &fend_core::ExchangeRateFnV2Options,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        self.used.store(true, Ordering::Relaxed);
        self.rates
            .per_euro(currency)
            .ok_or_else(|| format!("No exchange rate for {currency}").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_lists_and_filters_copied_calculations() {
        let entry = |id, expression: &str, result: &str| Calculation {
            id,
            expression: expression.into(),
            result: result.into(),
        };
        let history = CalculationHistory::new(vec![
            entry(2, "5 ft to cm", "152.4 cm"),
            entry(1, "12 * 8", "96"),
        ]);
        let titles = |query: &str| -> Option<Vec<String>> {
            history
                .answer(query)
                .map(|results| results.into_iter().map(|r| r.title).collect())
        };
        assert_eq!(titles("="), Some(vec!["152.4 cm".into(), "96".into()]));
        assert_eq!(titles("= CM"), Some(vec!["152.4 cm".into()]));
        assert_eq!(titles("= 12 8"), Some(vec!["96".into()]));
        assert_eq!(titles("= nothing"), Some(vec![]));
        assert_eq!(titles("12 * 8"), None);
        assert_eq!(titles("=12*8"), None);
        let row = &history.answer("= 96").unwrap()[0];
        let labels: Vec<_> = row.actions.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Copy Result", "Copy Expression", "Delete from History"]
        );
    }

    #[test]
    fn copying_an_answer_evaluates_it_again() {
        let answer = answer("12 * 8", None).unwrap();
        assert_eq!(
            answer.actions[0].action,
            Action::CopyCalculation {
                query: "12 * 8".into()
            }
        );
        assert_eq!(result("12 * 8", None).as_deref(), Some("96"));
        assert_eq!(result("hello", None), None);
    }

    #[test]
    fn a_leading_equals_sign_is_a_calculation() {
        assert_eq!(expression("=12*8"), "12*8");
        assert_eq!(expression("12*8"), "12*8");
        assert_eq!(expression("= 12*8"), "= 12*8");
        assert_eq!(expression("="), "=");
        let answer = answer(expression("=12*8"), None).unwrap();
        assert_eq!(
            answer.actions[0].action,
            Action::CopyCalculation {
                query: "12*8".into()
            }
        );
    }

    #[test]
    fn a_history_row_copies_its_saved_result() {
        let history = CalculationHistory::new(vec![Calculation {
            id: 7,
            expression: "12 * 8".into(),
            result: "96".into(),
        }]);
        let row = &history.answer("=").unwrap()[0];
        assert_eq!(
            row.actions[0].action,
            Action::CopySavedCalculation { id: 7 }
        );
        assert_eq!(
            history.get(7).map(|entry| entry.result.as_str()),
            Some("96")
        );
        assert_eq!(history.get(8), None);
    }

    fn value(query: &str) -> Option<String> {
        evaluate(query, None).map(|(value, _)| value)
    }

    #[test]
    fn evaluates_arithmetic_and_units() {
        assert_eq!(value("12 * 8").as_deref(), Some("96"));
        assert_eq!(value("sqrt(144)").as_deref(), Some("12"));
        assert_eq!(value("8 Mbps to MBps").as_deref(), Some("1 MBps"));
        assert!(value("5 ft to cm").is_some_and(|v| v.starts_with("152.4")));
    }

    #[test]
    fn ignores_text_bare_numbers_and_errors() {
        assert_eq!(value("safari"), None);
        assert_eq!(value("42"), None);
        for echo in ["30 percent", "20percent", "\u{2212}5", "2kg"] {
            assert_eq!(value(echo), None, "{echo}");
        }
        assert_eq!(value("1password"), None);
        assert_eq!(value("12 +"), None);
    }

    #[test]
    fn leaves_dates_and_clock_times_to_the_date_answers() {
        assert_eq!(value("2026-10-17"), None);
        assert_eq!(value("3pm"), None);
        assert_eq!(value("10am"), None);
        assert_eq!(value("10:30 PM"), None);
        assert_eq!(value("2026 - 10").as_deref(), Some("2016"));
        assert_eq!(value("2026-10").as_deref(), Some("2016"));
        assert_eq!(value("12-3-4").as_deref(), Some("5"));
        assert_eq!(value("100-5-3").as_deref(), Some("92"));
        assert_eq!(value("2026-1-5").as_deref(), Some("2020"));
    }

    #[test]
    fn offers_to_turn_on_rates_only_for_currency_queries() {
        let answer = rates_off_answer("100 usd to eur").unwrap();
        assert_eq!(answer.actions[0].action, Action::TurnOnCurrencyRates);
        assert!(rates_off_answer("100 USD MYR").is_some());
        for query in ["12 * 8", "5 ft to cm", "usd", "hello"] {
            assert!(rates_off_answer(query).is_none(), "{query}");
        }
    }

    #[test]
    fn converts_currency_with_cached_rates() {
        let rates = Rates::fixture();
        let (value, used) = evaluate("100 USD MYR", Some(&rates)).unwrap();
        assert!(used);
        assert_eq!(value, "401.71 MYR");
        let answer = answer("100 USD to MYR", Some(&rates)).unwrap();
        assert!(answer.subtitle.contains("2026-10-02"));
        assert_eq!(evaluate("100 USD to MYR", None), None);
    }

    #[test]
    fn reads_percentage_phrases() {
        for (query, expected) in [
            ("30 + 20%", "36"),
            ("200 - 10%", "180"),
            ("5 kg + 10%", "5.5 kg"),
            ("10% of 200", "20"),
            ("10% on 200", "220"),
            ("20 is what % of 200", "10%"),
            ("20 as a % of 200", "10%"),
            ("180 is what % off 200", "10%"),
            ("180 is what % on 150", "20%"),
            ("50 to 75 is what %", "50%"),
            ("40 to 90 as %", "125%"),
            ("3/20 is what %", "15%"),
            ("0.35 as %", "35%"),
            ("20 is 10% of what", "200"),
            ("220 is 10% on what", "200"),
            ("180 is 10% off what", "200"),
        ] {
            assert_eq!(value(query).as_deref(), Some(expected), "{query}");
        }
        assert_eq!(value("10% of what"), None);
        assert_eq!(value("2e-3%").as_deref(), Some("0.002%"));
        // Not a char boundary where a suffix would start.
        assert_eq!(value("2 Äpfel"), None);
        assert_eq!(value("5 €€"), None);
        // fend needs rates for any amount of money.
        let rates = Rates::fixture();
        for (query, expected) in [
            ("$30 + 20%", "$36"),
            ("30% of $200", "$60"),
            ("15% OFF $200", "$170"),
        ] {
            let (value, _) = evaluate(query, Some(&rates)).unwrap();
            assert_eq!(value, expected, "{query}");
        }
        assert!(rates_off_answer("$30 + 20%").is_some());
    }

    #[test]
    fn takes_percentages_in_turn() {
        for (query, expected) in [
            ("200 - 10% - 5%", "171"),
            ("100 + 10% + 20%", "132"),
            ("30 plus 20 percent", "36"),
            ("30 + 20Percent", "36"),
            ("20 is what percent of 200", "10%"),
            // A remainder, not a percentage.
            ("5 + 10 % 3", "6"),
        ] {
            assert_eq!(value(query).as_deref(), Some(expected), "{query}");
        }
    }

    #[test]
    fn gives_no_answer_for_a_percentage_in_another_shape() {
        for query in [
            // fend adds a hundredth: 100 + 0.1 + 5.
            "100 + 10% + 5",
            "(100 + 10%) * 2",
            "round(200 - 10%)",
            "100 - (10%)",
            "100 - 2 * 5%",
            "5% + 100 + 10%",
            "50 * 2 + 10%",
            "100 * 10% + 20%",
            "5 + 10% to fraction",
            "2 dozen 10%",
            // The percentage is not a plain number.
            "5 - 1e-2%",
            "10 - -5%",
            // fend reads `200 / 10%` as (200 / 10)%, which is 20%.
            "200 / 10%",
            "200 * 10%",
            "10% + 20%",
            "(10% + 20%) * 2",
            "5 * -10%",
            // A part of the question is not a plain amount.
            "50 * 2 is what % of 400",
            "1 + 2 is what %",
            // `percent` that is not a word of its own.
            "100 - (20 percent)",
            "5+20percent",
        ] {
            assert_eq!(value(query), None, "{query}");
        }
        assert!(rates_off_answer("$30 + 20% + 5").is_none());
    }

    #[test]
    fn a_percentage_of_is_one_value() {
        for (query, expected) in [
            ("10% of 200 + 50", "70"),
            ("15% of 80 + 80", "92"),
            ("10% off 200 + 5", "185"),
            ("10% on 200 * 2", "440"),
            ("10% of 200 / 4", "5"),
            ("5 + 10% of 200", "25"),
            ("100 - 10% off 200", "-80"),
            ("3 * 10% of 200", "60"),
            ("(10% of 200) / 4", "5"),
            ("10 percent of 200", "20"),
            ("10% of 200 per 4", "5"),
            // A phrase right after another one.
            ("10% of 200 + 20% of 300", "80"),
            ("10% of 200 * 10% of 50", "100"),
            ("10% off 200 - 5% off 100", "85"),
            ("5 + 10% of 200 + 10% of 300", "55"),
        ] {
            assert_eq!(value(query).as_deref(), Some(expected), "{query}");
        }
        for query in [
            // After `/` or a power, fend takes the percentage of all that
            // comes before it: 1/2% of 200 is 1.
            "1/2% of 200",
            "400 / 10% of 200",
            "400 / 10% off 200",
            "400 / -10% off 200",
            "2 ^ 10% on 200",
            "2 ^ -10% of 200",
            "2 ** 10% of 200",
            "10% of 200 / 10% of 50",
            "10% of 200 ^ 10% of 50",
            // `per` is fend's `/`. After it or another operator word, such
            // as `mod`, the sign belongs to the percentage.
            "400 per -10% off 200",
            "400 per +10% off 200",
            "400 mod +10% off 200",
            "5 choose +10% of 20",
            // A sign or a value right before the percentage.
            "10% of 200 - -10% of 50",
            "-10% off 200",
            "2 10% of 200",
            "1e-2% of 200",
            // More than a plain amount after it.
            "10% of 2^3",
            "10% of 5!",
            "10% of 200 mod 3",
            "10% of 20% of 200",
            "10% of 200 - 5%",
        ] {
            assert_eq!(value(query), None, "{query}");
        }
        let rates = Rates::fixture();
        for (query, expected) in [
            ("20% off $50 + $5", "$45"),
            ("30% of -$200", "$-60"),
            ("10% on  $50", "$55"),
        ] {
            let (value, _) = evaluate(query, Some(&rates)).unwrap();
            assert_eq!(value, expected, "{query}");
        }
    }

    #[test]
    fn an_operator_word_is_not_a_value() {
        for word in [
            "to", "as", "in", "per", "of", "mod", "and", "AND", "or", "OR", "xor", "XOR", "choose",
            "nCr", "permute", "nPr",
        ] {
            assert!(!starts_value(&format!("400 {word} -")), "{word}");
            assert!(!starts_value(&format!("400{word}+")), "{word}");
        }
        // A word that only ends like one, such as `min`, is a value.
        for text in ["400 -", "5 kg *", "(2) +", "5 min -", "x2mod -"] {
            assert!(starts_value(text), "{text}");
        }
    }

    #[test]
    fn reads_other_spellings_of_signs() {
        assert_eq!(ascii_signs("1\u{2212}2×3✕4÷5∕6≠7λ"), "1-2 *3 *4/5/6!=7\\");
        for (query, expected) in [
            ("5 × 3", "15"),
            ("5 ×* 3", "125"),
            ("12 ÷ 4", "3"),
            ("100 \u{2212} 10%", "90"),
            ("1e\u{2212}2 * 100", "1"),
        ] {
            assert_eq!(value(query).as_deref(), Some(expected), "{query}");
        }
        for query in [
            // Two products, not a power.
            "5 ×× 3",
            "5 *× 3",
            "100 \u{2212} 10% + 5",
            "100 ÷ 10% off 200",
            "2 ×* 10% on 200",
        ] {
            assert_eq!(value(query), None, "{query}");
        }
    }

    #[test]
    fn refuses_lambdas() {
        for query in [
            "(\\x.x + 1) 5",
            "(\u{3bb}x.x + 1) 5",
            "(x: x + 1) 5",
            "(x => x * 2) 5",
            "f = x: x * 2; f 5",
            // Each calls itself until the stack overflows.
            "(x: x x)(x: x x) 1",
            "(x => x x)(x => x x) 1",
            "f = x: f x; f 1",
        ] {
            assert_eq!(value(query), None, "{query}");
            assert!(rates_off_answer(query).is_none(), "{query}");
        }
    }

    #[test]
    fn refuses_input_that_fend_cannot_parse_fast() {
        let start = Instant::now();
        for query in [
            // Too deep for the stack.
            format!("1{}", ")".repeat(5000)),
            format!("{}1", "-".repeat(5000)),
            format!("{}1", "1+".repeat(300)),
            format!("100{}", " - 1%".repeat(100_000)),
            // 33 parentheses, and 501 characters.
            format!("{}10% of 200{}", "(".repeat(17), ")".repeat(16)),
            format!("1 +{}1", "\u{3000}".repeat(497)),
            // Short, but its rewrite nests too deep.
            format!("100{}", " - 1%".repeat(70)),
            // Parsed again and again.
            "1(".repeat(30),
            "1+1+1+1+1+1+1+(".repeat(8),
            format!("{}(", "1\u{2212}".repeat(15)).repeat(7),
            "1 2/(".repeat(16),
            "1 2^-(".repeat(16),
            // When the group fails, fend reads each `/` again as `1/x`.
            "(2/".repeat(30),
            // A lambda: fend parses its body again for each reading around
            // it. `λ` is fend's other spelling of `\`.
            "1 \\x.1+(".repeat(11),
            format!("{}1", "1 \u{3bb}x.".repeat(30)),
            "1 \u{3bb}x.1+(".repeat(12),
        ] {
            assert_eq!(value(&query), None);
            assert!(rates_off_answer(&query).is_none());
        }
        assert!(start.elapsed() < Duration::from_secs(1));
        // What people type still has an answer.
        for (query, expected) in [
            (format!("{}1", "1.5+".repeat(40)), "61"),
            ("(((2+3)*(4-1)+1)/(5-(2+1)))".into(), "8"),
            ("((1+2)*(3+4))*((5+6)-(7+8))".into(), "-84"),
            ("1 - (2 - (3 - (4 - (5 - (6 - 7)))))".into(), "4"),
            ("1-(1-(1-(1-(1-(1-1)))))".into(), "1"),
            (
                "1 + 1/(1 + 1/(1 + 1/(1 + 1/(1 + 1/(1 + 1)))))".into(),
                "approx. 1.6153846154",
            ),
            ("200 - 10% - 5% - 2% - 1%".into(), "165.9042"),
            // The limits count what the user typed, not the rewrite.
            ("200 - 1% - 1% - 1% - 1% - 1% - 1%".into(), "188.2960298802"),
            (
                format!("{}10% of 200{}", "(".repeat(16), ")".repeat(16)),
                "20",
            ),
            // 499 characters, but 1,489 bytes.
            (format!("1 +{}1", "\u{3000}".repeat(495)), "2"),
        ] {
            assert_eq!(value(&query).as_deref(), Some(expected), "{query}");
        }
    }

    #[test]
    fn reads_operator_words_and_euro_signs() {
        for (query, expected) in [
            ("10 plus 5", "15"),
            ("10 minus 4", "6"),
            ("10 Times 3", "30"),
            ("3 multiplied by 4", "12"),
            ("1,000 divided by 200", "5"),
            ("30 plus 20%", "36"),
        ] {
            assert_eq!(value(query).as_deref(), Some(expected), "{query}");
        }
        assert_eq!(value("10 divided 4"), None);
        assert_eq!(euro_amounts("€20 + €5k"), "20 EUR + 5k EUR");
        assert_eq!(euro_amounts("€ and €x"), "€ and €x");
        let rates = Rates::fixture();
        let (value, used) = evaluate("€20 + 10%", Some(&rates)).unwrap();
        assert!(used);
        assert_eq!(value, "22.00 EUR");
        assert!(rates_off_answer("€20 to usd").is_some());
    }

    #[test]
    fn rounds_money_to_cents() {
        assert_eq!(
            money("approx. 401.7094017094 MYR").as_deref(),
            Some("401.71 MYR")
        );
        assert_eq!(money("400 MYR").as_deref(), Some("400.00 MYR"));
        assert_eq!(money("MYR"), None);
    }

    #[test]
    fn drops_answers_that_fend_computes_through_an_f64() {
        for query in [
            "floor(1e20)",
            "ceil(1e25)",
            "round(123456789012345678901.4)",
            "floor(-1e20)",
            "FLOOR(1e20 m)",
            "floor(10000000000000000001)",
            "round(9007199254740993)",
            "floor(12345678901234567.5)",
            "ceil(1e20) to hex",
            "floor(9007199254740993) to hex",
            "floor(1e20) + 1",
            "floor(1e20) * 2",
            "sinh(50)",
            "Cosh(50)",
            // Wrong in decimal, but all 0s and 1s.
            "round(10000000000000001)",
            // Correct, but dropped by the 14-digit rule.
            "round(10000) to binary",
        ] {
            assert_eq!(value(query), None, "{query}");
        }
        assert_eq!(
            value("round(1234567890123.4)").as_deref(),
            Some("1234567890123")
        );
        assert_eq!(value("floor(3.5 km)").as_deref(), Some("3 km"));
        assert_eq!(value("2^64").as_deref(), Some("18446744073709551616"));
        assert_eq!(
            value("sinh(40)").as_deref(),
            Some("approx. 117692633418510000")
        );
    }

    #[test]
    fn keeps_exact_currency_fractions() {
        let rates = Rates::fixture();
        let (value, _) = evaluate("100 USD to MYR in exact", Some(&rates)).unwrap();
        assert_eq!(value, "401 83/117 MYR");
        assert_eq!(money("401 83/117 MYR"), None);
        let (value, _) = evaluate("100 USD to MYR in fraction", Some(&rates)).unwrap();
        assert_eq!(value, "47000/117 MYR");
    }

    #[test]
    fn rounds_currency_per_unit() {
        let rates = Rates::fixture();
        let value = |query| evaluate(query, Some(&rates)).unwrap().0;
        assert_eq!(value("10 USD/kg to MYR/kg"), "40.17 MYR / kg");
        assert_eq!(value("10 USD per kg to MYR per kg"), "40.17 MYR / kg");
    }

    #[test]
    fn keeps_significant_digits_of_small_amounts() {
        let rates = Rates::parse(
            br#"{"amount":1.0,"base":"EUR","date":"2026-10-02","rates":{"USD":1.17,"IDR":18850.0,"JPY":175.5}}"#,
            0,
        )
        .unwrap();
        let value = |query| evaluate(query, Some(&rates)).unwrap().0;
        assert_eq!(value("1 IDR to EUR"), "0.0000531 EUR");
        assert_eq!(value("1 JPY to USD"), "0.00667 USD");
        assert_eq!(value("100 USD to EUR"), "85.47 EUR");
        assert_eq!(money("0.5 EUR").as_deref(), Some("0.500 EUR"));
        assert_eq!(money("0 EUR").as_deref(), Some("0.00 EUR"));
        assert_eq!(money("-0.004 EUR").as_deref(), Some("-0.00400 EUR"));
    }
}
