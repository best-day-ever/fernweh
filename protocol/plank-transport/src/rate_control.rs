// SPDX-License-Identifier: AGPL-3.0-or-later

//! PLANK's rate policy at the Kynet/Quinn ownership boundary.
//!
//! KyProto remains responsible for media packetization and RaptorQ. This
//! module turns the requested encoder rate into a FEC-inclusive wire budget
//! and supplies a Host Quinn controller whose window does not collapse on
//! isolated repairable loss. Quinn enforces the selected rate with a bounded
//! token bucket, independently of the window, with bounded encoder peak
//! headroom. Encoder rate, FEC and bounded media queues are unchanged.

use quinn_proto::RttEstimator;
use quinn_proto::congestion::{Controller, ControllerFactory, ControllerMetrics};
use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MIN_VIDEO_BITRATE_BPS: u64 = 10_000_000;
const FEC_AND_PACKET_SCALE_NUMERATOR: u64 = 7;
const FEC_AND_PACKET_SCALE_DENOMINATOR: u64 = 5;
const AUDIO_AND_CONTROL_RESERVE_BPS: u64 = 1_000_000;
const MIN_WINDOW_PACKETS: u64 = 64;
// A short LAN RTT must not reduce the flight allowance to approximately one
// video frame. Keep bounded headroom for delayed acknowledgments, independently
// of the bounded queue of datagrams waiting to be transmitted.
const MIN_WINDOW_BYTES: u64 = 2 * 1024 * 1024;
const STEADY_WINDOW_NUMERATOR: u64 = 3;
const STEADY_WINDOW_DENOMINATOR: u64 = 2;
const MAX_WINDOW_RTT: Duration = Duration::from_millis(100);
const INITIAL_WINDOW_RTT: Duration = Duration::from_millis(200);

fn bounded_peak_video_bps(target_bps: u64, peak_bps: u64) -> u64 {
    peak_bps.clamp(target_bps, target_bps.saturating_add(target_bps / 2))
}

fn video_to_wire_bps(video_bps: u64) -> u64 {
    video_bps
        .saturating_mul(FEC_AND_PACKET_SCALE_NUMERATOR)
        .div_ceil(FEC_AND_PACKET_SCALE_DENOMINATOR)
        .saturating_add(AUDIO_AND_CONTROL_RESERVE_BPS)
}

fn window_for_rate(rate_bps: u64, rtt: Duration, mtu: u16) -> u64 {
    let rtt_nanos = rtt.min(MAX_WINDOW_RTT).as_nanos().max(1);
    let bdp_bytes = (rate_bps as u128)
        .saturating_mul(rtt_nanos)
        .div_ceil(8_000_000_000_u128);
    let target = bdp_bytes
        .saturating_mul(STEADY_WINDOW_NUMERATOR as u128)
        .div_ceil(STEADY_WINDOW_DENOMINATOR as u128)
        .min(u64::MAX as u128) as u64;
    target
        .max(MIN_WINDOW_BYTES)
        .max(MIN_WINDOW_PACKETS.saturating_mul(mtu as u64))
}

/// Shared requested rate and Host controller budget for one media connection.
#[derive(Debug)]
pub struct TransportRatePolicy {
    requested_video_bps: AtomicU64,
    active_video_bps: AtomicU64,
    paced_peak_video_bps: AtomicU64,
    repairable_congestion_events: AtomicU64,
    persistent_congestion_events: AtomicU64,
}

impl TransportRatePolicy {
    pub fn new(requested_video_bps: u64) -> Arc<Self> {
        let requested_video_bps = requested_video_bps.max(MIN_VIDEO_BITRATE_BPS);
        Arc::new(Self {
            requested_video_bps: AtomicU64::new(requested_video_bps),
            active_video_bps: AtomicU64::new(requested_video_bps),
            paced_peak_video_bps: AtomicU64::new(bounded_peak_video_bps(
                requested_video_bps,
                u64::MAX,
            )),
            repairable_congestion_events: AtomicU64::new(0),
            persistent_congestion_events: AtomicU64::new(0),
        })
    }

    pub fn requested_video_bps(&self) -> u64 {
        self.requested_video_bps.load(Ordering::Acquire)
    }

