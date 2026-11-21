//! Minimal PDF writer for service reports (no external dependency).
//!
//! Uses the standard Helvetica / Helvetica-Bold fonts with WinAnsiEncoding, so Spanish text (á, é, ñ, ¿, «»…)
//! renders in every PDF reader without embedding fonts. Layout is a simple top-to-bottom flow on A4 pages
//! with automatic page breaks, word wrapping measured with the real font metrics, and a footer on every page.

use std::fmt::Write as _;

pub const PAGE_W: f32 = 595.28;
pub const PAGE_H: f32 = 841.89;
pub const MARGIN: f32 = 50.0;
const FOOTER_H: f32 = 40.0;
pub const CONTENT_W: f32 = PAGE_W - 2.0 * MARGIN;

#[derive(Debug, Clone, Copy)]
pub struct Rgb(pub f32, pub f32, pub f32);

pub const INK: Rgb = Rgb(0.11, 0.13, 0.16);
pub const MUTED: Rgb = Rgb(0.42, 0.45, 0.50);
pub const RULE: Rgb = Rgb(0.85, 0.87, 0.89);
pub const BRAND: Rgb = Rgb(0.05, 0.55, 0.50);
pub const OK: Rgb = Rgb(0.13, 0.55, 0.30);
pub const WARN: Rgb = Rgb(0.72, 0.47, 0.05);
pub const DANGER: Rgb = Rgb(0.75, 0.20, 0.20);
pub const INFO: Rgb = Rgb(0.20, 0.42, 0.75);
pub const BAND: Rgb = Rgb(0.95, 0.96, 0.97);

// Advance widths (1/1000 em) for ASCII 32..=126 from the Adobe core font metrics.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278,
    584, 584, 584, 556, 1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667,
    944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333,
    500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];
const HELVETICA_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 333, 333,
    584, 584, 584, 611, 975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667,
    944, 667, 667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611, 611, 611, 389,
    556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];

/// Character → WinAnsiEncoding byte. Characters outside the code page become '?'.
fn win_ansi(c: char) -> u8 {
    match c {
        '\u{20}'..='\u{7e}' => c as u8,
        '\u{a0}'..='\u{ff}' => c as u32 as u8,
        '€' => 0x80,
        '‚' => 0x82,
        '„' => 0x84,
        '…' => 0x85,
        '‹' => 0x8b,
        '‘' => 0x91,
        '’' => 0x92,
        '“' => 0x93,
        '”' => 0x94,
        '•' => 0x95,
        '–' => 0x96,
        '—' => 0x97,
        '›' => 0x9b,
        // Not in WinAnsi: closest visible equivalent instead of '?'.
        '→' => 0xbb,
        '←' => 0xab,
        '\t' => b' ',
        _ => b'?',
    }
}

/// Base Latin letter used to estimate the width of accented characters.
fn width_char(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' => 'A',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' => 'O',
        'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'Ñ' => 'N',
        'Ç' => 'C',
        '¿' => '?',
        '¡' => '!',
        '«' | '»' | '‹' | '›' => '<',
        '–' => '-',
        '—' | '…' => 'M',
        '‘' | '’' => '\'',
        '“' | '”' => '"',
        '•' | '·' | 'º' | 'ª' => '.',
        _ => 'n',
    }
}

pub fn text_width(s: &str, size: f32, bold: bool) -> f32 {
    let table = if bold { &HELVETICA_BOLD } else { &HELVETICA };
    let units: u32 = s
        .chars()
        .map(|c| {
            let c = if (' '..='~').contains(&c) { c } else { width_char(c) };
            table[(c as usize).saturating_sub(32).min(94)] as u32
        })
        .sum();
    units as f32 * size / 1000.0
}

