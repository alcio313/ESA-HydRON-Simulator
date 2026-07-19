
#[derive(Debug, Clone)]
pub struct Lcg {
    state: u64,
}

impl Lcg {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_f64(&mut self) -> f64 {
        // Numerical Recipes LCG parameters
        self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.state as f64) / (u64::MAX as f64)
    }
}

pub fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

// Dot product helper
pub fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

// Norm helper
pub fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

// Normalize helper
pub fn normalize(a: [f64; 3]) -> [f64; 3] {
    let n = norm(a);
    if n > 0.0 {
        [a[0] / n, a[1] / n, a[2] / n]
    } else {
        [0.0, 0.0, 0.0]
    }
}

// Vector addition
pub fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

// Vector scaling
pub fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

// Rodrigues rotation of vector v about the unit axis k by angle theta (radians)
pub fn rotate_about_axis(v: [f64; 3], k: [f64; 3], theta: f64) -> [f64; 3] {
    let (s, c) = theta.sin_cos();
    let kxv = cross(k, v);
    let kdv = dot(k, v);
    [
        v[0] * c + kxv[0] * s + k[0] * kdv * (1.0 - c),
        v[1] * c + kxv[1] * s + k[1] * kdv * (1.0 - c),
        v[2] * c + kxv[2] * s + k[2] * kdv * (1.0 - c),
    ]
}

// Quaternion normalization
pub fn normalize_q(q: [f64; 4]) -> [f64; 4] {
    let n = (q[0]*q[0] + q[1]*q[1] + q[2]*q[2] + q[3]*q[3]).sqrt();
    if n > 0.0 {
        [q[0]/n, q[1]/n, q[2]/n, q[3]/n]
    } else {
        [1.0, 0.0, 0.0, 0.0]
    }
}

// ECI to ECEF rotation matrix at GST
pub fn eci_to_ecef_matrix(gst: f64) -> [[f64; 3]; 3] {
    let c = gst.cos();
    let s = gst.sin();
    [
        [c, s, 0.0],
        [-s, c, 0.0],
        [0.0, 0.0, 1.0],
    ]
}

// Matrix vector multiply
pub fn mat_vec_mult(m: [[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [
        m[0][0]*v[0] + m[0][1]*v[1] + m[0][2]*v[2],
        m[1][0]*v[0] + m[1][1]*v[1] + m[1][2]*v[2],
        m[2][0]*v[0] + m[2][1]*v[1] + m[2][2]*v[2],
    ]
}

// Rotate vector using quaternion (ECI to body frame)
// v_body = R(q) * v_eci
pub fn rotate_vector_q(q: [f64; 4], v: [f64; 3]) -> [f64; 3] {
    let q_vec = [q[1], q[2], q[3]];
    let q_scalar = q[0];
    
    // R(q) v = v + 2 * q_vec x (q_vec x v + q_scalar * v)
    let temp = add(cross(q_vec, v), scale(v, q_scalar));
    add(v, scale(cross(q_vec, temp), 2.0))
}

// Geodetic to ECEF conversion using WGS-84 ellipsoid parameters
pub fn lla_to_ecef(lat_rad: f64, lon_rad: f64, alt_m: f64) -> [f64; 3] {
    let a = 6378137.0; // Equatorial radius (m)
    let f = 1.0 / 298.257223563; // Flattening
    let e2 = f * (2.0 - f);
    
    let sin_lat = lat_rad.sin();
    let cos_lat = lat_rad.cos();
    let n = a / (1.0 - e2 * sin_lat * sin_lat).sqrt();
    
    let x = (n + alt_m) * cos_lat * lon_rad.cos();
    let y = (n + alt_m) * cos_lat * lon_rad.sin();
    let z = (n * (1.0 - e2) + alt_m) * sin_lat;
    
    [x, y, z]
}

/// Compute azimuth (°, N=0 clockwise), elevation (°, + = above horizon), and distance (km)
/// from observer at ECI `obs_r` (geodetic lat/lon provided for NED frame) to target at ECI `tgt_r`.
/// `obs_lat` and `obs_lon` are in radians.
pub fn az_el_dist(obs_r: [f64; 3], obs_lat: f64, obs_lon: f64, tgt_r: [f64; 3]) -> (f64, f64, f64) {
    // Range vector in ECI
    let dr = [tgt_r[0]-obs_r[0], tgt_r[1]-obs_r[1], tgt_r[2]-obs_r[2]];
    let dist_m = norm(dr);
    if dist_m < 1.0 { return (0.0, 0.0, 0.0); }
    let dr_u = normalize(dr);

    // NED unit vectors at observer (ECI, Earth assumed non-rotating for instantaneous geometry)
    // N: north = d(obs_r_unit)/d(lat) at obs position
    let (sin_lat, cos_lat) = (obs_lat.sin(), obs_lat.cos());
    let (sin_lon, cos_lon) = (obs_lon.sin(), obs_lon.cos());
    let north = [-sin_lat*cos_lon, -sin_lat*sin_lon, cos_lat];
    let east  = [-sin_lon,          cos_lon,           0.0  ];
    let up    = [ cos_lat*cos_lon,  cos_lat*sin_lon,  sin_lat];

    let d_n = dot(dr_u, north);
    let d_e = dot(dr_u, east);
    let d_u = dot(dr_u, up);

    let el_rad = d_u.asin();
    let az_rad = d_e.atan2(d_n);  // atan2(E, N) → 0=North, 90=East

    let az_deg = az_rad.to_degrees().rem_euclid(360.0);
    let el_deg = el_rad.to_degrees();
    let dist_km = dist_m / 1000.0;
    (az_deg, el_deg, dist_km)
}

// Simple hand-rolled TOML config loader to keep the application dependency-free
// ponytail: custom config loader that avoids external crate compilation and downloads.
