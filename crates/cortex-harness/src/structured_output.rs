//! Existing trusted-fixture JSON-lines observation, without a new wire schema.
//!
//! The first valid stdout JSON value is startup; EOF may complete a final value.
//! There are no payload request/version/kind/terminal fields in this contract.
//! Correlation is retained separately using existing canonical intent/binding.
//! Parsed output grants no authority or external-effect acceptance.
use cortex_core::program::{
    OperationId,
    domain::{DomainBindingObservation, DomainIntent},
    supervision::{InterruptionReason, ProcessLimits},
};

pub(crate) struct Framing {
    pub(crate) bytes: Vec<u8>,
    pub(crate) records: u64,
    pub(crate) malformed: bool,
    pub(crate) record_limit: Option<u64>,
}
impl Framing {
    fn valid(&self) -> bool {
        if self.record_limit.is_some() {
            // The extended transport validates JSON syntax without constructing
            // a semantic tree. Historical fixtures keep their original parser.
            serde_json::from_slice::<&serde_json::value::RawValue>(&self.bytes).is_ok()
        } else {
            serde_json::from_slice::<serde_json::Value>(&self.bytes).is_ok()
        }
    }
    fn record(&mut self) -> Result<(), InterruptionReason> {
        self.records = self
            .records
            .checked_add(1)
            .ok_or(InterruptionReason::EventLimit)?;
        if self.record_limit.is_some_and(|max| self.records > max) {
            return Err(InterruptionReason::EventLimit);
        }
        Ok(())
    }
    pub(crate) fn finish(&mut self) -> Result<(), InterruptionReason> {
        if !self.bytes.is_empty() {
            if !self.valid() {
                self.malformed = true;
                return Err(InterruptionReason::MalformedOutput);
            }
            self.record()?;
            self.bytes.clear();
        }
        Ok(())
    }
    pub(crate) fn feed(&mut self, input: &[u8], max: u64) -> Result<(), InterruptionReason> {
        for byte in input {
            if *byte == b'\n' {
                if self.bytes.is_empty() || !self.valid() {
                    self.malformed = true;
                    return Err(InterruptionReason::MalformedOutput);
                }
                self.record()?;
                self.bytes.clear();
            } else {
                if self.bytes.len() as u64 >= max {
                    self.malformed = true;
                    return Err(InterruptionReason::MalformedOutput);
                }
                self.bytes.push(*byte);
            }
        }
        Ok(())
    }
}