/// Splits text into lines no wider than `max` (explicit newlines are kept; very long words are cut).
pub fn wrap(text: &str, size: f32, bold: bool, max: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if text_width(&candidate, size, bold) <= max {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            // A single word wider than the line: cut it.
            let mut piece = String::new();
            for c in word.chars() {
                piece.push(c);
                if text_width(&piece, size, bold) > max {
                    piece.pop();
                    lines.push(std::mem::take(&mut piece));
                    piece.push(c);
                }
            }
            line = piece;
        }
        lines.push(line);
    }
    lines
}

fn escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match win_ansi(c) {
            b'(' => out.push_str("\\("),
            b')' => out.push_str("\\)"),
            b'\\' => out.push_str("\\\\"),
            b if b < 0x80 => out.push(b as char),
            b => {
                let _ = write!(out, "\\{b:03o}");
            }
        }
    }
    out
}

pub struct Document {
    title: String,
    footer: String,
    pages: Vec<String>,
    page: String,
    /// Current baseline position (from the bottom of the page).
    pub y: f32,
}

impl Document {
    pub fn new(title: &str, footer: &str) -> Self {
        Self { title: title.to_string(), footer: footer.to_string(), pages: Vec::new(), page: String::new(), y: PAGE_H - MARGIN }
    }

    fn new_page(&mut self) {
        self.pages.push(std::mem::take(&mut self.page));
        self.y = PAGE_H - MARGIN;
    }

    /// Starts a new page when less than `height` points remain.
    pub fn ensure(&mut self, height: f32) {
        if self.y - height < MARGIN + FOOTER_H {
            self.new_page();
        }
    }

    pub fn space(&mut self, h: f32) {
        self.y -= h;
    }

    pub fn text_at(&mut self, x: f32, y: f32, size: f32, bold: bool, color: Rgb, s: &str) {
        let font = if bold { "F2" } else { "F1" };
        let _ = writeln!(
            self.page,
            "BT /{font} {size:.1} Tf {:.3} {:.3} {:.3} rg {x:.2} {y:.2} Td ({}) Tj ET",
            color.0,
            color.1,
            color.2,
            escape(s)
        );
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgb) {
        let _ = writeln!(self.page, "{:.3} {:.3} {:.3} rg {x:.2} {y:.2} {w:.2} {h:.2} re f", color.0, color.1, color.2);
    }