    pub fn active_video_bps(&self) -> u64 {
        self.active_video_bps.load(Ordering::Acquire)
    }

    pub fn active_wire_bps(&self) -> u64 {
        video_to_wire_bps(self.paced_peak_video_bps.load(Ordering::Acquire))
    }

    /// Allow the encoder's peak rate to drain variable-size frames, but never
    /// grant more than 50% peak headroom over the selected target.
    pub fn set_requested_video_bps(&self, requested_video_bps: u64, peak_video_bps: u64) {
        let requested_video_bps = requested_video_bps.max(MIN_VIDEO_BITRATE_BPS);
        self.requested_video_bps
            .store(requested_video_bps, Ordering::Release);
        self.active_video_bps
            .store(requested_video_bps, Ordering::Release);
        self.paced_peak_video_bps.store(
            bounded_peak_video_bps(requested_video_bps, peak_video_bps),
            Ordering::Release,
        );
    }
}

#[derive(Clone)]
enum RatePolicy {
    Fixed(Arc<TransportRatePolicy>),
    Routed(RatePolicyBinding),
}

impl RatePolicy {
    fn current(&self) -> Arc<TransportRatePolicy> {
        match self {
            Self::Fixed(policy) => policy.clone(),
            Self::Routed(binding) => binding.0.lock().unwrap().clone(),
        }
    }
}

/// An incoming QUIC connection has no PLANK identity until its token arrives.
/// Bind its controller to the selected endpoint before acknowledging auth.
#[derive(Clone)]
pub(crate) struct RatePolicyBinding(Arc<Mutex<Arc<TransportRatePolicy>>>);

impl RatePolicyBinding {
    pub(crate) fn bind(&self, policy: Arc<TransportRatePolicy>) {
        *self.0.lock().unwrap() = policy;
    }
}

#[derive(Clone)]
struct PlankRateController {
    policy: RatePolicy,
    current_mtu: u16,
    last_rtt: Duration,
    bandwidth_estimate_bps: u64,
}

impl PlankRateController {
    #[cfg(test)]
    fn new(policy: Arc<TransportRatePolicy>, current_mtu: u16) -> Self {
        Self::with_policy(RatePolicy::Fixed(policy), current_mtu)
    }

    fn with_policy(policy: RatePolicy, current_mtu: u16) -> Self {
        Self {
            policy,
            current_mtu,
            last_rtt: INITIAL_WINDOW_RTT,
            bandwidth_estimate_bps: 0,
        }
    }

    fn update_rtt(&mut self, rtt: Duration) {
        self.last_rtt = rtt;
    }
}

impl Controller for PlankRateController {
    fn on_ack(
        &mut self,
        now: Instant,
        sent: Instant,
        bytes: u64,
        _app_limited: bool,
        rtt: &RttEstimator,
    ) {
        let delivery_time = now.saturating_duration_since(sent);
        if !delivery_time.is_zero() {
            self.bandwidth_estimate_bps = (bytes as u128)
                .saturating_mul(8_000_000_000)
                .checked_div(delivery_time.as_nanos())
                .unwrap_or_default()
                .min(u64::MAX as u128) as u64;
        }
        self.update_rtt(rtt.get());
    }