/// Finite syntax/timing summary. No raw private record is serialized or formatted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredSummary {
    /// Syntax-valid records parsed before interruption, including the first
    /// over-limit record when EventLimit occurs. An overflowed raw read may
    /// retain a prefix not yet parsed; this is not an accepted-work count.
    pub records: u64,
    /// First complete valid record, relative to the original admission start.
    pub first_record_ms: Option<u64>,
    /// Original syntax/limit/timing interruption, never cleared by later input.
    pub interruption: Option<InterruptionReason>,
    /// True only after observed stdout EOF with the final record accepted.
    pub stdout_finished: bool,
}
/// Incremental, explicitly selected JSON-lines consumer. It retains only one
/// finite frame, not a semantic JSON tree or unbounded event history.
pub struct StructuredObserver {
    framing: Framing,
    frame_bytes: u64,
    startup_ms: u64,
    first: Option<u64>,
    reason: Option<InterruptionReason>,
    finished: bool,
    last_ms: u64,
}
impl StructuredObserver {
    /// Validate explicit limits and reserve the frame before consuming bytes.
    /// raw_capacity is the total allocated stdout/stderr capture capacity; this
    /// consumer accounts it in addition to input, frame and 8 KiB read scratch.
    pub fn new(
        limits: &ProcessLimits,
        input_capacity: u64,
        raw_capacity: u64,
    ) -> Result<Self, InterruptionReason> {
        let t = limits
            .transport
            .as_ref()
            .ok_or(InterruptionReason::BufferLimit)?;
        if limits.wall_ms == 0
            || t.startup_ms == 0
            || t.startup_ms > limits.wall_ms
            || t.idle_ms == 0
            || t.idle_ms > limits.wall_ms
            || t.records == 0
            || limits.frame_bytes == 0
            || limits.frame_bytes > 1024 * 1024
            || limits.frame_bytes > limits.output_bytes
        {
            return Err(InterruptionReason::BufferLimit);
        }
        let cap =
            usize::try_from(limits.frame_bytes).map_err(|_| InterruptionReason::BufferLimit)?;
        let minimum = input_capacity
            .checked_add(raw_capacity)
            .and_then(|n| n.checked_add(limits.frame_bytes))
            .and_then(|n| n.checked_add(8192))
            .ok_or(InterruptionReason::BufferLimit)?;
        if minimum > t.buffer_bytes {
            return Err(InterruptionReason::BufferLimit);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(cap)
            .map_err(|_| InterruptionReason::BufferLimit)?;
        let total = input_capacity
            .checked_add(raw_capacity)
            .and_then(|n| n.checked_add(bytes.capacity() as u64))
            .and_then(|n| n.checked_add(8192))
            .ok_or(InterruptionReason::BufferLimit)?;
        if total > t.buffer_bytes {
            return Err(InterruptionReason::BufferLimit);
        }
        Ok(Self {
            framing: Framing {
                bytes,
                records: 0,
                malformed: false,
                record_limit: Some(t.records),
            },
            frame_bytes: limits.frame_bytes,
            startup_ms: t.startup_ms,
            first: None,
            reason: None,
            finished: false,
            last_ms: 0,
        })
    }
    fn remember(
        &mut self,
        result: Result<(), InterruptionReason>,
    ) -> Result<(), InterruptionReason> {
        if let Err(e) = result {
            self.reason.get_or_insert(e);
        }
        self.reason.map_or(Ok(()), Err)
    }
    /// Preserve the original clock and first interruption; regression refuses.
    pub fn check(&mut self, elapsed_ms: u64) -> Result<(), InterruptionReason> {
        if elapsed_ms < self.last_ms {
            return self.remember(Err(InterruptionReason::AuthorityUnknown));
        }
        self.last_ms = elapsed_ms;
        if let Some(reason) = self.reason {
            return Err(reason);
        }
        if self.first.is_none() && elapsed_ms >= self.startup_ms {
            return self.remember(Err(InterruptionReason::StartupTimeout));
        }
        Ok(())
    }
    /// Consume only bytes actually read from the correlated stdout stream.
    /// Each input byte is scanned once; JSON syntax is checked once per complete
    /// frame. There is no whole-prefix reparsing on each partial read.
    pub fn feed(&mut self, bytes: &[u8], elapsed_ms: u64) -> Result<(), InterruptionReason> {
        self.check(elapsed_ms)?;
        if self.finished {
            return self.remember(Err(InterruptionReason::MalformedOutput));
        }
        let result = self.framing.feed(bytes, self.frame_bytes);
        if self.first.is_none() && self.framing.records > 0 {
            self.first = Some(elapsed_ms);
        }
        self.remember(result)
    }
    /// Complete a valid final non-newline record at EOF. Empty startup refuses;
    /// this EOF observation is not a successful process or whole-domain result.
    pub fn finish(&mut self, elapsed_ms: u64) -> Result<(), InterruptionReason> {
        self.check(elapsed_ms)?;
        if self.finished {
            return self.reason.map_or(Ok(()), Err);
        }
        let result = self.framing.finish();
        if self.first.is_none() && self.framing.records > 0 {
            self.first = Some(elapsed_ms);
        }
        self.remember(result)?;
        if self.framing.records == 0 {
            return self.remember(Err(InterruptionReason::StartupIncomplete));
        }
        self.finished = true;
        Ok(())
    }
    /// Record an original external interruption such as cancellation/timeout.
    /// A later complete frame cannot replace that observation.
    pub fn interrupt(&mut self, reason: InterruptionReason) {
        self.reason.get_or_insert(reason);
    }
    /// Redacted finite summary. No output or source data is formatted.
    pub fn summary(&self) -> StructuredSummary {
        StructuredSummary {
            records: self.framing.records,
            first_record_ms: self.first,
            interruption: self.reason,
            stdout_finished: self.finished,
        }
    }
}

/// Read-only correlation to the existing canonical domain and release. This is
/// not a new provenance vocabulary, journal input, grant or signed observation.
#[derive(Clone, PartialEq, Eq)]
pub struct BoundStructuredObservation {
    intent: DomainIntent,
    binding: DomainBindingObservation,
    release: OperationId,
    summary: StructuredSummary,
}
impl BoundStructuredObservation {
    /// Bind a syntax observation to the already selected stream's full canonical
    /// context. Caller authenticity/liveness remain outside this pure constructor.
    pub fn new(
        intent: &DomainIntent,
        binding: &DomainBindingObservation,
        release: &OperationId,
        summary: StructuredSummary,
    ) -> Result<Self, InterruptionReason> {
        if binding.validate().is_err()
            || binding.identity.pool != intent.pool
            || binding.identity.request != intent.request
            || binding.configuration != intent.configuration
        {
            return Err(InterruptionReason::AuthorityLost);
        }
        Ok(Self {
            intent: intent.clone(),
            binding: binding.clone(),
            release: release.clone(),
            summary,
        })
    }
    /// Refuse changed request/member/package/generation/configuration/incarnation
    /// or release identity. No payload JSON field is treated as authority.
    pub fn check(
        &self,
        intent: &DomainIntent,
        binding: &DomainBindingObservation,
        release: &OperationId,
    ) -> Result<(), InterruptionReason> {
        if &self.intent != intent || &self.binding != binding || &self.release != release {
            Err(InterruptionReason::AuthorityLost)
        } else {
            Ok(())
        }
    }
    /// Syntax/timing observation, independently of process exit and acceptance.
    pub fn summary(&self) -> &StructuredSummary {
        &self.summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn limits() -> ProcessLimits {
        ProcessLimits {
            wall_ms: 1000,
            cleanup_ms: 100,
            input_bytes: 1024,
            output_bytes: 1024,
            frame_bytes: 64,
            authority_poll_ms: 1,
            executable_bytes: 4096,
            arguments: 0,
            argument_bytes: 0,
            transport: Some(cortex_core::program::supervision::TransportLimits {
                startup_ms: 100,
                idle_ms: 200,
                stdout_bytes: 1024,
                stderr_bytes: 1024,
                records: 8,
                buffer_bytes: 16384,
            }),
        }
    }
    fn observer() -> StructuredObserver {
        StructuredObserver::new(&limits(), 1024, 2048).unwrap()
    }
    #[test]
    fn every_split_and_multiframe_eof_have_fixed_oracles() {
        let data = b"{\"x\":1}\n[true,false]\n\"last\"";
        for split in 0..=data.len() {
            let mut p = observer();
            p.feed(&data[..split], 1).unwrap();
            p.feed(&data[split..], 2).unwrap();
            p.finish(3).unwrap();
            let s = p.summary();
            assert_eq!(s.records, 3);
            assert!(s.stdout_finished);
            assert_eq!(s.interruption, None);
        }
        let mut p = observer();
        for byte in data {
            p.feed(std::slice::from_ref(byte), 1).unwrap();
        }
        p.finish(2).unwrap();
        assert_eq!(p.summary().records, 3);
        let unicode = "\"é☃\"\n".as_bytes();
        for split in 0..=unicode.len() {
            let mut p = observer();
            p.feed(&unicode[..split], 1).unwrap();
            p.feed(&unicode[split..], 2).unwrap();
            p.finish(3).unwrap();
            assert_eq!(p.summary().records, 1);
        }
    }
    #[test]
    fn canonical_generic_json_does_not_invent_startup_or_terminal_schema() {
        let mut p = observer();
        p.feed(b"{\"type\":\"terminal\",\"version\":999}\n{\"type\":\"startup\",\"request\":\"other\"}\n",1).unwrap();
        p.finish(2).unwrap();
        assert_eq!(p.summary().records, 2);
        let mut adjacent = observer();
        adjacent.feed(b"0\n1\n", 2).unwrap();
        adjacent.finish(3).unwrap();
        assert_eq!(adjacent.summary().records, 2);
    }
    #[test]
    fn malformed_boundaries_and_original_failure_are_not_success() {
        for input in [
            b"\n".as_slice(),
            b" ".as_slice(),
            b"{",
            b"1 trailing",
            b"\"\\xff\"",
            b"{}{}",
        ] {
            let mut p = observer();
            let first = p.feed(input, 1).and_then(|_| p.finish(2));
            assert_eq!(first, Err(InterruptionReason::MalformedOutput));
            assert_eq!(p.feed(b"{}\n", 3), Err(InterruptionReason::MalformedOutput));
            assert!(!p.summary().stdout_finished);
        }
        let mut p = observer();
        assert_eq!(p.finish(1), Err(InterruptionReason::StartupIncomplete));
        let mut p = observer();
        p.interrupt(InterruptionReason::Cancelled);
        assert_eq!(p.feed(b"{}\n", 2), Err(InterruptionReason::Cancelled));
    }
    #[test]
    fn exact_frame_and_event_limits_reject_neighbors() {
        let mut l = limits();
        l.frame_bytes = 4;
        l.transport.as_mut().unwrap().records = 2;
        let mut p = StructuredObserver::new(&l, 0, 0).unwrap();
        p.feed(b"null\ntrue", 1).unwrap();
        p.finish(2).unwrap();
        assert_eq!(p.summary().records, 2);
        let mut p = StructuredObserver::new(&l, 0, 0).unwrap();
        assert_eq!(
            p.feed(b"false\n", 1),
            Err(InterruptionReason::MalformedOutput)
        );
        let mut p = StructuredObserver::new(&l, 0, 0).unwrap();
        assert_eq!(p.feed(b"0\n1\n2\n", 1), Err(InterruptionReason::EventLimit));
        let mut l = limits();
        l.transport.as_mut().unwrap().buffer_bytes = 1;
        assert!(matches!(
            StructuredObserver::new(&l, u64::MAX, 1),
            Err(InterruptionReason::BufferLimit)
        ));
    }
    #[test]
    fn counter_overflow_and_missing_limits_refuse_without_allocation() {
        let mut f = Framing {
            bytes: vec![],
            records: u64::MAX,
            malformed: false,
            record_limit: Some(u64::MAX),
        };
        assert_eq!(f.feed(b"0\n", 1), Err(InterruptionReason::EventLimit));
        for (frame, output, events) in [(0, 1, 1), (u64::MAX, u64::MAX, 1), (1, 1, 0)] {
            let mut l = limits();
            l.frame_bytes = frame;
            l.output_bytes = output;
            l.transport.as_mut().unwrap().records = events;
            assert!(matches!(
                StructuredObserver::new(&l, 0, 0),
                Err(InterruptionReason::BufferLimit)
            ));
        }
    }
    #[test]
    fn startup_uses_original_time_not_first_read_or_stderr_progress() {
        let mut p = observer();
        p.feed(b"{", 99).unwrap();
        assert_eq!(p.feed(b"}\n", 100), Err(InterruptionReason::StartupTimeout));
        let mut p = observer();
        p.feed(b"{}", 99).unwrap();
        p.finish(99).unwrap();
        p.check(200).unwrap();
        assert_eq!(p.summary().first_record_ms, Some(99));
        let mut p = observer();
        p.check(20).unwrap();
        assert_eq!(
            p.feed(b"{}\n", 19),
            Err(InterruptionReason::AuthorityUnknown)
        );
    }
    #[test]
    fn invalid_utf8_and_eof_mid_token_reject_without_cross_operation_mutation() {
        let mut bad = observer();
        assert_eq!(
            bad.feed(&[b'"', 0xff, b'"', b'\n'], 1),
            Err(InterruptionReason::MalformedOutput)
        );
        let mut good = observer();
        good.feed(b"{}\n", 1).unwrap();
        good.finish(2).unwrap();
        assert_eq!(good.summary().records, 1);
        for input in [b"\"".as_slice(), b"[0,", b"{\"x\":", b"tru"] {
            let mut p = observer();
            p.feed(input, 1).unwrap();
            assert_eq!(p.finish(2), Err(InterruptionReason::MalformedOutput));
        }
    }
}
