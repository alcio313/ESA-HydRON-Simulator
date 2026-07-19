use crate::models::{Satellite, OrbitType, Constellation, Segment};
use crate::config::Config;

pub fn create_satellites_from_config(config: &Config) -> Constellation {
    let mut leo_sats = Vec::new();
    let r_earth = config.env.r_earth;
    let r_leo = r_earth + config.leo_alt_km * 1000.0;
    let v_leo_mag = (config.env.mu / r_leo).sqrt();
    let inc_leo = config.leo_inc_deg.to_radians();

    for i in 0..config.leo_num {
        let u = (i as f64) * 2.0 * std::f64::consts::PI / (config.leo_num as f64);
        let r_plane = [r_leo * u.cos(), r_leo * u.sin(), 0.0];
        let v_plane = [-v_leo_mag * u.sin(), v_leo_mag * u.cos(), 0.0];
        
        let c_i = inc_leo.cos();
        let s_i = inc_leo.sin();
        let r_eci = [
            r_plane[0],
            r_plane[1] * c_i,
            r_plane[1] * s_i
        ];
        let v_eci = [
            v_plane[0],
            v_plane[1] * c_i,
            v_plane[1] * s_i
        ];

        leo_sats.push(Satellite {
            id: format!("LEO_{:02}", i),
            orbit_type: OrbitType::LEO,
            r: r_eci,
            v: v_eci,
            q: [1.0, 0.0, 0.0, 0.0],
            omega: [0.0, 0.0, 0.0],
            mass: config.leo_mass,
            area: config.leo_area,
            cd: config.leo_cd,
            cr: config.leo_cr,
            inertia: [0.4, 0.4, 0.5],
            h_rw: [0.0, 0.0, 0.0],
            is_custom: false,
            custom_color: None,
        });
    }

    let mut meo_sats = Vec::new();
    let r_meo = r_earth + config.meo_alt_km * 1000.0;
    let v_meo_mag = (config.env.mu / r_meo).sqrt();
    let inc_meo = config.meo_inc_deg.to_radians();

    for i in 0..config.meo_num {
        let raan = if !config.meo_raans.is_empty() { config.meo_raans[0] } else { 0.0 };
        let raan_rad = raan.to_radians();
        let u = (i as f64) * 2.0 * std::f64::consts::PI / (config.meo_num as f64);
        let r_plane = [r_meo * u.cos(), r_meo * u.sin(), 0.0];
        let v_plane = [-v_meo_mag * u.sin(), v_meo_mag * u.cos(), 0.0];

        let c_r = raan_rad.cos();
        let s_r = raan_rad.sin();
        let c_i = inc_meo.cos();
        let s_i = inc_meo.sin();

        let r_eci = [
            c_r * r_plane[0] - s_r * c_i * r_plane[1],
            s_r * r_plane[0] + c_r * c_i * r_plane[1],
            s_i * r_plane[1]
        ];
        let v_eci = [
            c_r * v_plane[0] - s_r * c_i * v_plane[1],
            s_r * v_plane[0] + c_r * c_i * v_plane[1],
            s_i * v_plane[1]
        ];

        meo_sats.push(Satellite {
            id: format!("MEO_{:02}", i),
            orbit_type: OrbitType::MEO,
            r: r_eci,
            v: v_eci,
            q: [1.0, 0.0, 0.0, 0.0],
            omega: [0.0, 0.0, 0.0],
            mass: config.meo_mass,
            area: config.meo_area,
            cd: config.meo_cd,
            cr: config.meo_cr,
            inertia: [1.5, 1.5, 2.0],
            h_rw: [0.0, 0.0, 0.0],
            is_custom: false,
            custom_color: None,
        });
    }

    let mut geo_sats = Vec::new();
    let r_geo = r_earth + config.geo_alt_km * 1000.0;
    let v_geo_mag = (config.env.mu / r_geo).sqrt();
    let inc_geo = config.geo_inc_deg.to_radians();

    for i in 0..config.geo_num {
        let lon_rad = (i as f64) * 2.0 * std::f64::consts::PI / (config.geo_num as f64);
        let r_plane = [r_geo * lon_rad.cos(), r_geo * lon_rad.sin(), 0.0];
        let v_plane = [-v_geo_mag * lon_rad.sin(), v_geo_mag * lon_rad.cos(), 0.0];

        let c_i = inc_geo.cos();
        let s_i = inc_geo.sin();
        let r_eci = [
            r_plane[0],
            r_plane[1] * c_i,
            r_plane[1] * s_i
        ];
        let v_eci = [
            v_plane[0],
            v_plane[1] * c_i,
            v_plane[1] * s_i
        ];

        geo_sats.push(Satellite {
            id: format!("GEO_{:02}", i),
            orbit_type: OrbitType::GEO,
            r: r_eci,
            v: v_eci,
            q: [1.0, 0.0, 0.0, 0.0],
            omega: [0.0, 0.0, 0.0],
            mass: config.geo_mass,
            area: config.geo_area,
            cd: config.geo_cd,
            cr: config.geo_cr,
            inertia: [15.0, 15.0, 20.0],
            h_rw: [0.0, 0.0, 0.0],
            is_custom: false,
            custom_color: None,
        });
    }

    let segments = vec![
        Segment { orbit_type: OrbitType::LEO, satellites: leo_sats },
        Segment { orbit_type: OrbitType::MEO, satellites: meo_sats },
        Segment { orbit_type: OrbitType::GEO, satellites: geo_sats },
    ];

    Constellation {
        name: config.name.clone(),
        segments,
    }
}