    fn on_congestion_event(
        &mut self,
        _now: Instant,
        _sent: Instant,
        is_persistent_congestion: bool,
        _lost_bytes: u64,
    ) {
        let policy = self.policy.current();
        let counter = if is_persistent_congestion {
            &policy.persistent_congestion_events
        } else {
            &policy.repairable_congestion_events
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }

    fn on_mtu_update(&mut self, new_mtu: u16) {
        self.current_mtu = new_mtu;
    }

    fn window(&self) -> u64 {
        // A live slider change or authenticated listener binding must take
        // effect even before the next ACK refreshes the stored window.
        window_for_rate(
            self.policy.current().active_wire_bps(),
            self.last_rtt,
            self.current_mtu,
        )
    }

    fn pacing_rate(&self) -> Option<u64> {
        Some(self.policy.current().active_wire_bps())
    }

    fn metrics(&self) -> ControllerMetrics {
        let mut metrics = ControllerMetrics::default();
        metrics.congestion_window = self.window();
        metrics.pacing_rate = self.pacing_rate();
        metrics.bandwidth_estimate = Some(self.bandwidth_estimate_bps);
        metrics
    }

    fn clone_box(&self) -> Box<dyn Controller> {
        Box::new(self.clone())
    }

    fn initial_window(&self) -> u64 {
        self.window()
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

/// Constructs one PLANK rate controller per Quinn path.
pub struct PlankRateControllerFactory {
    policy: RatePolicy,
}

impl PlankRateControllerFactory {
    pub fn new(policy: Arc<TransportRatePolicy>) -> Arc<Self> {
        eprintln!(
            "PLANK sender: bitrate-paced peak-limit=target*1.5 wire-budget=peak*1.4+1000000 burst-ms=8"
        );
        Arc::new(Self {
            policy: RatePolicy::Fixed(policy),
        })
    }

    pub(crate) fn new_routed() -> (Arc<Self>, RatePolicyBinding) {
        let binding =
            RatePolicyBinding(Arc::new(Mutex::new(TransportRatePolicy::new(150_000_000))));
        (
            Arc::new(Self {
                policy: RatePolicy::Routed(binding.clone()),
            }),
            binding,
        )
    }
}

impl ControllerFactory for PlankRateControllerFactory {
    fn build(self: Arc<Self>, _now: Instant, current_mtu: u16) -> Box<dyn Controller> {
        Box::new(PlankRateController::with_policy(
            self.policy.clone(),
            current_mtu,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_budget_includes_fixed_raptorq_and_packet_overhead() {
        assert_eq!(video_to_wire_bps(100_000_000), 141_000_000);
        assert_eq!(video_to_wire_bps(150_000_000), 211_000_000);
    }

    #[test]
    fn host_send_policy_does_not_change_encoder_target() {
        let policy = TransportRatePolicy::new(50_000_000);
        policy.set_requested_video_bps(50_000_000, 100_000_000);
        assert_eq!(policy.requested_video_bps(), 50_000_000);
        assert_eq!(policy.active_video_bps(), 50_000_000);
        assert_eq!(policy.active_wire_bps(), 106_000_000);
    }

    #[test]
    fn fifty_megabit_cbr_and_vbr_profiles_keep_bounded_headroom() {
        let policy = TransportRatePolicy::new(50_000_000);
        for (peak, wire) in [
            (1_000_000, 71_000_000),
            (50_000_000, 71_000_000),
            (60_000_000, 85_000_000),
            (75_000_000, 106_000_000),
            (1_000_000_000, 106_000_000),
        ] {
            policy.set_requested_video_bps(50_000_000, peak);
            assert_eq!(policy.active_video_bps(), 50_000_000);
            assert_eq!(policy.active_wire_bps(), wire);
        }
    }

    #[test]
    fn live_rate_reduction_updates_scheduler_and_window_without_an_ack() {
        let policy = TransportRatePolicy::new(500_000_000);
        let controller = PlankRateController::new(policy.clone(), 1_344);
        let previous_window = controller.window();
        policy.set_requested_video_bps(50_000_000, 1_000_000_000);
        assert_eq!(controller.pacing_rate(), Some(106_000_000));
        assert!(controller.window() < previous_window);
        assert_eq!(controller.metrics().pacing_rate, controller.pacing_rate());
    }

    #[test]
    fn selected_target_controls_pacing_at_low_and_high_rates() {
        let policy = TransportRatePolicy::new(150_000_000);
        for (requested, peak) in [
            (10_000_000, 15_000_000),
            (150_000_000, 225_000_000),
            (1_000_000_000, 1_500_000_000),
        ] {
            policy.set_requested_video_bps(requested, peak);
            assert_eq!(policy.active_video_bps(), requested);
            assert_eq!(policy.requested_video_bps(), requested);
            assert_eq!(
                policy.active_wire_bps(),
                video_to_wire_bps(bounded_peak_video_bps(requested, peak))
            );
        }
    }

    #[test]
    fn controller_window_uses_selected_budget_and_stays_rtt_bounded() {
        let policy = TransportRatePolicy::new(100_000_000);
        policy.set_requested_video_bps(100_000_000, 100_000_000);
        let mut controller = PlankRateController::new(policy, 1_344);
        controller.update_rtt(Duration::from_millis(20));
        assert_eq!(controller.window(), MIN_WINDOW_BYTES);
        controller.update_rtt(Duration::from_secs(10));
        assert_eq!(controller.window(), 2_643_750);
        controller.update_rtt(Duration::from_micros(1));
        assert_eq!(controller.window(), MIN_WINDOW_BYTES.max(MIN_WINDOW_PACKETS * 1_344));
    }

    #[test]
    fn steady_window_keeps_ack_headroom_below_the_rate_derived_target() {
        assert_eq!(
            window_for_rate(200_000_000, Duration::from_millis(20), 1_344),
            MIN_WINDOW_BYTES
        );
        assert_eq!(
            window_for_rate(200_000_000, Duration::from_micros(100), 1_344),
            MIN_WINDOW_BYTES
        );
        assert_eq!(
            window_for_rate(200_000_000, Duration::from_millis(100), 1_344),
            3_750_000
        );
        assert_eq!(
            window_for_rate(200_000_000, Duration::from_micros(100), u16::MAX),
            MIN_WINDOW_PACKETS * u16::MAX as u64
        );
    }

    #[test]
    fn excessive_encoder_peak_is_capped_at_fifty_percent_headroom() {
        let policy = TransportRatePolicy::new(100_000_000);
        policy.set_requested_video_bps(150_000_000, 225_000_000);
        assert_eq!(policy.requested_video_bps(), 150_000_000);
        assert_eq!(policy.active_video_bps(), 150_000_000);
        assert_eq!(policy.active_wire_bps(), 316_000_000);
        policy.set_requested_video_bps(150_000_000, 1_000_000_000);
        assert_eq!(policy.active_video_bps(), 150_000_000);
        assert_eq!(policy.active_wire_bps(), 316_000_000);
    }

    #[test]
    fn repairable_random_loss_does_not_collapse_the_window() {
        let policy = TransportRatePolicy::new(150_000_000);
        let factory = PlankRateControllerFactory::new(policy);
        let now = Instant::now();
        let mut controller = factory.build(now, 1_344);
        let initial_window = controller.window();
        for sequence in 0..500 {
            controller.on_congestion_event(
                now + Duration::from_millis(sequence),
                now,
                false,
                1_344,
            );
        }
        assert_eq!(controller.window(), initial_window);
    }

    #[test]
    fn incoming_connection_binds_only_its_authenticated_endpoint_rate_policy() {
        let (owner_factory, owner_binding) = PlankRateControllerFactory::new_routed();
        let (guest_factory, guest_binding) = PlankRateControllerFactory::new_routed();
        let mut owner = owner_factory.build(Instant::now(), 1_344);
        let mut guest = guest_factory.build(Instant::now(), 1_344);
        let owner_policy = TransportRatePolicy::new(150_000_000);
        let guest_policy = TransportRatePolicy::new(50_000_000);
        owner_binding.bind(owner_policy.clone());
        guest_binding.bind(guest_policy.clone());
        owner_policy.set_requested_video_bps(500_000_000, 1_000_000_000);
        assert_eq!(owner.metrics().pacing_rate, Some(1_051_000_000));
        assert_eq!(guest.metrics().pacing_rate, Some(106_000_000));
        owner.on_congestion_event(Instant::now(), Instant::now(), false, 1_344);
        guest.on_congestion_event(Instant::now(), Instant::now(), true, 1_344);
        assert_eq!(
            owner_policy
                .repairable_congestion_events
                .load(Ordering::Relaxed),
            1
        );
        assert_eq!(
            owner_policy
                .persistent_congestion_events
                .load(Ordering::Relaxed),
            0
        );
        assert_eq!(
            guest_policy
                .repairable_congestion_events
                .load(Ordering::Relaxed),
            0
        );
        assert_eq!(
            guest_policy
                .persistent_congestion_events
                .load(Ordering::Relaxed),
            1
        );
    }
}
