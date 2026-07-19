use crate::models::{Satellite, GroundStation, AtmosphereModel, SimEnvironment};
use crate::math::*;

pub fn step_orbit(sat: &mut Satellite, dt: f64, env: &SimEnvironment, sun_vector: [f64; 3]) {
    let mut state = [
        sat.r[0], sat.r[1], sat.r[2],
        sat.v[0], sat.v[1], sat.v[2]
    ];

    let mass = sat.mass;
    let cd = sat.cd;
    let area = sat.area;
    let cr = sat.cr;

    let deriv = |x: &[f64; 6]| -> [f64; 6] {
        let r_vec = [x[0], x[1], x[2]];
        let v_vec = [x[3], x[4], x[5]];
        let r_len = norm(r_vec);
        let r_len3 = r_len.powi(3);
        let r_len5 = r_len.powi(5);

        // Core central two-body gravity
        let mut a = scale(r_vec, -env.mu / r_len3);

        // 1. J2 Perturbation
        if env.j2 > 0.0 {
            let j2_coeff = 1.5 * env.j2 * env.mu * env.r_earth.powi(2) / r_len5;
            let z2_r2_ratio = x[2] * x[2] / (r_len * r_len);
            let a_j2 = [
                j2_coeff * x[0] * (5.0 * z2_r2_ratio - 1.0),
                j2_coeff * x[1] * (5.0 * z2_r2_ratio - 1.0),
                j2_coeff * x[2] * (5.0 * z2_r2_ratio - 3.0),
            ];
            a = add(a, a_j2);
        }

        // 2. Atmospheric Drag (only for LEO / MEO below 1500km)
        let altitude = r_len - env.r_earth;
        if altitude < 1_500_000.0 && cd > 0.0 {
            // Exponential atmospheric model
            let h0_m = env.h0_km * 1000.0;
            let scale_height_m = env.scale_height_km * 1000.0;
            let rho = env.rho0_500km * (-(altitude - h0_m) / scale_height_m).exp();
            
            // Relative velocity vector (assuming Earth's atmosphere co-rotates with Earth)
            let omega_earth = [0.0, 0.0, 7.292115e-5];
            let v_rel = [
                v_vec[0] - (-omega_earth[2] * r_vec[1]),
                v_vec[1] - (omega_earth[2] * r_vec[0]),
                v_vec[2]
            ];
            let v_rel_len = norm(v_rel);
            let a_drag = scale(v_rel, -0.5 * rho * cd * area / mass * v_rel_len);
            a = add(a, a_drag);
        }

        // 3. Solar Radiation Pressure (SRP)
        if env.p_srp > 0.0 {
            // s_hat is the unit sun direction vector
            let s_hat = normalize(sun_vector);
            let a_srp = scale(s_hat, env.p_srp * cr * area / mass);
            a = add(a, a_srp);
        }

        [v_vec[0], v_vec[1], v_vec[2], a[0], a[1], a[2]]
    };

    // RK4 numerical integration
    let k1 = deriv(&state);
    let mut x2 = [0.0; 6];
    for i in 0..6 { x2[i] = state[i] + 0.5 * dt * k1[i]; }
    let k2 = deriv(&x2);
    let mut x3 = [0.0; 6];
    for i in 0..6 { x3[i] = state[i] + 0.5 * dt * k2[i]; }
    let k3 = deriv(&x3);
    let mut x4 = [0.0; 6];
    for i in 0..6 { x4[i] = state[i] + dt * k3[i]; }
    let k4 = deriv(&x4);

    for i in 0..6 {
        state[i] += (dt / 6.0) * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
    }

    sat.r = [state[0], state[1], state[2]];
    sat.v = [state[3], state[4], state[5]];
}

// Ideal fixed-geometry propagation: rigidly rotates the state (r, v) about the
// orbit normal at the current angular rate. Radius, speed and orbit plane never
// change, so the satellite's angular position advances uniformly and the
// constellation geometry is immune to external perturbations (J2, drag, SRP).
pub fn step_orbit_ideal(sat: &mut Satellite, dt: f64) {
    let h = cross(sat.r, sat.v);
    let h_len = norm(h);
    let r_len2 = dot(sat.r, sat.r);
    if h_len < 1e-9 || r_len2 < 1e-9 {
        return; // degenerate state: no defined orbit plane
    }
    let axis = scale(h, 1.0 / h_len);
    let theta = h_len / r_len2 * dt; // instantaneous angular rate ω = |r×v|/|r|²
    sat.r = rotate_about_axis(sat.r, axis, theta);
    sat.v = rotate_about_axis(sat.v, axis, theta);
}

