use posture_application::InspectionFailure;
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::process::{Command, Stdio};

pub(super) struct PendingInput<'a> {
    writer: Option<io::PipeWriter>,
    bytes: &'a [u8],
}
impl<'a> PendingInput<'a> {
    pub(super) fn prepare(
        command: &mut Command,
        bytes: &'a [u8],
    ) -> Result<Self, InspectionFailure> {
        let (reader, writer) = io::pipe().map_err(|_| InspectionFailure::Unavailable)?;
        nonblocking(&writer)?;
        command.stdin(Stdio::from(reader));
        Ok(Self {
            writer: if bytes.is_empty() { None } else { Some(writer) },
            bytes,
        })
    }
    pub(super) fn write_pending(&mut self) -> Result<(), InspectionFailure> {
        let Some(writer) = &mut self.writer else {
            return Ok(());
        };
        // One bounded write per loop lets stdout draining and the absolute deadline advance.
        match writer.write(&self.bytes[..self.bytes.len().min(4096)]) {
            Ok(0) => return Err(InspectionFailure::Failed),
            Ok(count) => self.bytes = &self.bytes[count..],
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err(InspectionFailure::Failed),
        }
        if self.bytes.is_empty() {
            self.writer = None;
        }
        Ok(())
    }
}

pub(super) fn nonblocking(descriptor: &impl AsRawFd) -> Result<(), InspectionFailure> {
    // This pipe endpoint is owned here. Its flags cannot affect the opposite endpoint.
    let fd = descriptor.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1 {
        Err(InspectionFailure::Unavailable)
    } else {
        Ok(())
    }
}
