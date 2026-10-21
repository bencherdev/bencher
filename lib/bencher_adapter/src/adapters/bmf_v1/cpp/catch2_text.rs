use std::{borrow::Cow, rc::Rc};

use bencher_json::project::report::Adapter;

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        cpp::catch2::{Statistics, insert},
        leaf::LeafResults,
    },
    results::adapter_results::AdapterResults,
};

/// Catch2's console reporter, read cell by cell as its table printer wrote each benchmark,
/// under its test case and section headers. The text states no confidence level, so the
/// interval names carry none.
///
/// Every line is placed as Catch2 printed it or counted: a line the printer cannot place, which
/// only the program's own output inside a table can make, skips and counts the benchmark it
/// falls in, else goes with the benchmark before it that lost step where it can only continue
/// one, else counts as a benchmark of its own, so that no benchmark is lost uncounted. A print
/// that is byte for byte a row of Catch2's, a value cell padded as Catch2 pads one, or a line of
/// dashes as long as the rule reads as Catch2's own. Colour codes are read past, and where the
/// console is too narrow for the table's title, every benchmark is counted and none kept.
pub struct AdapterCppCatch2Text;

impl Adaptable for AdapterCppCatch2Text {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut results = LeafResults::default();
        for benchmark in reading(input).benchmarks {
            if let Some(Benchmark {
                path,
                name,
                statistics,
            }) = benchmark
            {
                insert(&mut results, &path, &name, &statistics);
            } else {
                results.skip_benchmark();
            }
        }
        results.finish_with_skips(Adapter::CppCatch2Text)
    }
}

/// The reading of Catch2's console output, with its colour codes taken out.
fn reading(input: &str) -> Reading {
    let input = strip_ansi(input);
    Printer::new(&input).read()
}

/// The text without the ANSI escape sequences that `--colour-mode ansi` writes: an escape, `[`,
/// any parameter and intermediate bytes, and a final byte.
fn strip_ansi(input: &str) -> Cow<'_, str> {
    if !input.contains('\u{1b}') {
        return Cow::Borrowed(input);
    }
    let mut text = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.next_if_eq(&'[').is_some() {
            while chars
                .next_if(|c| ('\u{20}'..='\u{3f}').contains(c))
                .is_some()
            {}
            chars.next_if(|c| ('\u{40}'..='\u{7e}').contains(c));
        } else {
            text.push(c);
        }
    }
    Cow::Owned(text)
}

/// Every benchmark in the order Catch2 printed it, `None` for one skipped and counted, and the
/// lines the printer could not place, each counted with a skipped benchmark.
struct Reading {
    benchmarks: Vec<Option<Benchmark>>,
    #[cfg(test)]
    unplaced: Vec<usize>,
    #[cfg(test)]
    exhausted: bool,
}

struct Benchmark {
    path: Vec<String>,
    name: String,
    statistics: Statistics<'static>,
}

/// A benchmark read from its first line to its last, and whether the printer lost step in it.
struct Outcome {
    start: usize,
    end: usize,
    lost_step: bool,
    benchmark: Option<Benchmark>,
}

/// Catch2's console table printer, replayed over its output: the column it last wrote, which
/// sets each cell's width, and where in the text its next cell starts, with how much of each
/// line it placed, and what a reading it only tries changed there. For each line, it knows the
/// next that is not empty and the next rule or table header. The reader stops after a number of
/// steps proportional to the input's lines, leaving the rest unplaced and so counted.
struct Printer<'a> {
    lines: Vec<&'a str>,
    next_text: Vec<usize>,
    next_break: Vec<usize>,
    at: At,
    covered: Vec<usize>,
    placed: Vec<bool>,
    undo: Vec<Undo>,
    trials: usize,
    rule: usize,
    wraps_bytes: bool,
    last_table: Option<Table>,
    trying: bool,
    narrow: bool,
    steps: usize,
    budget: usize,
}

/// Where the printer is, which a reading it only tries puts back.
#[derive(Clone)]
struct At {
    line: usize,
    pos: usize,
    column: i8,
    table: Option<Table>,
    path: Rc<[String]>,
}

/// A line's cover and placement before a reading that is only tried changed them.
struct Undo {
    line: usize,
    covered: usize,
    placed: bool,
}

impl<'a> Printer<'a> {
    fn new(input: &'a str) -> Self {
        let lines: Vec<&str> = input.lines().collect();
        // The run's banner is a line of `~` as long as Catch2's rule.
        let rule = lines
            .iter()
            .find(|line| !line.is_empty() && line.bytes().all(|byte| byte == b'~'))
            .map_or(0, |line| line.len());
        // Catch2 v2's `TextFlow` wraps by bytes, where v3's wraps by characters.
        let wraps_bytes = lines.iter().any(|line| line.contains(" is a Catch v2."));
        let at = At {
            line: 0,
            pos: 0,
            column: -1,
            table: None,
            path: Rc::from([]),
        };
        let budget = STEPS_PER_LINE * (lines.len() + 1);
        let mut printer = Self {
            covered: vec![0; lines.len()],
            placed: vec![false; lines.len()],
            next_text: Vec::new(),
            next_break: Vec::new(),
            lines,
            at,
            undo: Vec::new(),
            trials: 0,
            rule,
            wraps_bytes,
            last_table: None,
            trying: false,
            narrow: false,
            steps: 0,
            budget,
        };
        let end = printer.lines.len();
        let (mut text, mut rule) = (end, end);
        let (mut next_text, mut next_break) = (vec![end], vec![end]);
        for line in (0..end).rev() {
            if !printer.line(line).is_empty() {
                text = line;
            }
            let header = printer.table_header(line);
            printer.narrow |= header.is_some_and(|(table, _)| table.wrapped);
            if printer.is_rule(printer.line(line)) || header.is_some() {
                rule = line;
            }
            next_text.push(text);
            next_break.push(rule);
        }
        next_text.reverse();
        next_break.reverse();
        (printer.next_text, printer.next_break) = (next_text, next_break);
        printer
    }