// 2. step_attitude: Propagates the spacecraft attitude dynamics using quaternion kinematic integration
// and Euler's equations of rotational motion with reaction wheels, magnetorquers, and disturbances.
pub fn step_attitude(sat: &mut Satellite, dt: f64, b_eci: [f64; 3], torque_rw_cmd: [f64; 3], dipole_mtq_cmd: [f64; 3]) {
    // 1. Euler dynamics: I * domega/dt + omega x (I * omega) = tau_rw + tau_mtq + tau_dist
    let i_x = sat.inertia[0];
    let i_y = sat.inertia[1];
    let i_z = sat.inertia[2];
    
    // Magnetic field in body frame: B_body = R(q) * B_eci
    let b_body = rotate_vector_q(sat.q, b_eci);
    
    // Torque from magnetorquer: tau_mtq = m x B
    let tau_mtq = cross(dipole_mtq_cmd, b_body);
    
    // Torque from reaction wheels (action/reaction on spacecraft body)
    let tau_rw = torque_rw_cmd; 
    
    // Environmental disturbances (gravity gradient mockup as basic test dist)
    // ponytail: disturbance torque is simplified to constant bias + small white noise mockup
    let tau_dist = [1e-6, -1e-6, 5e-7];

    let total_torque = [
        tau_rw[0] + tau_mtq[0] + tau_dist[0],
        tau_rw[1] + tau_mtq[1] + tau_dist[1],
        tau_rw[2] + tau_mtq[2] + tau_dist[2],
    ];

    let omega_x_i_omega = [
        sat.omega[1] * (i_z * sat.omega[2]) - sat.omega[2] * (i_y * sat.omega[1]),
        sat.omega[2] * (i_x * sat.omega[0]) - sat.omega[0] * (i_z * sat.omega[2]),
        sat.omega[0] * (i_y * sat.omega[1]) - sat.omega[1] * (i_x * sat.omega[0]),
    ];

    let domega = [
        (total_torque[0] - omega_x_i_omega[0]) / i_x,
        (total_torque[1] - omega_x_i_omega[1]) / i_y,
        (total_torque[2] - omega_x_i_omega[2]) / i_z,
    ];

    // Update wheel angular momentum: h_rw_dot = -tau_rw
    for i in 0..3 {
        sat.h_rw[i] += -tau_rw[i] * dt;
    }

    // Update omega
    for i in 0..3 {
        sat.omega[i] += domega[i] * dt;
    }

    // 2. Quaternion kinematics integration: dq/dt = 0.5 * Omega(omega) * q
    let q = sat.q;
    let w = sat.omega;
    let dq = [
        -0.5 * (q[1]*w[0] + q[2]*w[1] + q[3]*w[2]),
         0.5 * (q[0]*w[0] + q[2]*w[2] - q[3]*w[1]),
         0.5 * (q[0]*w[1] - q[1]*w[2] + q[3]*w[0]),
         0.5 * (q[0]*w[2] + q[1]*w[1] - q[2]*w[0]),
    ];

    let new_q = [
        q[0] + dq[0] * dt,
        q[1] + dq[1] * dt,
        q[2] + dq[2] * dt,
        q[3] + dq[3] * dt,
    ];

    sat.q = normalize_q(new_q);
}

// 3. step_atmosphere: Updates atmospheric state for each ground station using a discrete Markov chain.
pub fn step_atmosphere(gs: &mut GroundStation, model: &mut AtmosphereModel) {
    let r = model.lcg.next_f64();
    let current_state = gs.atmos_state;
    let row = &model.transition_matrix[current_state];
    
    let mut cumulative = 0.0;
    let mut next_state = current_state;
    
    for (idx, &prob) in row.iter().enumerate() {
        cumulative += prob;
        if r < cumulative {
            next_state = idx;
            break;
        }
    }
    
    gs.atmos_state = next_state;
    gs.k_value = model.k_values[next_state] / 1000.0; // Convert 1/km to 1/m
}

// 4. visible: Evaluates geometric LoS between two space nodes (ISL). Uses r_earth+100km buffer.
