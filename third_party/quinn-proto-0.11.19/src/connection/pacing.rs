//! Pacing of packet transmissions.

use crate::{Duration, Instant};

use tracing::warn;

/// A simple token-bucket pacer
///
/// The pacer's capacity is derived on a fraction of the congestion window
/// which can be sent in regular intervals
/// Once the bucket is empty, further transmission is blocked.
/// The bucket refills at a rate slightly faster
/// than one congestion window per RTT, as recommended in
/// <https://tools.ietf.org/html/draft-ietf-quic-recovery-34#section-7.7>
pub(super) struct Pacer {
    capacity: u64,
    last_window: u64,
    last_mtu: u16,
    tokens: u64,
    prev: Instant,
    last_rate: Option<u64>,
    fractional_tokens: u128,
}

impl Pacer {
    /// Obtains a new [`Pacer`].
    pub(super) fn new(smoothed_rtt: Duration, window: u64, mtu: u16, now: Instant) -> Self {
        let capacity = optimal_capacity(smoothed_rtt, window, mtu);
        Self {
            capacity,
            last_window: window,
            last_mtu: mtu,
            tokens: capacity,
            prev: now,
            last_rate: None,
            fractional_tokens: 0,
        }
    }

    /// Record that a packet has been transmitted.
    pub(super) fn on_transmit(&mut self, packet_length: u16) {
        self.tokens = self.tokens.saturating_sub(packet_length.into())
    }

    /// Return how long we need to wait before sending `bytes_to_send`
    ///
    /// If we can send a packet right away, this returns `None`. Otherwise, returns `Some(d)`,
    /// where `d` is the time before this function should be called again.
    ///
    /// The 5/4 ratio used here comes from the suggestion that N = 1.25 in the draft IETF RFC for
    /// QUIC.
    #[cfg(test)]
    pub(super) fn delay(
        &mut self,
        smoothed_rtt: Duration,
        bytes_to_send: u64,
        mtu: u16,
        window: u64,
        now: Instant,
    ) -> Option<Instant> {
        self.delay_with_rate(smoothed_rtt, bytes_to_send, mtu, window, now, None)
    }

    pub(super) fn delay_with_rate(
        &mut self,
        smoothed_rtt: Duration,
        bytes_to_send: u64,
        mtu: u16,
        window: u64,
        now: Instant,
        rate_bps: Option<u64>,
    ) -> Option<Instant> {
        if let Some(rate_bps) = rate_bps {
            return self.delay_capped(bytes_to_send, mtu, now, rate_bps.max(1));
        }

        if self.last_rate.take().is_some() {
            self.capacity = optimal_capacity(smoothed_rtt, window, mtu);
            self.tokens = self.tokens.min(self.capacity);
            self.prev = now;
            self.fractional_tokens = 0;
        }
        debug_assert_ne!(
            window, 0,
            "zero-sized congestion control window is nonsense"
        );

        if window != self.last_window || mtu != self.last_mtu {
            self.capacity = optimal_capacity(smoothed_rtt, window, mtu);

            // Clamp the tokens
            self.tokens = self.capacity.min(self.tokens);
            self.last_window = window;
            self.last_mtu = mtu;
        }

        // if we can already send a packet, there is no need for delay
        if self.tokens >= bytes_to_send {
            return None;
        }

        // we disable pacing for extremely large windows
        if window > u64::from(u32::MAX) {
            return None;
        }

        let window = window as u32;

        let time_elapsed = now.checked_duration_since(self.prev).unwrap_or_else(|| {
            warn!("received a timestamp early than a previous recorded time, ignoring");
            Default::default()
        });

        if smoothed_rtt.as_nanos() == 0 {
            return None;
        }

        let elapsed_rtts = time_elapsed.as_secs_f64() / smoothed_rtt.as_secs_f64();
        let new_tokens = window as f64 * 1.25 * elapsed_rtts;
        self.tokens = self
            .tokens
            .saturating_add(new_tokens as _)
            .min(self.capacity);

        self.prev = now;

        // if we can already send a packet, there is no need for delay
        if self.tokens >= bytes_to_send {
            return None;
        }

        let unscaled_delay = smoothed_rtt
            .checked_mul((bytes_to_send.max(self.capacity) - self.tokens) as _)
            .unwrap_or(Duration::MAX)
            / window;

        // divisions come before multiplications to prevent overflow
        // this is the time at which the pacing window becomes empty
        Some(self.prev + (unscaled_delay / 5) * 4)
    }