    fn read(mut self) -> Reading {
        let mut outcomes: Vec<Outcome> = Vec::new();
        while let Some(first) = self.cell(Kind::Name, Within::Run) {
            let start = self.at.line;
            // Cells that end a row on a line of their own after a benchmark are the rest of its
            // row, which the program's own output moved there.
            if let Cell::Bad = first
                && let Some(previous) = outcomes.last_mut()
                && previous.end + 1 == start
                && self.is_fragment(start)
            {
                previous.benchmark = None;
                previous.lost_step = true;
                previous.end = self.resync(start).saturating_sub(1);
                continue;
            }
            let path = self.at.path.to_vec();
            let read = self.benchmark(first);
            let lost_step = matches!(read, Err(Stop::Unreadable));
            let end = if lost_step {
                self.resync(start).saturating_sub(1).max(start)
            } else {
                self.at.line.max(start)
            };
            outcomes.push(Outcome {
                start,
                end,
                lost_step,
                benchmark: read.ok().map(|(name, statistics)| Benchmark {
                    path,
                    name,
                    statistics,
                }),
            });
        }
        let unplaced: Vec<usize> = (0..self.lines.len())
            .filter(|&line| !self.is_placed(line))
            .collect();
        let lines: Vec<(usize, bool)> = unplaced
            .iter()
            .map(|&line| (line, self.continues(line)))
            .collect();
        count(&mut outcomes, &lines);
        Reading {
            benchmarks: outcomes
                .into_iter()
                .map(|outcome| outcome.benchmark.filter(|_| !self.narrow))
                .collect(),
            #[cfg(test)]
            unplaced,
            #[cfg(test)]
            exhausted: self.steps > self.budget,
        }
    }

    /// Whether a line can only go on with a benchmark started before it: the rest of a row on a
    /// line of its own, a row whose first column is empty and whose next holds no samples, or a
    /// row after one that holds only a line of a wrapped name.
    fn continues(&self, line: usize) -> bool {
        let Some(table) = self.last_table else {
            return self.is_fragment(line);
        };
        let (text, before) = (
            self.line(line),
            line.checked_sub(1).map(|line| self.line(line)),
        );
        let first = |text: &'a str| text.get(..table.name_width).unwrap_or(text);
        let samples = text
            .get(table.name_width..)
            .and_then(|rest| {
                value_cell(rest, COLUMN_WIDTH, Justify::Right).and_then(|length| rest.get(..length))
            })
            .is_some_and(|cell| cell.trim().parse::<u64>().is_ok());
        let wrapped = before.is_some_and(|before| {
            before.len() == table.width()
                && !first(before).trim().is_empty()
                && before
                    .get(table.name_width..)
                    .is_some_and(|rest| rest.bytes().all(|byte| byte == b' '))
        });
        self.is_fragment(line) || (first(text).trim().is_empty() && !samples) || wrapped
    }

    /// The name and statistics of a completed benchmark, wherever in the table's columns Catch2
    /// printed it, after all of its cells, so that the next benchmark starts in step.
    fn benchmark(&mut self, first: Cell<'a>) -> Result<(String, Statistics<'static>), Stop> {
        let text = first.text()?;
        let mut name = Vec::new();
        // Catch2 prints no name line for an empty name, so its first cell is the samples.
        let samples = if self.unnamed(text) {
            text
        } else {
            name.push(text);
            loop {
                let cell = self.text(Kind::Value)?;
                if !cell.is_empty() {
                    break cell;
                }
                // A wrapped name's next line follows two more empty cells.
                for _ in 0..2 {
                    self.text(Kind::Empty)?;
                }
                name.push(self.text(Kind::Name)?);
            }
        };
        let iterations = self.text(Kind::Value)?;
        let table = self.at.table.ok_or(Stop::Failed)?;
        let statistics = if table.analysis {
            let mut cells = [""; 13];
            for (cell, kind) in cells.iter_mut().zip(ANALYSIS) {
                *cell = self.text(kind)?;
            }
            analysis(cells)
        } else {
            duration(self.text(Kind::Value)?).map(|mean| Statistics {
                mean: [mean; 3],
                std_dev: None,
                level: None,
            })
        };
        if samples.parse::<u64>().is_err() || iterations.parse::<u64>().is_err() {
            return Err(Stop::Failed);
        }
        let width = table.name_width.saturating_sub(2);
        let lines: Vec<_> = name.into_iter().map(|line| (line, width)).collect();
        Ok((
            join(&lines, self.wraps_bytes),
            statistics.ok_or(Stop::Failed)?,
        ))
    }

    /// Whether a benchmark's first cell is the samples of a benchmark with no name: a count,
    /// followed by the iterations and a duration rather than by the samples and iterations.
    fn unnamed(&mut self, first: &str) -> bool {
        if first.parse::<u64>().is_err() {
            return false;
        }
        let trial = self.try_from();
        let unnamed = matches!(
            (self.text(Kind::Value), self.text(Kind::Value)),
            (Ok(iterations), Ok(duration_cell))
                if iterations.parse::<u64>().is_ok() && duration(duration_cell).is_some()
        );
        self.put_back(trial);
        unnamed
    }