    pub fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Rgb, width: f32) {
        let _ = writeln!(self.page, "{:.3} {:.3} {:.3} RG {width:.2} w {x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S", color.0, color.1, color.2);
    }

    pub fn rule(&mut self) {
        self.ensure(10.0);
        self.space(6.0);
        self.line(MARGIN, self.y, PAGE_W - MARGIN, self.y, RULE, 0.6);
        self.space(10.0);
    }

    /// Wrapped paragraph at `x` with width `w`.
    pub fn paragraph_at(&mut self, x: f32, w: f32, s: &str, size: f32, bold: bool, color: Rgb) {
        let leading = size * 1.35;
        for line in wrap(s, size, bold, w) {
            self.ensure(leading);
            self.y -= leading;
            self.text_at(x, self.y + leading * 0.25, size, bold, color, &line);
        }
    }

    pub fn paragraph(&mut self, s: &str, size: f32, bold: bool, color: Rgb) {
        self.paragraph_at(MARGIN, CONTENT_W, s, size, bold, color);
    }

    pub fn heading(&mut self, s: &str) {
        self.ensure(40.0);
        self.space(10.0);
        self.paragraph(s, 12.5, true, INK);
        self.space(2.0);
        self.line(MARGIN, self.y, MARGIN + 36.0, self.y, BRAND, 1.6);
        self.space(8.0);
    }

    /// Two-column label/value list.
    pub fn key_values(&mut self, rows: &[(&str, String)]) {
        let label_w = 150.0;
        for (label, value) in rows {
            let value = if value.trim().is_empty() { "—".to_string() } else { value.clone() };
            let lines = wrap(&value, 9.5, false, CONTENT_W - label_w);
            let leading = 9.5 * 1.4;
            self.ensure(leading * lines.len() as f32 + 2.0);
            let top = self.y;
            self.text_at(MARGIN, top - leading + leading * 0.25, 9.0, false, MUTED, label);
            for (i, line) in lines.iter().enumerate() {
                let y = top - leading * (i as f32 + 1.0) + leading * 0.25;
                self.text_at(MARGIN + label_w, y, 9.5, false, INK, line);
            }
            self.y = top - leading * lines.len() as f32 - 2.0;
        }
    }

    /// Simple table: header row on a band, rows separated by thin rules. Widths are fractions of the content width.
    pub fn table(&mut self, headers: &[&str], widths: &[f32], rows: &[Vec<String>]) {
        let size = 8.5;
        let leading = size * 1.35;
        let cols: Vec<f32> = widths.iter().map(|w| w * CONTENT_W).collect();
        let header = |doc: &mut Document| {
            doc.ensure(leading + 8.0);
            doc.rect(MARGIN, doc.y - leading - 4.0, CONTENT_W, leading + 4.0, BAND);
            let mut x = MARGIN + 4.0;
            for (h, w) in headers.iter().zip(&cols) {
                doc.text_at(x, doc.y - leading + 0.5, size, true, MUTED, h);
                x += w;
            }
            doc.y -= leading + 4.0;
        };
        header(self);
        for row in rows {
            let cells: Vec<Vec<String>> = row.iter().zip(&cols).map(|(c, w)| wrap(c, size, false, w - 8.0)).collect();
            let n = cells.iter().map(Vec::len).max().unwrap_or(1).max(1);
            let h = leading * n as f32 + 5.0;
            if self.y - h < MARGIN + FOOTER_H {
                self.new_page();
                header(self);
            }
            let top = self.y;
            let mut x = MARGIN + 4.0;
            for (lines, w) in cells.iter().zip(&cols) {
                for (i, line) in lines.iter().enumerate() {
                    self.text_at(x, top - 3.0 - leading * (i as f32 + 1.0) + leading * 0.3, size, false, INK, line);
                }
                x += w;
            }
            self.y = top - h;
            self.line(MARGIN, self.y, PAGE_W - MARGIN, self.y, RULE, 0.4);
        }
    }

    /// Finishes the document: footer with page numbers on every page, then serializes it.
    pub fn finish(mut self, created: &str) -> Vec<u8> {
        if !self.page.is_empty() || self.pages.is_empty() {
            self.pages.push(std::mem::take(&mut self.page));
        }
        let total = self.pages.len();
        let footer = self.footer.clone();
        let pages: Vec<String> = self
            .pages
            .into_iter()
            .enumerate()
            .map(|(i, mut content)| {
                let y = MARGIN - 18.0;
                let _ = writeln!(content, "{:.3} {:.3} {:.3} RG 0.4 w {MARGIN:.2} {:.2} m {:.2} {:.2} l S", RULE.0, RULE.1, RULE.2, y + 12.0, PAGE_W - MARGIN, y + 12.0);
                let num = format!("Página {} de {total}", i + 1);
                let num_x = PAGE_W - MARGIN - text_width(&num, 8.0, false);
                let _ = writeln!(content, "BT /F1 8.0 Tf {:.3} {:.3} {:.3} rg {MARGIN:.2} {y:.2} Td ({}) Tj ET", MUTED.0, MUTED.1, MUTED.2, escape(&footer));
                let _ = writeln!(content, "BT /F1 8.0 Tf {:.3} {:.3} {:.3} rg {num_x:.2} {y:.2} Td ({}) Tj ET", MUTED.0, MUTED.1, MUTED.2, escape(&num));
                content
            })
            .collect();
        serialize(&self.title, created, &pages)
    }
}

/// Document metadata uses PDF text strings (not the page font encoding): UTF-16BE with a byte-order mark.
fn text_string(s: &str) -> String {
    let mut out = String::from("<FEFF");
    for unit in s.encode_utf16() {
        let _ = write!(out, "{unit:04X}");
    }
    out.push('>');
    out
}

