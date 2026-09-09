//! What a MagicDNS fallback pin is, and when a hosts file is done being edited.
//!
//! A PIN IS THE FALLBACK that answers for a tailnet name when MagicDNS is
//! unavailable, so it must be exactly right or not there at all: a stale pin
//! resolves confidently to the wrong host, which is worse than no pin. Every
//! rule here follows from that one sentence.

/// The address a working localhost record must map.
///
/// THE ONE ADDRESS, not the whole `127.0.0.0/8` block. The block is loopback,
/// but the record a machine actually resolves `localhost` through is this one,
/// and a gate that accepted `127.0.0.2` would pass a file on which localhost
/// still does not resolve.
pub const LOOPBACK: &[u8] = b"127.0.0.1";

/// Written between the columns of the record this installs. `hosts(5)`: "Items
/// are separated by any number of blanks and/or tab characters."
const FIELD_SEPARATOR: u8 = b'\t';

/// Bytes a pin field may not contain.
///
/// Space, tab, form feed and newline split one record into extra columns or
/// extra LINES when the file is read back, and `#` starts a comment that
/// truncates the record. THE CARRIAGE RETURN IS HERE FOR A DIFFERENT REASON: it
/// does not split a record, but a field carrying one would be written out as a
/// name the reader then sees as a different name, so the record could never
/// converge and every apply would rewrite the file.
///
/// Other Unicode spaces are absent on purpose. None of them splits a hosts
/// record, so none of them can smuggle a column.
const FORBIDDEN_IN_A_COLUMN: [u8; 6] = [b' ', b'\t', b'\n', b'\r', 0x0c, b'#'];

/// A file is converged when EXACTLY this many lines name the pin.
///
/// EXACTLY, not at least. Testing that a correct line EXISTS let a correct line
/// plus a stale duplicate read as converged, which leaves two lines naming the
/// pin and the resolver picking one.
const CONVERGED_CLAIMING_LINES: usize = 1;

/// One pin: the three columns of the one record it converges on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    fqdn: Vec<u8>,
    ip: Vec<u8>,
    short: Vec<u8>,
}

/// Which field of a pin could not be used, for a refusal that names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Fqdn,
    Ip,
    Short,
}

impl Field {
    pub fn as_str(self) -> &'static str {
        match self {
            Field::Fqdn => "fqdn",
            Field::Ip => "ip",
            Field::Short => "short",
        }
    }
}

impl Pin {
    /// A pin, or the first field that is not usable as exactly one hosts
    /// column.
    ///
    /// RE-CHECKED HERE even though the caller that renders these refuses them
    /// too. This is a standalone component with its own contract, and a field
    /// that is not one column would write extra columns, or extra LINES, into a
    /// file being rewritten as root.
    pub fn new(fqdn: &[u8], ip: &[u8], short: &[u8]) -> Result<Self, Field> {
        for (field, value) in [(Field::Fqdn, fqdn), (Field::Ip, ip), (Field::Short, short)] {
            if !is_single_column(value) {
                return Err(field);
            }
        }
        Ok(Pin {
            fqdn: fqdn.to_vec(),
            ip: ip.to_vec(),
            short: short.to_vec(),
        })
    }

    /// The two names this pin owns.
    ///
    /// IT OWNS ITS SHORT NAME TOO, by decision: the fallback exists to answer
    /// for BOTH names, so an unrelated line claiming the short name is dropped
    /// rather than left to compete. Which of two such lines the resolver would
    /// pick was never measured, and ownership makes the question moot.
    pub fn names(&self) -> [&[u8]; 2] {
        [&self.fqdn, &self.short]
    }

    /// The fully qualified name, for the messages that name the pin.
    pub fn fqdn(&self) -> &[u8] {
        &self.fqdn
    }

    /// The one canonical record this pin converges on, with no terminator.
    pub fn record(&self) -> Vec<u8> {
        let mut record = self.ip.clone();
        record.push(FIELD_SEPARATOR);
        record.extend_from_slice(&self.fqdn);
        record.push(FIELD_SEPARATOR);
        record.extend_from_slice(&self.short);
        record
    }
}

/// Is this value usable as exactly one hosts column?
fn is_single_column(value: &[u8]) -> bool {
    !value.is_empty()
        && !value
            .iter()
            .any(|byte| FORBIDDEN_IN_A_COLUMN.contains(byte))
}

/// What one walk of the file found, and the whole of what convergence is
/// decided from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Survey {
    /// How many lines claim either of the pin's names.
    pub claiming_lines: usize,
    /// Whether one of them is byte for byte the record this pin wants.
    pub desired_record_present: bool,
    /// Whether the file's last line ends with a terminator.
    pub ends_with_terminator: bool,
}

impl Survey {
    /// Is the file done, or does it need rebuilding?
    ///
    /// THE TERMINATOR IS A CONVERGENCE CONDITION, not a cosmetic one, and it is
    /// the one only a rebuild can satisfy. The resolver reads an unterminated
    /// final line one byte short of what the file holds, so a file whose final
    /// unterminated line WAS the pin's record read as converged while the
    /// resolver read that record as naming something else. Reporting success on
    /// a file the resolver reads differently from its bytes is the same
    /// fail-quiet this tool exists to close, one door over.
    pub fn is_converged(&self) -> bool {
        self.claiming_lines == CONVERGED_CLAIMING_LINES
            && self.desired_record_present
            && self.ends_with_terminator
    }
}

#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