    /// A cell of the benchmark being read: a failure, a test case or section header, which
    /// Catch2 v2 prints after an assertion inside the samples with no failure, or the end of the
    /// output ends it, and a cell Catch2 did not print loses step.
    fn text(&mut self, kind: Kind) -> Result<&'a str, Stop> {
        self.cell(kind, Within::Benchmark)
            .ok_or(Stop::Failed)?
            .text()
    }

    /// Catch2's next cell, through any table it closed and opened again on the way.
    fn cell(&mut self, kind: Kind, within: Within) -> Option<Cell<'a>> {
        while self.step() {
            if let Some(table) = self.at.table {
                if self.column_break(table, within) {
                    return Some(self.read_cell(table, kind));
                }
            } else if let Within::Benchmark = within
                && self.header_next()
            {
                return Some(Cell::Failed);
            } else {
                self.open()?;
            }
        }
        None
    }

    /// One step of the reader, which every loop of it takes, and whether the reader has steps
    /// left.
    fn step(&mut self) -> bool {
        self.steps += 1;
        self.steps <= self.budget
    }

    /// After a benchmark that lost step, the printer leaves the table: its rows are read again
    /// from the next one that a benchmark reads cleanly from.
    fn resync(&mut self, start: usize) -> usize {
        let line = self.at.line.max(start + 1);
        self.save(line);
        if let Some(covered) = self.covered.get_mut(line) {
            *covered = 0;
        }
        (self.at.line, self.at.pos, self.at.column, self.at.table) = (line, 0, -1, None);
        line
    }

    /// Catch2's `ColumnBreak`, which wraps the line after the last column, or `false` where the
    /// text shows that Catch2 closed the table: `close` ends the line with a `RowBreak`, which
    /// writes a newline only past the first column, then writes a newline of its own.
    fn column_break(&mut self, table: Table, within: Within) -> bool {
        let exhausted = self.at.pos >= self.current().len();
        // A row that ends before its last column inside a benchmark ends at the program's own
        // newline, and Catch2's row goes on at the next line that holds the rest of it.
        if let Within::Benchmark = within
            && exhausted
            && (1..LAST_COLUMN).contains(&self.at.column)
            && let Some(line) = self.row_after(self.at.line + 1, self.at.column + 1, table)
        {
            (self.at.line, self.at.pos) = (line, 0);
            self.at.column += 1;
            return true;
        }
        if self.at.column == 0 && exhausted {
            // `close` at the first column writes no `RowBreak`, so the column stays.
            self.next_line();
            self.at.table = None;
            return false;
        }
        if self.at.column == LAST_COLUMN || (self.at.column > 0 && exhausted) {
            self.next_line();
            self.at.column = -1;
        }
        // Inside a benchmark, an empty line that a row of Catch2's follows is the program's own.
        while let Within::Benchmark = within
            && self.at.column == -1
            && self.current().is_empty()
            && self.row_follows()
        {
            self.next_line();
        }
        if self.at.column == -1 && self.current().is_empty() {
            return self.close();
        }
        // Catch2 prints a rule or a table's header only past `close`.
        if self.at.column == -1
            && (self.is_rule(self.current()) || self.table_header(self.at.line).is_some())
        {
            self.at.table = None;
            return false;
        }
        self.at.column += 1;
        true
    }

    /// The first line from `start` that is not empty, where it holds exactly Catch2's cells from
    /// `column` to the last.
    fn row_after(&self, start: usize, column: i8, table: Table) -> Option<usize> {
        let line = *self.next_text.get(start)?;
        let text = self.lines.get(line)?;
        (row_length(text, column, table.name_width) == Some(text.len())).then_some(line)
    }

    /// Whether the next line that is not empty is a row of Catch2's.
    fn row_follows(&self) -> bool {
        self.next_text
            .get(self.at.line)
            .and_then(|&line| self.lines.get(line))
            .is_some_and(|line| self.is_row(line))
    }

    /// The empty line of `close`, after which Catch2's table is closed.
    fn close(&mut self) -> bool {
        self.place(self.at.line);
        self.next_line();
        self.at.table = None;
        false
    }

    fn next_line(&mut self) {
        self.at.line += 1;
        self.at.pos = 0;
    }

    fn current(&self) -> &'a str {
        self.lines.get(self.at.line).copied().unwrap_or_default()
    }

    /// Places a line whole, unless it is a rule with text before its dashes, which only the
    /// program's own output glued to the rule can make.
    fn place(&mut self, line: usize) {
        let text = self.line(line);
        if self.is_rule(text) && !text.trim_end_matches('-').is_empty() {
            return;
        }
        self.save(line);
        if let Some(placed) = self.placed.get_mut(line) {
            *placed = true;
        }
    }

    fn cover(&mut self, length: usize) {
        self.save(self.at.line);
        if let Some(covered) = self.covered.get_mut(self.at.line) {
            *covered += length;
        }
        self.at.pos += length;
    }

    /// Keeps a line's cover and placement, inside a reading that is only tried, to put back.
    fn save(&mut self, line: usize) {
        if self.trials > 0
            && let (Some(&covered), Some(&placed)) = (self.covered.get(line), self.placed.get(line))
        {
            self.undo.push(Undo {
                line,
                covered,
                placed,
            });
        }
    }

    /// Starts a reading that is only tried, from where the printer is.
    fn try_from(&mut self) -> (At, usize) {
        self.trials += 1;
        (self.at.clone(), self.undo.len())
    }

    /// Puts back where the printer was and what it had placed before a reading it only tried.
    fn put_back(&mut self, (at, mark): (At, usize)) {
        while self.undo.len() > mark
            && let Some(undo) = self.undo.pop()
        {
            if let (Some(covered), Some(placed)) = (
                self.covered.get_mut(undo.line),
                self.placed.get_mut(undo.line),
            ) {
                (*covered, *placed) = (undo.covered, undo.placed);
            }
        }
        self.trials -= 1;
        self.at = at;
    }

    /// Whether a line is placed whole, or every byte of it is in a cell.
    fn is_placed(&self, line: usize) -> bool {
        let length = self.lines.get(line).map_or(0, |line| line.len());
        self.placed.get(line).copied().unwrap_or_default()
            || self
                .covered
                .get(line)
                .is_some_and(|&covered| covered >= length)
    }

    /// Whether a line holds only the cells of a row past its first column.
    fn is_fragment(&self, line: usize) -> bool {
        self.lines.get(line).is_some_and(|line| {
            (1..=LAST_COLUMN).any(|column| {
                trailing_length(line, usize::from(LAST_COLUMN.abs_diff(column)) + 1)
                    == Some(line.len())
            })
        })
    }

    /// Whether a test case or section header comes before Catch2's next table.
    fn header_next(&self) -> bool {
        self.next_break
            .get(self.at.line)
            .and_then(|&line| self.lines.get(line))
            .is_some_and(|line| self.is_rule(line))
    }

    /// The output up to Catch2's next table, whose header and rule Catch2's `open` prints: the
    /// headers of test cases and sections, and everything else Catch2 prints outside a table,
    /// none of which is a row of a table. A row out of its table opens it again where a
    /// benchmark reads cleanly from it, and is left unplaced otherwise.
    fn open(&mut self) -> Option<()> {
        while self.step()
            && let Some(line) = self.lines.get(self.at.line).copied()
        {
            let at = self.at.line;
            if self.is_rule(line) && !self.starts_no_header(at) {
                self.place(at);
                self.next_line();
                let start = self.at.line;
                self.skip_past_rule();
                let end = self.at.line.saturating_sub(1).max(start);
                let lines = self.lines.get(start..end).unwrap_or_default();
                self.at.path = header_path(lines, self.rule, self.wraps_bytes).into();
            } else if let Some((table, lines)) = self.table_header(at) {
                for line in at..at + lines {
                    self.place(line);
                }
                (self.at.line, self.at.pos) = (at + lines, 0);
                (self.at.table, self.last_table) = (Some(table), Some(table));
                return Some(());
            } else if self.is_row(line) {
                // A reading only tried ends at a row out of its table, which it would not place.
                if self.trying {
                    return None;
                }
                if let Some(table) = self.last_table
                    && self.reads_cleanly(at, table)
                {
                    (self.at.pos, self.at.column, self.at.table) = (0, -1, Some(table));
                    return Some(());
                }
                self.next_line();
            } else {
                self.place(at);
                self.next_line();
            }
        }
        None
    }

    /// Whether a rule at `line` starts no header of Catch2's, which always names its test case
    /// and never holds a table's header or a row, as where the program printed a rule of its own.
    fn starts_no_header(&self, line: usize) -> bool {
        let block = self
            .lines
            .get(line + 1..)
            .unwrap_or_default()
            .iter()
            .take_while(|line| !self.is_rule(line))
            .count();
        block == 0
            || (line + 1..=line + block)
                .any(|line| self.table_header(line).is_some() || self.is_row(self.line(line)))
    }

    /// The table whose header Catch2's `open` prints from `line`, and how many lines it takes:
    /// `benchmark name` padded to the first column's width, which `TextFlow` wraps where the
    /// column is too narrow for it and leaves blank where it is too narrow for a letter and its
    /// hyphen, the exact titles of the analysis or the mean table's columns on each of their
    /// lines, then a rule as wide as the console.
    fn table_header(&self, line: usize) -> Option<(Table, usize)> {
        let first = self.line(line);
        HEADINGS.iter().find_map(|header| {
            let last = header.titles.first()?.last()?;
            let name_width = first
                .strip_suffix(last)?
                .len()
                .checked_sub(2 * COLUMN_WIDTH)
                .filter(|&width| width > 2)?;
            let names = title(name_width - 2);
            let count = names.len().max(header.titles.len());
            let titles = (0..count).all(|offset| {
                let name = names.get(offset).map_or("", String::as_str);
                let cell = COLUMN_WIDTH;
                let expected = header.titles.get(offset).map_or_else(
                    || name.to_owned(),
                    |[samples, iterations, third]| {
                        format!("{name:name_width$}{samples:cell$}{iterations:cell$}{third}")
                    },
                );
                self.line(line + offset) == expected
            });
            let rule = self.line(line + count);
            let rule_fits = rule.bytes().all(|byte| byte == b'-')
                && header
                    .rules
                    .iter()
                    .any(|&rule_width| rule.len() == name_width + rule_width);
            (titles && rule_fits).then_some((
                Table {
                    analysis: header.analysis,
                    name_width,
                    wrapped: names.len() != 1,
                },
                count + 1,
            ))
        })
    }

    fn line(&self, line: usize) -> &'a str {
        self.lines.get(line).copied().unwrap_or_default()
    }

    /// Whether a line is one that Catch2's table printer writes: it ends in a value cell or holds
    /// a failure after a name, or it starts with a cell of the first column and then a value
    /// cell, or it is all spaces as wide as the table. A rule, which closes a table, is
    /// never a row, so that no line both closes a table and opens it again.
    fn is_row(&self, line: &str) -> bool {
        if self.is_rule(line) {
            return false;
        }
        let failure = line
            .find(FAILED)
            .is_some_and(|at| at > 0 && line.get(..at).is_some_and(|name| name.ends_with(' ')));
        if failure || trailing_length(line, 1).is_some_and(|length| length > 0) {
            return true;
        }
        let Some(table) = self.last_table else {
            return false;
        };
        if line.bytes().all(|byte| byte == b' ') {
            return line.len() == table.width();
        }
        line.get(..table.name_width)
            .is_some_and(|cell| cell.ends_with(' '))
            && line
                .get(table.name_width..)
                .is_some_and(|rest| value_cell(rest, COLUMN_WIDTH, Justify::Right).is_some())
    }

    /// Whether a whole named benchmark reads from `line` as the first row of `table`.
    fn reads_cleanly(&mut self, line: usize, table: Table) -> bool {
        let trial = self.try_from();
        self.trying = true;
        (self.at.line, self.at.pos, self.at.column, self.at.table) = (line, 0, -1, Some(table));
        let clean = self.cell(Kind::Name, Within::Run).is_some_and(|first| {
            first.text().is_ok_and(|name| !name.trim().is_empty()) && self.benchmark(first).is_ok()
        });
        self.trying = false;
        self.put_back(trial);
        clean
    }

    /// Past a header's lines and the rule that ends them, short of a row, which no header holds.
    fn skip_past_rule(&mut self) {
        while let Some(line) = self.lines.get(self.at.line).copied() {
            if self.is_row(line) {
                break;
            }
            self.place(self.at.line);
            self.next_line();
            if self.is_rule(line) {
                break;
            }
        }
    }

    /// Whether a failure's cell starts here, and past it if so. Catch2's `RowBreak` ends the line
    /// after one past the first column, and so does v3's `close` after one in the first, so a
    /// failure runs, past any blank line of its message, to the last line ending in its `)` and
    /// a space before Catch2's next row, which ends in a value or fills the table's width, or its
    /// next assertion or rule. After one in the first column, v2 goes on with the next benchmark
    /// in the same row. A name that starts like a failure is followed by its values.
    fn failure(&mut self, rest: &str, kind: Kind, width: usize) -> bool {
        if !rest.starts_with(FAILED) {
            return false;
        }
        let after = self.lines.get(self.at.line + 1..).unwrap_or_default();
        let full = self.at.table.map(Table::width);
        let span = after
            .iter()
            .position(|line| {
                line.contains(FAILED)
                    || line.ends_with(": FAILED:")
                    || self.is_rule(line)
                    || ends_in_value(line)
                    || Some(line.len()) == full
            })
            .unwrap_or(after.len());
        let message = after.get(..span).unwrap_or_default();
        let last = message
            .iter()
            .rposition(|line| line.ends_with(' ') && line.trim_end().ends_with(')'))
            .map(|offset| offset + 1);
        // Catch2 writes a space after every cell.
        let closed = rest.ends_with(' ') && is_failure(rest);
        if let Some(offset) = last.or_else(|| closed.then_some(0)) {
            self.cover(rest.len());
            for line in self.at.line + 1..=self.at.line + offset {
                self.place(line);
            }
            self.at.line += offset;
            self.at.pos = self.current().len();
            return true;
        }
        if matches!(kind, Kind::Name) || self.at.column != 0 {
            return false;
        }
        if let Some(end) = rest.find(") ") {
            self.cover((end + 2).max(width).min(rest.len()));
            return true;
        }
        // A message on more than one line ends where v2's next cells start, its cell padded by
        // the length of all of its text.
        if self.wraps_bytes
            && let Some(&line) = after.get(span)
            && let Some(end) = line.find(") ")
        {
            let length =
                rest.len() + message.iter().map(|line| line.len() + 1).sum::<usize>() + end + 2;
            self.cover(rest.len());
            for line in self.at.line + 1..=self.at.line + span {
                self.place(line);
            }
            (self.at.line, self.at.pos) = (self.at.line + span + 1, 0);
            self.cover((end + 2 + width.saturating_sub(length + 1)).min(line.len()));
            return true;
        }
        false
    }

    /// A line of Catch2's rule, which may follow the end of the program's own output.
    fn is_rule(&self, line: &str) -> bool {
        let dashes = line.len() - line.trim_end_matches('-').len();
        if self.rule == 0 {
            dashes > 0 && dashes == line.len()
        } else {
            dashes >= self.rule
        }
    }

    /// The cell at the current column: the first column is the name's width and left-justified,
    /// and the others are right-justified, and a cell's text overflows its width unpadded.
    fn read_cell(&mut self, table: Table, kind: Kind) -> Cell<'a> {
        let rest = self.current().get(self.at.pos..).unwrap_or_default();
        let (width, justify) = self.geometry(table);
        if self.failure(rest, kind, width) {
            return Cell::Failed;
        }
        if let Kind::Name = kind {
            let cell = rest
                .get(
                    ..name_length(
                        rest,
                        self.at.column,
                        width,
                        table.name_width.saturating_sub(2),
                    ),
                )
                .unwrap_or(rest);
            // A name in the first column starts the cell, so its own leading spaces are its own.
            let text = if self.at.column == 0 {
                cell.trim_end()
            } else {
                cell.trim()
            };
            let lines = if self.wraps_bytes {
                text.len()
            } else {
                text.chars().count()
            };
            // `TextFlow` wraps a name to two less than the first column, and `ColumnBreak` pads
            // its cell by bytes.
            if lines + 2 > table.name_width || cell.len() != (text.len() + 1).max(width) {
                return Cell::Bad;
            }
            self.cover(cell.len());
            return Cell::Text(text);
        }
        let cell = value_cell(rest, width, justify).and_then(|length| rest.get(..length));
        if let Some(cell) = cell
            && kind.fits(cell.trim())
        {
            self.cover(cell.len());
            return Cell::Text(cell.trim());
        }
        // A cell that is not Catch2's is the program's own output, before the rest of Catch2's
        // row on the next line that holds it, unless the line starts a row of its own.
        let starts_row =
            self.at.pos == 0 && self.is_row(self.current()) && !self.continues(self.at.line);
        if !starts_row && let Some(line) = self.row_after(self.at.line + 1, self.at.column, table) {
            (self.at.line, self.at.pos) = (line, 0);
            let rest = self.current().get(self.at.pos..).unwrap_or_default();
            if let Some(cell) =
                value_cell(rest, width, justify).and_then(|length| rest.get(..length))
                && kind.fits(cell.trim())
            {
                self.cover(cell.len());
                return Cell::Text(cell.trim());
            }
            return Cell::Bad;
        }
        // A cell padded as Catch2 pads one whose value does not read is Catch2's.
        if let Some(cell) = cell
            && !matches!(kind, Kind::Empty)
        {
            self.cover(cell.len());
            return Cell::Text(cell.trim());
        }
        Cell::Bad
    }

    /// The current column's width and justification.
    fn geometry(&self, table: Table) -> (usize, Justify) {
        if self.at.column == 0 {
            (table.name_width, Justify::Left)
        } else {
            (COLUMN_WIDTH, Justify::Right)
        }
    }
}