    /// A rate cap must not inherit the window/RTT gain, or bank idle time for
    /// a later catch-up burst. Refill on every poll, even when tokens remain.
    fn delay_capped(
        &mut self,
        bytes_to_send: u64,
        mtu: u16,
        now: Instant,
        rate_bps: u64,
    ) -> Option<Instant> {
        let capacity = ((u128::from(rate_bps) * CAPPED_BURST_INTERVAL_NANOS / 8_000_000_000)
            .min(u128::from(u64::MAX)) as u64)
            .clamp(2 * u64::from(mtu), MAX_BURST_SIZE * u64::from(mtu));
        if self.last_rate != Some(rate_bps) || self.last_mtu != mtu {
            // Apply reductions before spending saved tokens. Time before a
            // policy change is never credited at the new rate.
            self.capacity = capacity;
            self.tokens = self.tokens.min(capacity);
            self.last_rate = Some(rate_bps);
            self.last_mtu = mtu;
            self.fractional_tokens = 0;
            self.prev = now;
        } else if let Some(elapsed) = now.checked_duration_since(self.prev) {
            let credit = elapsed
                .as_nanos()
                .saturating_mul(u128::from(rate_bps))
                .saturating_add(self.fractional_tokens);
            let whole = (credit / 8_000_000_000).min(u128::from(u64::MAX)) as u64;
            self.tokens = self.tokens.saturating_add(whole).min(capacity);
            self.fractional_tokens = if self.tokens == capacity {
                0
            } else {
                credit % 8_000_000_000
            };
            self.prev = now;
        }

        if self.tokens >= bytes_to_send {
            return None;
        }
        // Wake when the next datagram fits. Waiting for the entire bucket
        // makes coarse timer wakeups discard refill credit and underdeliver
        // the selected rate. GSO may use the credit present at the wakeup.
        let missing = u128::from(bytes_to_send - self.tokens);
        let nanos = missing
            .saturating_mul(8_000_000_000)
            .saturating_sub(self.fractional_tokens)
            .div_ceil(u128::from(rate_bps))
            .min(u128::from(u64::MAX)) as u64;
        Some(self.prev + Duration::from_nanos(nanos))
    }
}

/// Calculates a pacer capacity for a certain window and RTT
///
/// The goal is to emit a burst (of size `capacity`) in timer intervals
/// which compromise between
/// - ideally distributing datagrams over time
/// - constantly waking up the connection to produce additional datagrams
///
/// Too short burst intervals means we will never meet them since the timer
/// accuracy in user-space is not high enough. If we miss the interval by more
/// than 25%, we will lose that part of the congestion window since no additional
/// tokens for the extra-elapsed time can be stored.
///
/// Too long burst intervals make pacing less effective.
fn optimal_capacity(smoothed_rtt: Duration, window: u64, mtu: u16) -> u64 {
    let rtt = smoothed_rtt.as_nanos().max(1);

    let capacity = ((window as u128 * BURST_INTERVAL_NANOS) / rtt) as u64;

    // Small bursts are less efficient (no GSO), could increase latency and don't effectively
    // use the channel's buffer capacity. Large bursts might block the connection on sending.
    capacity.clamp(MIN_BURST_SIZE * mtu as u64, MAX_BURST_SIZE * mtu as u64)
}

/// The burst interval
///
/// The capacity will we refilled in 4/5 of that time.
/// 2ms is chosen here since framework timers might have 1ms precision.
/// If kernel-level pacing is supported later a higher time here might be
/// more applicable.
const BURST_INTERVAL_NANOS: u128 = 2_000_000; // 2ms

