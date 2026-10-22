//! Link-level bandwidth and latency helpers.
//!
//! MVP fluid model does not use explicit buffers: fluid flow is smooth and
//! congestion is captured as a reduction in flow rate (see lib.rs doc).

/// Convert link bandwidth from Gbps (bits per second) to bytes per nanosecond.
/// 1 Gbps = 1e9 bits/s = 1.25e8 bytes/s = 0.125 bytes/ns.
pub fn gbps_to_bytes_per_ns(gbps: f64) -> f64 {
    gbps * 0.125
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_matches_known_values() {
        assert!((gbps_to_bytes_per_ns(100.0) - 12.5).abs() < 1e-9);
        assert!((gbps_to_bytes_per_ns(1.0) - 0.125).abs() < 1e-9);
        assert!((gbps_to_bytes_per_ns(400.0) - 50.0).abs() < 1e-9);
    }
}