/// Count each unplaced line, in order, against each benchmark it falls in, else, where it only
/// goes on with a benchmark, against the one before it where that one lost step, else as a
/// benchmark of its own. The benchmarks read are in order, each ending by the line the next
/// starts on, so one pass over them finds every line's.
fn count(outcomes: &mut Vec<Outcome>, lines: &[(usize, bool)]) {
    let read = outcomes.len();
    // The first benchmark read that has not ended before the line, and the one that ended last.
    let (mut next, mut before) = (0, None::<usize>);
    for &(line, continues) in lines {
        while next < read
            && let Some(outcome) = outcomes.get(next)
            && outcome.end < line
        {
            let end = outcome.end;
            if before
                .and_then(|before| outcomes.get(before))
                .is_none_or(|before| before.end < end)
            {
                before = Some(next);
            }
            next += 1;
        }
        let mut found = false;
        for outcome in outcomes
            .get_mut(next..read)
            .unwrap_or_default()
            .iter_mut()
            .take_while(|outcome| outcome.start <= line)
        {
            outcome.benchmark = None;
            found = true;
        }
        if found {
            continue;
        }
        if continues
            && let Some(outcome) = before.and_then(|before| outcomes.get_mut(before))
            && outcome.lost_step
        {
            outcome.end = line;
            continue;
        }
        before = Some(outcomes.len());
        outcomes.push(Outcome {
            start: line,
            end: line,
            lost_step: true,
            benchmark: None,
        });
    }
}

