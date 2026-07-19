use crate::math::Lcg;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrbitType {
    LEO,
    MEO,
    GEO,
}

#[derive(Debug, Clone)]
pub struct Satellite {
    pub id: String,
    pub orbit_type: OrbitType,
    // Orbital state (ECI frame, SI units: m, m/s)
    pub r: [f64; 3],
    pub v: [f64; 3],
    // Attitude state (ECI to Body frame quaternion [q0, q1, q2, q3] where q0 is scalar)
    pub q: [f64; 4],
    pub omega: [f64; 3], // Angular velocity relative to ECI in body frame (rad/s)
    // Physical parameters
    pub mass: f64,
    pub area: f64,
    pub cd: f64,
    pub cr: f64,
    pub inertia: [f64; 3], // Ix, Iy, Iz (kg*m^2), diagonal terms
    // Actuator states
    pub h_rw: [f64; 3], // Reaction wheels angular momentum (Nms)
    pub is_custom: bool,
    pub custom_color: Option<[u8; 3]>, // RGB override for custom satellites
}

#[derive(Debug, Clone)]
pub struct GroundStation {
    pub id: String,
    pub name: String,
    pub lat_rad: f64,
    pub lon_rad: f64,
    pub alt_m: f64,
    pub downlink_nominal_gbps: f64,
    // Atmosphere dynamic state
    pub atmos_state: usize,
    pub k_value: f64, // Attenuation coefficient (1/m)
}

#[derive(Debug, Clone)]
pub struct AtmosphereModel {
    pub states: Vec<String>,
    pub k_values: Vec<f64>, // 1/m
    pub transition_matrix: Vec<Vec<f64>>,
    pub lcg: Lcg,
}

#[derive(Debug, Clone)]
pub struct Segment {
    pub orbit_type: OrbitType,
    pub satellites: Vec<Satellite>,
}

#[derive(Debug, Clone)]
pub struct Constellation {
    pub name: String,
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone)]
pub struct LaserLink {
    pub from_id: String,
    pub to_id: String,
    pub visible: bool,
    pub distance_km: f64,
    pub capacity_gbps: f64,
    pub latency_ms: f64,
}

// Global environmental parameters from config
#[derive(Debug, Clone)]
pub struct SimEnvironment {
    pub mu: f64,
    pub r_earth: f64,
    pub j2: f64,
    pub rho0_500km: f64,
    pub h0_km: f64,
    pub scale_height_km: f64,
    pub p_srp: f64,
}

