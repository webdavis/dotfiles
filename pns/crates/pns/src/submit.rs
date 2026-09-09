use pns_protocol::{DecodedRequest, MAX_BYTES, ResultEnvelope, Status, decode_request};
use std::io::{self, Read, Write};

pub fn run(
    args: &[String],
    input: impl Read,
    mut output: impl Write,
    submit: impl FnOnce(DecodedRequest) -> ResultEnvelope,
) -> io::Result<Status> {
    let result = if args.len() == 1 && args[0] == "--json" {
        receive(input, submit)
    } else {
        refusal("submit_usage")
    };
    let encoded = result.encode().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "pns result exceeds protocol bounds",
        )
    })?;
    writeln!(output, "{encoded}")?;
    Ok(result.status)
}

fn receive(
    input: impl Read,
    submit: impl FnOnce(DecodedRequest) -> ResultEnvelope,
) -> ResultEnvelope {
    let mut bytes = Vec::new();
    if input
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return refusal("input_unreadable");
    }
    match decode_request(&bytes) {
        Ok(decoded) => submit(decoded),
        Err(rejected) => ResultEnvelope::rejected(&rejected),
    }
}

fn refusal(code: &str) -> ResultEnvelope {
    ResultEnvelope {
        request_id: None,
        status: Status::Rejected,
        decision_id: None,
        interaction: None,
        destinations: Vec::new(),
        diagnostics: vec![code.into()],
    }
}

#[cfg(test)]
mod tests;