/// The length of a value cell as Catch2 writes one of `width`: a right-justified value padded to
/// the width unless it has as many characters or more, a left-justified one padded after it, and
/// an empty cell all spaces.
fn value_cell(rest: &str, width: usize, justify: Justify) -> Option<usize> {
    let padding = rest.len() - rest.trim_start().len();
    if padding >= width {
        return Some(width);
    }
    let value = value_length(rest.get(padding..)?);
    match justify {
        Justify::Right => {
            let length = padding + value + 1;
            let padded = if value + 1 < width {
                length == width
            } else {
                padding == 0
            };
            (padded && rest.get(padding + value..length) == Some(" ")).then_some(length)
        },
        Justify::Left => {
            let length = (value + 1).max(width);
            (padding == 0 && rest.get(value..length)?.bytes().all(|byte| byte == b' '))
                .then_some(length)
        },
    }
}

/// The length of Catch2's cells from `column` to the last that end `rest`, the first column as
/// wide as the name and left-justified.
fn row_length(rest: &str, column: i8, name_width: usize) -> Option<usize> {
    if column > 0 {
        return trailing_length(rest, usize::from(LAST_COLUMN.abs_diff(column)) + 1);
    }
    let trailing = trailing_length(rest, usize::from(LAST_COLUMN.unsigned_abs()))?;
    let first = rest.get(..rest.len() - trailing)?;
    (first.len() >= name_width && first.ends_with(' ') && Kind::Value.fits(first.trim()))
        .then_some(rest.len())
}

/// The mean and the standard deviation of an analysis table's cells, each with its interval,
/// after the estimated run time.
fn analysis(cells: [&str; 13]) -> Option<Statistics<'static>> {
    let [
        _,
        "",
        mean,
        low,
        high,
        "",
        std_dev,
        low_std_dev,
        high_std_dev,
        "",
        "",
        "",
        "",
    ] = cells
    else {
        return None;
    };
    Some(Statistics {
        mean: durations([mean, low, high])?,
        std_dev: Some(durations([std_dev, low_std_dev, high_std_dev])?),
        level: None,
    })
}