fn pdf_date(created: &str) -> String {
    // "2026-11-21T13:29:00" (local time) → "D:20261121132900-05'00'" with the machine's current UTC offset.
    let digits: String = created.chars().filter(char::is_ascii_digit).take(14).collect();
    let offset = chrono::Local::now().format("%z").to_string(); // "+0100"
    format!("D:{digits}{}{}'{}'", &offset[..1], &offset[1..3], &offset[3..5])
}

fn serialize(title: &str, created: &str, pages: &[String]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");
    let page_ids: Vec<usize> = (0..pages.len()).map(|i| 6 + 2 * i).collect();
    let mut objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!("<< /Type /Pages /Kids [{}] /Count {} >>", page_ids.iter().map(|id| format!("{id} 0 R")).collect::<Vec<_>>().join(" "), pages.len()).into_bytes(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>".to_vec(),
        format!("<< /Title {} /Producer (AndroidMasters) /Creator (AndroidMasters) /CreationDate ({}) >>", text_string(title), pdf_date(created)).into_bytes(),
    ];
    for (i, content) in pages.iter().enumerate() {
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",
                7 + 2 * i
            )
            .into_bytes(),
        );
        let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        stream.extend_from_slice(content.as_bytes());
        stream.extend_from_slice(b"\nendstream");
        objects.push(stream);
    }
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_and_wrapping() {
        assert!((text_width("AAA", 10.0, false) - 20.01).abs() < 0.01);
        assert!(text_width("m", 10.0, true) > text_width("m", 10.0, false));
        assert_eq!(text_width("á", 10.0, false), text_width("a", 10.0, false));
        let lines = wrap("uno dos tres cuatro cinco seis siete ocho", 10.0, false, 60.0);
        assert!(lines.len() > 1 && lines.iter().all(|l| text_width(l, 10.0, false) <= 60.0));
        assert_eq!(wrap("a\nb", 10.0, false, 100.0), vec!["a", "b"]);
        let long = wrap(&"x".repeat(200), 10.0, false, 50.0);
        assert!(long.len() > 3 && long.iter().all(|l| text_width(l, 10.0, false) <= 50.0));
    }

    #[test]
    fn spanish_text_is_win_ansi_and_parentheses_escaped() {
        assert_eq!(escape("Diagnóstico (FRP) ñ «x» —"), "Diagn\\363stico \\(FRP\\) \\361 \\253x\\273 \\227");
        assert_eq!(escape("emoji 📱"), "emoji ?");
        assert_eq!(escape("Abierta → En curso"), "Abierta \\273 En curso");
        assert_eq!(text_string("Año – 1"), "<FEFF0041006E006F0020201300200031>".replace("006E", "00F1"));
        assert!(pdf_date("2026-11-21T13:29:00").starts_with("D:20261121132900") && pdf_date("2026-11-21T13:29:00").ends_with('\''));
    }

    #[test]
    fn valid_structure_with_page_breaks() {
        let mut doc = Document::new("Prueba", "AndroidMasters");
        for i in 0..200 {
            doc.paragraph(&format!("Línea {i}"), 10.0, false, INK);
        }
        let bytes = doc.finish("2026-11-21T13:29:00");
        let text = String::from_utf8_lossy(&bytes);
        assert!(bytes.starts_with(b"%PDF-1.4") && text.trim_end().ends_with("%%EOF"));
        let count: usize = text.split("/Count ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
        assert!(count >= 3, "200 lines need several pages");
        assert!(text.contains(&format!("P\\341gina {count} de {count}")));
        // Every xref offset points at its object.
        let start: usize = text.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
        let xref = std::str::from_utf8(&bytes[start..]).unwrap();
        for (n, line) in xref.lines().skip(3).take_while(|l| l.ends_with(" n ")).enumerate() {
            let off: usize = line[..10].parse().unwrap();
            assert!(bytes[off..].starts_with(format!("{} 0 obj", n + 1).as_bytes()), "object {}", n + 1);
        }
    }
}
