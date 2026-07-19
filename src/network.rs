use crate::math::normalize;
use crate::math::{norm, dot};
use crate::models::SimEnvironment;

/// One satellite as a node in the ground-reach routing graph.
pub struct RouteNode {
    /// MEO/GEO relays can forward traffic for other satellites; LEO terminals cannot.
    pub is_relay: bool,
    /// Maximum bitrate the satellite's payload can handle (Gbps).
    pub max_cap: f64,
    /// Reference distance used for the space-to-ground (SGL) free-space loss model.
    pub sgl_ref_dist: f64,
    /// Reference distance used for the inter-satellite (ISL) free-space loss model.
    pub isl_ref_dist: f64,
    /// ECI position (m).
    pub r: [f64; 3],
}

/// Compute, for every satellite, the widest-path (max-bottleneck) capacity to reach the
/// ground network — allowing traffic to be relayed through one or more MEO/GEO relay
/// satellites when a satellite is not directly connected to a ground station.
///
/// A relay that has no direct ground link can still reach the ground through another relay;
/// the end-to-end rate is limited at each hop by the relay's maximum capacity and by the
/// inter-satellite link capacity, which in turn caps the bitrate available to LEO terminals
/// transmitting to ground via the network.
///
/// Returns, per satellite, `cap_to_ground`: the largest achievable bottleneck bitrate to the
/// ground (0 if it can reach neither a ground station nor a relay that does).
pub fn compute_ground_reach(
    nodes: &[RouteNode],
    gs_eci: &[[f64; 3]],
    gs_k: &[f64],
    env: &SimEnvironment,
) -> Vec<f64> {
    let n = nodes.len();
    let mut direct_sgl = vec![0.0_f64; n];

    // Base case: the best direct space-to-ground link for each satellite.
    for (idx, node) in nodes.iter().enumerate() {
        let mut best_cap = 0.0_f64;
        for (i, gs) in gs_eci.iter().enumerate() {
            let cap = compute_link_capacity(
                node.r, *gs, true, gs_k[i], node.sgl_ref_dist, node.max_cap, env,
            )
            .min(node.max_cap);
            if cap > best_cap {
                best_cap = cap;
            }
        }
        direct_sgl[idx] = best_cap;
    }

    // Widest-path relaxation: a node can improve its reach by forwarding through a relay
    // neighbour that already reaches the ground. Each pass can extend the best path by one
    // more relay hop, so it converges in at most `n` passes; the extra cap is a safety bound.
    let mut cap_to_ground = direct_sgl.clone();
    for _ in 0..=n {
        let mut changed = false;
        for a in 0..n {
            for b in 0..n {
                if a == b || !nodes[b].is_relay || cap_to_ground[b] <= 0.0 {
                    continue;
                }
                if !visible(nodes[a].r, nodes[b].r, env.r_earth) {
                    continue;
                }
                let nominal = nodes[a].max_cap.min(nodes[b].max_cap);
                let isl = compute_link_capacity(
                    nodes[a].r, nodes[b].r, false, 0.0, nodes[a].isl_ref_dist, nominal, env,
                );
                let via = nodes[a].max_cap.min(isl).min(cap_to_ground[b]);
                if via > cap_to_ground[a] + 1e-9 {
                    cap_to_ground[a] = via;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    cap_to_ground
}

pub fn visible(r1: [f64; 3], r2: [f64; 3], r_earth: f64) -> bool {
    let d = [r2[0] - r1[0], r2[1] - r1[1], r2[2] - r1[2]];
    let d_len_sq = dot(d, d);
    if d_len_sq == 0.0 { return true; }
    let u_min = -dot(r1, d) / d_len_sq;
    // Ray occultation height limit: 100 km for ISL atmospheric blockage.
    let r_occult = r_earth + 100_000.0;
    if (0.0..=1.0).contains(&u_min) {
        let closest_point = [
            r1[0] + u_min * d[0],
            r1[1] + u_min * d[1],
            r1[2] + u_min * d[2]
        ];
        norm(closest_point) >= r_occult
    } else {
        norm(r1) >= r_occult && norm(r2) >= r_occult
    }
}

// visible_sgl: LoS between a satellite and a ground station on Earth's surface.
// A GS is always at norm ≈ r_earth, so we cannot require norm(GS) >= r_earth+100km.
// Instead: link is blocked only if the interior of the segment dips below r_earth.
pub fn visible_sgl(r_sat: [f64; 3], r_gs: [f64; 3], r_earth: f64) -> bool {
    let d = [r_sat[0] - r_gs[0], r_sat[1] - r_gs[1], r_sat[2] - r_gs[2]];
    let d_len_sq = dot(d, d);
    if d_len_sq == 0.0 { return false; }
    // u_min: parameter of closest approach along the GS→Sat segment
    let u_min = -dot(r_gs, d) / d_len_sq;
    if u_min <= 0.0 {
        // Closest point is the GS itself: segment goes upward → satellite is above horizon
        return true;
    }
    if u_min >= 1.0 {
        // Closest point is the satellite: segment never dips → visible
        return true;
    }
    // Interior closest point: check it doesn't go through the solid Earth
    let closest = [
        r_gs[0] + u_min * d[0],
        r_gs[1] + u_min * d[1],
        r_gs[2] + u_min * d[2],
    ];
    norm(closest) >= r_earth
}

// 5. compute_link_capacity: Calculates instantaneous laser link bandwidth.
pub fn compute_link_capacity(
    r_from: [f64; 3],
    r_to: [f64; 3],
    is_sgl: bool,
    gs_k: f64,
    ref_dist_km: f64,
    nominal_capacity: f64,
    env: &SimEnvironment,
) -> f64 {
    let d_vec = [r_to[0] - r_from[0], r_to[1] - r_from[1], r_to[2] - r_from[2]];
    let d_m = norm(d_vec);

    // Use correct visibility check: SGL endpoints are on Earth's surface
    let is_vis = if is_sgl {
        // r_from = satellite, r_to = GS (or vice versa — pick the one closer to Earth)
        let (r_sat, r_gs) = if norm(r_from) > norm(r_to) { (r_from, r_to) } else { (r_to, r_from) };
        visible_sgl(r_sat, r_gs, env.r_earth)
    } else {
        visible(r_from, r_to, env.r_earth)
    };
    if !is_vis {
        return 0.0;
    }

    // Transmittance T_atm = exp(-k * L)
    let t_atm = if is_sgl {
        // Position of ground station (assumed r_from or r_to; whichever is closer to Earth center)
        let r_gs = if norm(r_from) < norm(r_to) { r_from } else { r_to };
        let r_sat = if norm(r_from) < norm(r_to) { r_to } else { r_from };
        // Direction vector must point from ground station to satellite for slant path calculation
        let dir = normalize([r_sat[0] - r_gs[0], r_sat[1] - r_gs[1], r_sat[2] - r_gs[2]]);
        
        let r_gs_len = norm(r_gs);
        let r_atm = env.r_earth + 10_000.0; // Weather/troposphere boundary at 10 km for realistic attenuation
        
        // Quadratic equation for ray boundary intersection: u^2 + 2(r_gs . dir)u + (r_gs^2 - r_atm^2) = 0
        let b = 2.0 * dot(r_gs, dir);
        let c = r_gs_len * r_gs_len - r_atm * r_atm;
        let disc: f64 = b * b - 4.0 * c;
        
        let l_slant = if disc >= 0.0 {
            let u1 = (-b + disc.sqrt()) / 2.0;
            if u1 > 0.0 { u1.min(d_m) } else { 0.0 }
        } else {
            0.0
        };
        
        let att_db = gs_k * l_slant;
        10.0_f64.powf(-att_db / 10.0)
    } else {
        1.0 // Inter-Satellite Link has no atmospheric attenuation
    };

    // Free space divergence path loss logic: f(d) = (d0 / d)^2
    let d_km = d_m / 1000.0;
    let dist_ratio = ref_dist_km / d_km;
    
    nominal_capacity * t_atm * (dist_ratio * dist_ratio)
}