/// A name from the lines `TextFlow` wrapped it onto, each with the width it had: a line that
/// ends in a character `TextFlow` breaks after, or one that starts with a character it breaks
/// before, joins without a space, and a full line that split a word drops the hyphen `TextFlow`
/// added.
fn join(lines: &[(&str, usize)], wraps_bytes: bool) -> String {
    let mut name = String::new();
    let mut previous: Option<(&str, usize)> = None;
    for &(line, width) in lines {
        if let Some((previous, previous_width)) = previous {
            if is_split(previous, previous_width, wraps_bytes) {
                name.pop();
            } else if !previous.ends_with(BREAKS_AFTER) && !line.starts_with(BREAKS_BEFORE) {
                name.push(' ');
            }
        }
        name.push_str(line);
        previous = Some((line, width));
    }
    name
}

/// A full line ending in the hyphen `TextFlow` adds where it found nowhere to break: never at
/// the line's start, and at Catch2's whitespace only after a character that is not.
fn is_split(line: &str, width: usize, wraps_bytes: bool) -> bool {
    let length = if wraps_bytes {
        line.len()
    } else {
        line.chars().count()
    };
    length == width
        && line.strip_suffix('-').is_some_and(|word| {
            !word.contains(BREAKS_AFTER)
                && !word.chars().zip(word.chars().skip(1)).any(|(before, c)| {
                    BREAKS_BEFORE.contains(&c)
                        || (WHITESPACE.contains(&c) && !WHITESPACE.contains(&before))
                })
        })
}

/// Whether a line ends in a value cell that holds a count or a duration, as a row of Catch2's
/// does, where a failure's padded last line ends in spaces alone.
fn ends_in_value(line: &str) -> bool {
    trailing_length(line, 1).is_some()
        && line
            .strip_suffix(' ')
            .and_then(value_suffix)
            .is_some_and(|length| length > 0)
}

/// A failure's cell that ends its line.
fn is_failure(rest: &str) -> bool {
    let text = rest.trim();
    text.starts_with(FAILED) && text.ends_with(')')
}

/// The length of a name cell, which Catch2 pads by bytes to its column's `width` and which
/// overflows it unpadded: up to a failure that ends its line right after it, or to the value
/// cells that fill the rest of its line, else its width where the name fits, else the rest of
/// the line. `TextFlow` wraps a name to no more than `limit` characters.
fn name_length(rest: &str, column: i8, width: usize, limit: usize) -> usize {
    if let Some(failed) = rest
        .match_indices(FAILED)
        .map(|(start, _)| start)
        .find(|&start| {
            let name = rest.get(..start).unwrap_or_default().trim();
            name.chars().count() <= limit
                && rest
                    .get(start..)
                    .is_some_and(|failure| is_failure(failure) || !failure.ends_with(' '))
        })
    {
        return failed;
    }
    let cells = usize::from(LAST_COLUMN.abs_diff(column));
    if let Some(trailing) = trailing_length(rest, cells)
        && rest.len() - trailing >= width
    {
        return rest.len() - trailing;
    }
    let fits = rest
        .get(..width)
        .is_some_and(|chunk| chunk.ends_with(' ') && (column == 0 || chunk.starts_with(' ')));
    if fits { width } else { rest.len() }
}

/// The length of the `cells` value cells that end `rest`, each right-justified in its column and
/// followed by a space, read from the end of the line.
fn trailing_length(rest: &str, cells: usize) -> Option<usize> {
    let mut end = rest.len();
    for _ in 0..cells {
        let text = rest.get(..end)?.strip_suffix(' ')?;
        let value = value_suffix(text)?;
        let start = (text.len() - value).checked_sub((COLUMN_WIDTH - 1).saturating_sub(value))?;
        if !rest
            .get(start..text.len() - value)?
            .bytes()
            .all(|byte| byte == b' ')
        {
            return None;
        }
        end = start;
    }
    Some(rest.len() - end)
}

/// The length of the count or duration that ends `text`, which is none for an empty cell.
fn value_suffix(text: &str) -> Option<usize> {
    if text.is_empty() || text.ends_with(' ') {
        return Some(0);
    }
    let (before, last) = text.rsplit_once(' ').unwrap_or(("", text));
    if nanos(last).is_some() {
        let number = before.rsplit(' ').next()?;
        number.parse::<f64>().ok()?;
        return Some(number.len() + 1 + last.len());
    }
    last.parse::<u64>().ok()?;
    Some(last.len())
}

/// The length of a cell's value: a count, or a number and its time unit.
fn value_length(text: &str) -> usize {
    let number = text.split(' ').next().unwrap_or_default();
    let unit = text
        .get(number.len() + 1..)
        .and_then(|rest| rest.split(' ').next())
        .filter(|unit| nanos(unit).is_some());
    unit.map_or(number.len(), |unit| number.len() + 1 + unit.len())
}

/// A duration as Catch2 writes it, a number and its unit, in nanoseconds.
fn duration(text: &str) -> Option<f64> {
    let (number, unit) = text.split_once(' ')?;
    Some(number.parse::<f64>().ok()? * nanos(unit)?)
}

fn durations([estimate, lower, upper]: [&str; 3]) -> Option<[f64; 3]> {
    Some([duration(estimate)?, duration(lower)?, duration(upper)?])
}

/// The nanoseconds in each unit Catch2's `Duration` prints, which reach minutes from 60 s up.
fn nanos(unit: &str) -> Option<f64> {
    Some(match unit {
        "ns" => 1.0,
        "us" => 1e3,
        "ms" => 1e6,
        "s" => 1e9,
        "m" => 6e10,
        _ => return None,
    })
}

/// The test case, then each section, which Catch2 indents and wraps in a column as wide as its
/// `rule`: a line indented to just past an entry's first `: ` continues it, as does an
/// unindented line after the test case. A section wrapped otherwise reads as a section of its
/// own, because the text cannot tell them apart.
fn header_path(lines: &[&str], rule: usize, wraps_bytes: bool) -> Vec<String> {
    let mut entries: Vec<Header<'_>> = Vec::new();
    for line in lines {
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let width = rule.saturating_sub(indent);
        let continues = entries
            .last()
            .is_some_and(|header| header.indent == Some(indent))
            || (entries.len() == 1 && indent == 0);
        if continues && let Some(header) = entries.last_mut() {
            header.lines.push((text, width));
        } else {
            entries.push(Header {
                lines: vec![(text, width)],
                indent: line.find(": ").map(|at| at + 2),
            });
        }
    }
    entries
        .iter()
        .map(|header| join(&header.lines, wraps_bytes))
        .collect()
}