// A fixed rate has no window/RTT catch-up gain. Keep enough bounded credit for
// ordinary multi-ms runtime wakeups, or a two-ms bucket underdelivers the video
// rate whenever the sender is scheduled less frequently. Uncapped pacing keeps
// the upstream interval above; neither policy accumulates unlimited idle credit.
const CAPPED_BURST_INTERVAL_NANOS: u128 = 8_000_000; // 8ms

/// Allows some usage of GSO, and doesn't slow down the handshake.
const MIN_BURST_SIZE: u64 = 10;

/// Creating 256 packets took 1ms in a benchmark, so larger bursts don't make sense.
const MAX_BURST_SIZE: u64 = 256;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capped_sender_obeys_rate_envelope_with_large_window_and_gso_batches() {
        let now = Instant::now();
        let mtu = 1344;
        let rate = 68_500_000;
        let mut pacer = Pacer::new(Duration::from_micros(1), u64::MAX, mtu, now);
        let mut sent = 0;
        let mut largest_batch = 0;
        for tick in 0..=1000 {
            let time = now + Duration::from_micros(tick * 100);
            let mut batch = 0;
            while pacer
                .delay_with_rate(Duration::ZERO, mtu.into(), mtu, u64::MAX, time, Some(rate))
                .is_none()
            {
                pacer.on_transmit(mtu);
                sent += u64::from(mtu);
                batch += u64::from(mtu);
            }
            largest_batch = largest_batch.max(batch);
            let allowance = rate * tick * 100 / 8_000_000 + pacer.capacity;
            assert!(sent <= allowance, "{sent} > {allowance} at tick {tick}");
        }
        assert_eq!(pacer.capacity, 68_500);
        assert!(largest_batch <= pacer.capacity);
        assert!(
            sent > rate / 8 / 10 * 99 / 100,
            "cap must still deliver the selected rate"
        );
    }

    #[test]
    fn capped_sender_delivers_the_selected_rate_with_coarse_timer_wakeups() {
        let now = Instant::now();
        let rate = 68_500_000;
        let mtu = 1344;
        let mut pacer = Pacer::new(Duration::from_micros(1), u64::MAX, mtu, now);
        let mut sent = 0u64;
        // Runtime scheduling is not a real-time clock. A four-ms wakeup must
        // not turn the transport ceiling into half the required video rate.
        for tick in 0..=250 {
            let time = now + Duration::from_millis(tick * 4);
            while pacer
                .delay_with_rate(Duration::ZERO, mtu.into(), mtu, u64::MAX, time, Some(rate))
                .is_none()
            {
                pacer.on_transmit(mtu);
                sent += u64::from(mtu);
            }
            let allowed = rate * tick * 4 / 8_000 + pacer.capacity;
            assert!(
                sent <= allowed,
                "rate envelope exceeded: {sent} > {allowed}"
            );
        }
        assert!(
            sent >= rate / 8 * 95 / 100,
            "coarse timer starved the sender: {sent} bytes"
        );
    }

    #[test]
    fn capped_sender_cannot_bank_idle_time_or_resume_with_a_frame_sized_burst() {
        let now = Instant::now();
        let mtu = 1344;
        let rate = 68_500_000;
        let mut pacer = Pacer::new(Duration::from_millis(200), 10_000_000, mtu, now);
        for time in [now, now + Duration::from_secs(60)] {
            let mut burst = 0;
            while pacer
                .delay_with_rate(
                    Duration::from_millis(200),
                    mtu.into(),
                    mtu,
                    10_000_000,
                    time,
                    Some(rate),
                )
                .is_none()
            {
                pacer.on_transmit(mtu);
                burst += u64::from(mtu);
            }
            assert!(burst <= 68_500);
            assert!(burst >= 68_500 - u64::from(mtu));
            // A second poll at the same instant cannot re-credit the idle time.
            assert!(
                pacer
                    .delay_with_rate(Duration::ZERO, mtu.into(), mtu, u64::MAX, time, Some(rate))
                    .is_some()
            );
        }
    }

    #[test]
    fn rate_reduction_clamps_saved_credit_immediately_before_next_ack() {
        let now = Instant::now();
        let mtu = 1344;
        let mut pacer = Pacer::new(Duration::from_millis(20), 10_000_000, mtu, now);
        assert!(
            pacer
                .delay_with_rate(
                    Duration::from_millis(20),
                    mtu.into(),
                    mtu,
                    10_000_000,
                    now,
                    Some(676_000_000)
                )
                .is_none()
        );
        let later = now + Duration::from_secs(1);
        let mut burst = 0;
        while pacer
            .delay_with_rate(
                Duration::from_millis(20),
                mtu.into(),
                mtu,
                10_000_000,
                later,
                Some(68_500_000),
            )
            .is_none()
        {
            pacer.on_transmit(mtu);
            burst += u64::from(mtu);
        }
        assert!(burst <= 68_500);
        assert_eq!(pacer.capacity, 68_500);
    }

    #[test]
    fn capped_sender_keeps_fractional_credit_between_frequent_polls() {
        let now = Instant::now();
        let mut pacer = Pacer::new(Duration::from_millis(20), 100_000, 1200, now);
        pacer.delay_with_rate(Duration::ZERO, 1200, 1200, 100_000, now, Some(1_000_000));
        pacer.tokens = 0;
        for tick in 1..=8000 {
            pacer.delay_with_rate(
                Duration::ZERO,
                1200,
                1200,
                100_000,
                now + Duration::from_nanos(tick),
                Some(1_000_000),
            );
        }
        assert_eq!(pacer.tokens, 1);
        assert_eq!(pacer.fractional_tokens, 0);
    }

    #[test]
    fn capped_sender_handles_clock_regression_mtu_and_rate_changes() {
        let now = Instant::now();
        let mut pacer = Pacer::new(Duration::from_millis(20), 100_000, 1200, now);
        pacer.delay_with_rate(Duration::ZERO, 1200, 1200, 100_000, now, Some(1_000_000));
        pacer.tokens = 0;
        assert!(
            pacer
                .delay_with_rate(
                    Duration::ZERO,
                    1200,
                    1200,
                    100_000,
                    now - Duration::from_secs(1),
                    Some(1_000_000)
                )
                .is_some()
        );
        assert_eq!(pacer.prev, now);
        pacer.delay_with_rate(Duration::ZERO, 9000, 9000, u64::MAX, now, Some(1_000_000));
        assert_eq!(pacer.capacity, 18_000);
        assert_eq!(pacer.tokens, 0);
        // The default controller still selects the upstream pacer.
        pacer.delay_with_rate(Duration::from_millis(20), 1200, 1200, 100_000, now, None);
        assert_eq!(pacer.last_rate, None);
        assert_eq!(
            pacer.capacity,
            optimal_capacity(Duration::from_millis(20), 100_000, 1200)
        );
    }

    #[test]
    fn does_not_panic_on_bad_instant() {
        let old_instant = Instant::now();
        let new_instant = old_instant + Duration::from_micros(15);
        let rtt = Duration::from_micros(400);

        assert!(
            Pacer::new(rtt, 30000, 1500, new_instant)
                .delay(Duration::from_micros(0), 0, 1500, 1, old_instant)
                .is_none()
        );
        assert!(
            Pacer::new(rtt, 30000, 1500, new_instant)
                .delay(Duration::from_micros(0), 1600, 1500, 1, old_instant)
                .is_none()
        );
        assert!(
            Pacer::new(rtt, 30000, 1500, new_instant)
                .delay(Duration::from_micros(0), 1500, 1500, 3000, old_instant)
                .is_none()
        );
    }

    #[test]
    fn derives_initial_capacity() {
        let window = 2_000_000;
        let mtu = 1500;
        let rtt = Duration::from_millis(50);
        let now = Instant::now();

        let pacer = Pacer::new(rtt, window, mtu, now);
        assert_eq!(
            pacer.capacity,
            (window as u128 * BURST_INTERVAL_NANOS / rtt.as_nanos()) as u64
        );
        assert_eq!(pacer.tokens, pacer.capacity);

        let pacer = Pacer::new(Duration::from_millis(0), window, mtu, now);
        assert_eq!(pacer.capacity, MAX_BURST_SIZE * mtu as u64);
        assert_eq!(pacer.tokens, pacer.capacity);

        let pacer = Pacer::new(rtt, 1, mtu, now);
        assert_eq!(pacer.capacity, MIN_BURST_SIZE * mtu as u64);
        assert_eq!(pacer.tokens, pacer.capacity);
    }

    #[test]
    fn adjusts_capacity() {
        let window = 2_000_000;
        let mtu = 1500;
        let rtt = Duration::from_millis(50);
        let now = Instant::now();

        let mut pacer = Pacer::new(rtt, window, mtu, now);
        assert_eq!(
            pacer.capacity,
            (window as u128 * BURST_INTERVAL_NANOS / rtt.as_nanos()) as u64
        );
        assert_eq!(pacer.tokens, pacer.capacity);
        let initial_tokens = pacer.tokens;

        pacer.delay(rtt, mtu as u64, mtu, window * 2, now);
        assert_eq!(
            pacer.capacity,
            (2 * window as u128 * BURST_INTERVAL_NANOS / rtt.as_nanos()) as u64
        );
        assert_eq!(pacer.tokens, initial_tokens);

        pacer.delay(rtt, mtu as u64, mtu, window / 2, now);
        assert_eq!(
            pacer.capacity,
            (window as u128 / 2 * BURST_INTERVAL_NANOS / rtt.as_nanos()) as u64
        );
        assert_eq!(pacer.tokens, initial_tokens / 2);

        pacer.delay(rtt, mtu as u64, mtu * 2, window, now);
        assert_eq!(
            pacer.capacity,
            (window as u128 * BURST_INTERVAL_NANOS / rtt.as_nanos()) as u64
        );

        pacer.delay(rtt, mtu as u64, 20_000, window, now);
        assert_eq!(pacer.capacity, 20_000_u64 * MIN_BURST_SIZE);
    }

    #[test]
    fn computes_pause_correctly() {
        let window = 2_000_000u64;
        let mtu = 1000;
        let rtt = Duration::from_millis(50);
        let old_instant = Instant::now();

        let mut pacer = Pacer::new(rtt, window, mtu, old_instant);
        let packet_capacity = pacer.capacity / mtu as u64;

        for _ in 0..packet_capacity {
            assert_eq!(
                pacer.delay(rtt, mtu as u64, mtu, window, old_instant),
                None,
                "When capacity is available packets should be sent immediately"
            );

            pacer.on_transmit(mtu);
        }

        let pace_duration = Duration::from_nanos((BURST_INTERVAL_NANOS * 4 / 5) as u64);

        assert_eq!(
            pacer
                .delay(rtt, mtu as u64, mtu, window, old_instant)
                .expect("Send must be delayed")
                .duration_since(old_instant),
            pace_duration
        );

        // Refill half of the tokens
        assert_eq!(
            pacer.delay(
                rtt,
                mtu as u64,
                mtu,
                window,
                old_instant + pace_duration / 2
            ),
            None
        );
        assert_eq!(pacer.tokens, pacer.capacity / 2);

        for _ in 0..packet_capacity / 2 {
            assert_eq!(
                pacer.delay(rtt, mtu as u64, mtu, window, old_instant),
                None,
                "When capacity is available packets should be sent immediately"
            );

            pacer.on_transmit(mtu);
        }

        // Refill all capacity by waiting more than the expected duration
        assert_eq!(
            pacer.delay(
                rtt,
                mtu as u64,
                mtu,
                window,
                old_instant + pace_duration * 3 / 2
            ),
            None
        );
        assert_eq!(pacer.tokens, pacer.capacity);
    }
}
