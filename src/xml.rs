//! XML escaping and UTF-16 encoding helpers for `.anx` output.
//!
//! `.anx` is XML text encoded as UTF-16 LE with a BOM. The Python builder emits
//! the tree as strings, escaping `& < > "` and stripping the control characters
//! XML 1.0 forbids (U+0000–U+001F except TAB/LF/CR); ANB rejects documents that
//! contain those. We replicate exactly that escaping so output stays valid.

/// Escape a string for use as XML text or a double-quoted attribute value, and
/// drop XML-1.0-forbidden control characters.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // Allowed control chars: TAB (\t), LF (\n), CR (\r).
            '\t' | '\n' | '\r' => out.push(c),
            // Strip the remaining C0 controls XML 1.0 forbids.
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

/// An attribute pair for [`Writer`]; the value is escaped on write.
pub type Attr<'a> = (&'a str, String);

/// A destination for XML text fragments. Implemented by [`String`] (materialize
/// in memory) and [`Utf16Sink`] (encode to UTF-16 and stream to a writer), so the
/// same emit code serves both the buffered and the low-memory streaming paths.
pub trait XmlSink {
    fn write_str(&mut self, s: &str);
}

impl XmlSink for String {
    fn write_str(&mut self, s: &str) {
        self.push_str(s);
    }
}

/// A streaming sink that encodes fragments to UTF-16 LE (BOM first) and flushes
/// to an inner writer in bounded chunks — peak memory stays at the buffer size
/// regardless of document length. IO errors are captured and surfaced by
/// [`Utf16Sink::finish`].
pub struct Utf16Sink<W: std::io::Write> {
    inner: W,
    buf: Vec<u8>,
    err: Option<std::io::Error>,
}

const STREAM_FLUSH_BYTES: usize = 1 << 16; // 64 KiB

impl<W: std::io::Write> Utf16Sink<W> {
    /// Create a sink, writing the UTF-16 BOM up front.
    pub fn new(mut inner: W) -> std::io::Result<Self> {
        inner.write_all(&[0xFF, 0xFE])?;
        Ok(Self {
            inner,
            buf: Vec::with_capacity(STREAM_FLUSH_BYTES + 1024),
            err: None,
        })
    }

    /// Flush the remaining buffer and return any captured IO error.
    pub fn finish(mut self) -> std::io::Result<()> {
        if let Some(e) = self.err {
            return Err(e);
        }
        self.inner.write_all(&self.buf)?;
        self.inner.flush()
    }
}

impl<W: std::io::Write> XmlSink for Utf16Sink<W> {
    fn write_str(&mut self, s: &str) {
        if self.err.is_some() {
            return;
        }
        for u in s.encode_utf16() {
            self.buf.extend_from_slice(&u.to_le_bytes());
        }
        if self.buf.len() >= STREAM_FLUSH_BYTES {
            if let Err(e) = self.inner.write_all(&self.buf) {
                self.err = Some(e);
            }
            self.buf.clear();
        }
    }
}

/// A streaming sink that writes fragments as UTF-8 bytes to an inner writer in
/// bounded chunks — peak memory stays at the buffer size regardless of document
/// length. This is the string/XML analogue of [`Utf16Sink`] (Python's
/// `iter_xml`): the declaration says `utf-8` and no transcoding happens, so it is
/// the low-memory path for serving an XML body. IO errors are captured and
/// surfaced by [`Utf8Sink::finish`].
pub struct Utf8Sink<W: std::io::Write> {
    inner: W,
    buf: Vec<u8>,
    err: Option<std::io::Error>,
}

impl<W: std::io::Write> Utf8Sink<W> {
    /// Create a sink over `inner`.
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            buf: Vec::with_capacity(STREAM_FLUSH_BYTES + 1024),
            err: None,
        }
    }

    /// Flush the remaining buffer and return any captured IO error.
    pub fn finish(mut self) -> std::io::Result<()> {
        if let Some(e) = self.err {
            return Err(e);
        }
        self.inner.write_all(&self.buf)?;
        self.inner.flush()
    }
}

impl<W: std::io::Write> XmlSink for Utf8Sink<W> {
    fn write_str(&mut self, s: &str) {
        if self.err.is_some() {
            return;
        }
        self.buf.extend_from_slice(s.as_bytes());
        if self.buf.len() >= STREAM_FLUSH_BYTES {
            if let Err(e) = self.inner.write_all(&self.buf) {
                self.err = Some(e);
            }
            self.buf.clear();
        }
    }
}

/// A minimal XML writer that emits to any [`XmlSink`].
///
/// In `compact` mode (the `.anx` file default) output is newline-separated with
/// no indentation; in pretty mode each nesting level is indented by two spaces,
/// matching Python's `to_xml(compact=False)`. Attributes are emitted in the
/// order given, which keeps output deterministic.
pub struct Writer<'s> {
    out: &'s mut dyn XmlSink,
    compact: bool,
    depth: usize,
}