/// `benchmark name` as `TextFlow` wraps it into a column `width` wide: at the space where it
/// does not fit, and inside a word longer than the column, after a hyphen it adds. A column with
/// no room for a letter before the hyphen holds no line of it.
fn title(width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut rest = TABLE_HEADER;
    while !rest.is_empty() {
        if rest.len() <= width {
            lines.push(rest.to_owned());
            break;
        }
        if let Some(space) = rest
            .get(..=width)
            .and_then(|head| head.rfind(' '))
            .filter(|&space| space > 0)
        {
            lines.push(rest.get(..space).unwrap_or_default().to_owned());
            rest = rest.get(space + 1..).unwrap_or_default();
        } else {
            let cut = width.saturating_sub(1);
            if cut == 0 {
                break;
            }
            lines.push(format!("{}-", rest.get(..cut).unwrap_or_default()));
            rest = rest.get(cut..).unwrap_or_default();
        }
    }
    lines
}

/// A test case or section header's lines, each with its width, and the indent of the lines it
/// wraps onto, just past its first `: `.
struct Header<'a> {
    lines: Vec<(&'a str, usize)>,
    indent: Option<usize>,
}

/// What Catch2 prints in a cell, or a cell it did not print.
#[derive(Clone, Copy)]
enum Cell<'a> {
    Text(&'a str),
    Failed,
    Bad,
}

impl<'a> Cell<'a> {
    fn text(self) -> Result<&'a str, Stop> {
        match self {
            Self::Text(text) => Ok(text),
            Self::Failed => Err(Stop::Failed),
            Self::Bad => Err(Stop::Unreadable),
        }
    }
}

/// Why a benchmark was not read: it failed, ended, or held values that do not read, all in
/// step with Catch2's printer, or the printer lost step in it.
enum Stop {
    Failed,
    Unreadable,
}

/// The kind of text a cell holds: a line of a benchmark's name, which only Catch2's own
/// wrapping keeps within the first column, a count or a duration, which an empty cell may hold
/// instead, or nothing.
#[derive(Clone, Copy)]
enum Kind {
    Name,
    Value,
    Empty,
}

impl Kind {
    fn fits(self, text: &str) -> bool {
        text.is_empty()
            || match self {
                Self::Name => true,
                Self::Value => text.parse::<u64>().is_ok() || duration(text).is_some(),
                Self::Empty => false,
            }
    }
}

/// The cells of an analysis table after the iterations: the estimated run time, then the mean
/// and the standard deviation each with its interval, under the empty first column of each row,
/// and a last row of empty cells.
const ANALYSIS: [Kind; 13] = [
    Kind::Value,
    Kind::Empty,
    Kind::Value,
    Kind::Value,
    Kind::Value,
    Kind::Empty,
    Kind::Value,
    Kind::Value,
    Kind::Value,
    Kind::Empty,
    Kind::Empty,
    Kind::Empty,
    Kind::Empty,
];

/// Where a cell is read: between benchmarks, across any header, or inside one, which a header
/// ends.
#[derive(Clone, Copy)]
enum Within {
    Run,
    Benchmark,
}

#[derive(Clone, Copy)]
enum Justify {
    Left,
    Right,
}

/// A benchmark table: the full analysis, or the mean alone under `--benchmark-no-analysis`,
/// the width of its first column, which Catch2's version and console width set, and whether
/// that width made `TextFlow` wrap the table's title or leave it blank.
#[derive(Clone, Copy)]
struct Table {
    analysis: bool,
    name_width: usize,
    wrapped: bool,
}

impl Table {
    /// The width of a full row: the first column and the three after it.
    fn width(self) -> usize {
        self.name_width + usize::from(LAST_COLUMN.unsigned_abs()) * COLUMN_WIDTH
    }
}

/// A table header Catch2 prints: whether it is the analysis table's, its column titles past the
/// first column on each of its lines, and the console's width less one past the first column,
/// which Catch2 v2.11 sets 11 less for the analysis table than every other version and table.
struct Heading {
    analysis: bool,
    titles: &'static [[&'static str; 3]],
    rules: &'static [usize],
}

const HEADINGS: [Heading; 3] = [
    Heading {
        analysis: true,
        titles: &[
            ["samples", "iterations", "est run time"],
            ["mean", "low mean", "high mean"],
            ["std dev", "low std dev", "high std dev"],
        ],
        rules: &[42],
    },
    Heading {
        analysis: true,
        titles: &[
            ["samples", "iterations", "estimated"],
            ["mean", "low mean", "high mean"],
            ["std dev", "low std dev", "high std dev"],
        ],
        rules: &[42, 31],
    },
    Heading {
        analysis: false,
        titles: &[["     samples", "  iterations", "        mean"]],
        rules: &[42],
    },
];
/// The steps the reader may take for each line, many times the few it takes on any output of
/// Catch2's, with the program's own or not.
const STEPS_PER_LINE: usize = 64;
const COLUMN_WIDTH: usize = 14;
const LAST_COLUMN: i8 = 3;
const TABLE_HEADER: &str = "benchmark name";
const FAILED: &str = "Benchmark failed (";
const BREAKS_AFTER: [char; 15] = [
    ']', ')', '}', '>', '.', ',', ':', ';', '*', '+', '-', '=', '&', '/', '\\',
];
const BREAKS_BEFORE: [char; 5] = ['[', '(', '{', '<', '|'];
const WHITESPACE: [char; 4] = [' ', '\t', '\r', '\n'];

#[cfg(test)]
mod test_cpp_catch2_text {
    use bencher_json::project::report::Adapter;
    use pretty_assertions::assert_eq;

    use crate::adapters::bmf_v1::{
        cpp::catch2::test_cpp_catch2_v1::{GENERIC, assert_v3, latency, mean},
        test_util::{assert_ignores_average, assert_variant, convert_v1},
    };

    use super::{COLUMN_WIDTH, Justify, Printer, join, name_length, reading, value_cell};

    /// Kills a printer that leaves a line it cannot place uncounted, or that cannot place a line
    /// of Catch2's own: on every Catch2 text fixture, each line is placed or counted, a counted
    /// line comes with a skipped benchmark, and only a fixture where the program prints inside a
    /// table, whose header was edited, or whose console is too narrow for the table's title has
    /// a counted line.
    #[test]
    fn adapter_cpp_catch2_v1_text_lines() {
        for (fixture, input) in fixtures() {
            let reading = reading(&input);
            // `issue_351.txt` has an edited header that its rows do not fit.
            let foreign = [
                "stray",
                "print",
                "lastcol",
                "issue_351",
                "distfail",
                "blankfail",
                "blankun",
                "absorb",
                "unnamed",
                "fakeheader",
                "crossing",
                "sepline",
                "failtext",
                "failvalue",
                "width_58",
                "width_46",
            ]
            .iter()
            .any(|foreign| fixture.contains(foreign));
            assert!(
                reading.unplaced.is_empty()
                    || (foreign && reading.benchmarks.iter().any(Option::is_none)),
                "{fixture}: {:?}",
                reading.unplaced
            );
        }
    }

    /// Kills a reader that can loop without end, as one that took a benchmark named like the
    /// table's header for the header did: on every line prefix of each Catch2 text fixture, with
    /// each of its lines deleted, and with each written twice, the reader stops within its bound
    /// of steps.
    #[test]
    fn adapter_cpp_catch2_v1_text_terminates() {
        for (fixture, input) in fixtures() {
            let lines: Vec<&str> = input.split_inclusive('\n').collect();
            let mut inputs: Vec<String> = (0..=lines.len())
                .map(|end| lines.get(..end).unwrap_or_default().concat())
                .collect();
            for line in 0..lines.len() {
                let (before, after) = lines.split_at(line);
                let rest = after.get(1..).unwrap_or_default();
                inputs.push([before, rest].concat().concat());
                inputs.push(
                    [before, after.get(..1).unwrap_or_default(), after]
                        .concat()
                        .concat(),
                );
            }
            for (case, input) in inputs.iter().enumerate() {
                assert!(!reading(input).exhausted, "{fixture}: input {case}");
            }
        }
    }

    /// Kills a reader with no bound on its steps, or one that keeps a benchmark past its bound:
    /// a reader stopped early leaves the rest of its input unplaced, and counts at least each
    /// benchmark it did not read.
    #[test]
    fn adapter_cpp_catch2_v1_text_step_bound() {
        let input = std::fs::read_to_string("./tool_output/cpp/catch2/v3.txt")
            .expect("Failed to read a fixture");
        let mut printer = Printer::new(&input);
        printer.budget = 60;
        let reading = printer.read();
        let kept = reading
            .benchmarks
            .iter()
            .filter(|benchmark| benchmark.is_some())
            .count();
        assert!(reading.exhausted);
        assert!(kept < 5, "{kept}");
        assert!(
            reading.benchmarks.len() >= 6,
            "{}",
            reading.benchmarks.len()
        );
    }

    /// Kills a join that takes a character outside Catch2's whitespace, as a no-break space, for
    /// a place `TextFlow` breaks, which keeps the hyphen it added where it split a word.
    #[test]
    fn adapter_cpp_catch2_v1_text_split_whitespace() {
        assert_eq!(
            join(&[("aaaa\u{a0}bbbb-", 10), ("cc", 10)], false),
            "aaaa\u{a0}bbbbcc"
        );
    }

    /// Each Catch2 text fixture's path and text.
    fn fixtures() -> Vec<(String, String)> {
        std::fs::read_dir("./tool_output/cpp/catch2")
            .expect("Failed to read fixtures")
            .map(|entry| entry.expect("Failed to read a fixture").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "txt"))
            .map(|path| {
                let input = std::fs::read_to_string(&path).expect("Failed to read a fixture");
                (path.display().to_string(), input)
            })
            .collect()
    }

    /// Kills a reader that slices every cell at its width, where Catch2 writes a longer text
    /// unpadded: a name printed past the first column, which ends at the empty cells that fill its
    /// line, at a failure that ends its line, or at its line's end, and two durations too long for
    /// their cells.
    #[test]
    fn adapter_cpp_catch2_v1_text_overflow() {
        let wrapped = format!("Fibonacci 15 with a rather long {}", " ".repeat(28));
        assert_eq!(name_length(&wrapped, 1, COLUMN_WIDTH, 35), 32);
        assert_eq!(
            name_length(
                "throws early, long name Benchmark failed (boom) ",
                1,
                COLUMN_WIDTH,
                35
            ),
            24
        );
        assert_eq!(
            name_length("requires early, long name ", 1, COLUMN_WIDTH, 35),
            26
        );
        assert_eq!(
            value_cell(
                "3.55271e-15 ns 1.77636e-15 ns ",
                COLUMN_WIDTH,
                Justify::Right
            ),
            Some(15)
        );
    }

    /// Kills a leaf that drops a section or a wrapped line of a name, names a benchmark without
    /// its test case, keeps version 0's band of the mean plus and minus the standard deviation,
    /// names a level the text does not state, claims a failed benchmark or skips it uncounted, or
    /// records no leaf.
    #[test]
    fn adapter_cpp_catch2_v1_text_v3() {
        for adapter in [Adapter::CppCatch2, Adapter::CppCatch2Text] {
            let results = convert_v1(adapter, "cpp/catch2/v3.txt");
            assert_eq!(
                results.resolved_adapter,
                Some(Adapter::CppCatch2Text),
                "{adapter}"
            );
            assert_v3(
                &results,
                GENERIC,
                [
                    [2.67901, 2.63, 2.72468, 0.108_618, 0.086_459_7, 0.15412],
                    [42.9915, 42.7461, 43.9477, 1.0315, 0.044_331_3, 2.04879],
                    [79.1209, 78.5418, 81.3099, 2.37207, 0.181_505, 4.70229],
                    [25.0838, 24.7882, 26.2139, 1.142, 0.041_219_6, 2.2425],
                    [4_006.52, 3_959.46, 4_101.46, 148.59, 38.3199, 233.861],
                ],
            );
        }
    }

    /// Kills a leaf that reads the estimated run time of a table without analysis as anything
    /// but its mean.
    #[test]
    fn adapter_cpp_catch2_v1_text_no_analysis() {
        let results = convert_v1(Adapter::CppCatch2Text, "cpp/catch2/v3_no_analysis.txt");
        assert_eq!(results.inner.len(), 2);
        assert_variant(&results, "Fibonacci / Fibonacci 10", "{}", &[mean(26.4006)]);
        assert_variant(
            &results,
            "Fibonacci / Fibonacci 20",
            "{}",
            &[mean(3_976.72)],
        );
    }

    /// Kills a leaf that misreads Catch2 v2's columns.
    #[test]
    fn adapter_cpp_catch2_v1_text_v2() {
        let results = convert_v1(Adapter::CppCatch2Text, "cpp/catch2/four.txt");
        assert_eq!(results.inner.len(), 4);
        assert_variant(
            &results,
            "Fibonacci / Fibonacci 10",
            "{}",
            &[latency(GENERIC, [344.0, 341.0, 349.0, 19.0, 11.0, 29.0])],
        );
        assert_variant(
            &results,
            "More Fibonacci / Fibonacci-15_bench",
            "{}",
            &[latency(
                GENERIC,
                [3_789.0, 3_734.0, 3_888.0, 362.0, 234.0, 539.0],
            )],
        );
    }

    /// Kills a text leaf that reads the average setting.
    #[test]
    fn adapter_cpp_catch2_v1_text_ignores_average() {
        for suffix in ["cpp/catch2/v3.txt", "cpp/catch2/four.txt"] {
            assert_ignores_average(Adapter::CppCatch2Text, suffix);
        }
    }
}