impl<'s> Writer<'s> {
    /// Create a writer over `out`. `compact` drops indentation.
    pub fn new(out: &'s mut dyn XmlSink, compact: bool) -> Self {
        Self {
            out,
            compact,
            depth: 0,
        }
    }

    fn indent(&mut self) {
        if !self.compact {
            for _ in 0..self.depth {
                self.out.write_str("  ");
            }
        }
    }

    /// `<?xml version='1.0' encoding='{encoding}'?>`.
    ///
    /// The declaration must name the encoding of the bytes the caller actually
    /// hands back: `utf-8` for the string forms (which return a Rust `String`,
    /// UTF-8 in memory) and `utf-16` for the `.anx` byte writer (UTF-16 LE + BOM).
    /// This mirrors upstream 1.25.0, which threads the same distinction through
    /// its serializers.
    pub fn declaration(&mut self, encoding: &str) {
        self.out.write_str("<?xml version='1.0' encoding='");
        self.out.write_str(encoding);
        self.out.write_str("'?>\n");
    }

    /// A provenance comment (excluded from conformance digests upstream).
    pub fn comment(&mut self, text: &str) {
        self.out.write_str("<!-- ");
        self.out.write_str(text);
        self.out.write_str(" -->\n");
    }

    /// Open tag `<Tag a="..">` on its own line (increments nesting depth).
    pub fn open(&mut self, tag: &str, attrs: &[Attr]) {
        self.write_tag(tag, attrs, false);
        self.depth += 1;
    }

    /// Self-closing tag `<Tag a=".."/>` on its own line.
    pub fn empty(&mut self, tag: &str, attrs: &[Attr]) {
        self.write_tag(tag, attrs, true);
    }

    /// A text-content element `<Tag>escaped text</Tag>` on one line.
    pub fn text_element(&mut self, tag: &str, text: &str) {
        self.indent();
        self.out.write_str("<");
        self.out.write_str(tag);
        self.out.write_str(">");
        self.out.write_str(&escape(text));
        self.out.write_str("</");
        self.out.write_str(tag);
        self.out.write_str(">\n");
    }

    /// Close tag `</Tag>` on its own line (decrements nesting depth).
    pub fn close(&mut self, tag: &str) {
        self.depth = self.depth.saturating_sub(1);
        self.indent();
        self.out.write_str("</");
        self.out.write_str(tag);
        self.out.write_str(">\n");
    }

    fn write_tag(&mut self, tag: &str, attrs: &[Attr], self_closing: bool) {
        self.indent();
        self.out.write_str("<");
        self.out.write_str(tag);
        for (name, value) in attrs {
            self.out.write_str(" ");
            self.out.write_str(name);
            self.out.write_str("=\"");
            self.out.write_str(&escape(value));
            self.out.write_str("\"");
        }
        self.out
            .write_str(if self_closing { "/>\n" } else { ">\n" });
    }
}

/// Encode an XML string to UTF-16 LE bytes with a leading BOM (`FF FE`), the
/// on-disk form of an `.anx` file.
pub fn to_utf16le_with_bom(s: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(2 + s.len() * 2);
    bytes.extend_from_slice(&[0xFF, 0xFE]);
    for unit in s.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_markup_chars() {
        assert_eq!(
            escape("a & b < c > d \" e"),
            "a &amp; b &lt; c &gt; d &quot; e"
        );
    }

    #[test]
    fn keeps_allowed_whitespace_strips_forbidden_controls() {
        // TAB/LF/CR survive; U+0001 is dropped entirely.
        assert_eq!(escape("a\tb\nc\rd\u{0001}e"), "a\tb\nc\rde");
    }

    #[test]
    fn writer_emits_compact_lines() {
        let mut s = String::new();
        {
            let mut w = Writer::new(&mut s, true);
            w.open("Chart", &[]);
            w.empty(
                "Strength",
                &[("Id", "ID1".into()), ("Name", "A & B".into())],
            );
            w.close("Chart");
        }
        assert_eq!(
            s,
            "<Chart>\n<Strength Id=\"ID1\" Name=\"A &amp; B\"/>\n</Chart>\n"
        );
    }

    #[test]
    fn utf16_sink_streams_with_bom() {
        let mut out: Vec<u8> = Vec::new();
        {
            let mut sink = Utf16Sink::new(&mut out).unwrap();
            {
                let mut w = Writer::new(&mut sink, true);
                w.empty("A", &[]);
            }
            sink.finish().unwrap();
        }
        // BOM then "<A/>\n" in UTF-16 LE.
        assert_eq!(&out[..2], &[0xFF, 0xFE]);
        let units: Vec<u16> = out[2..]
            .chunks(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(String::from_utf16(&units).unwrap(), "<A/>\n");
    }

    #[test]
    fn bom_then_utf16le() {
        let b = to_utf16le_with_bom("A");
        assert_eq!(b, vec![0xFF, 0xFE, 0x41, 0x00]);
    }
}
