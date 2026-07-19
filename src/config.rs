use std::io::{self, BufRead};
#[cfg(not(target_arch = "wasm32"))]
use std::io::BufReader;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::fs::File;
use crate::models::{GroundStation, SimEnvironment};

#[derive(Debug, Clone)]
pub struct Config {
    pub name: String,
    pub leo_num: usize,
    pub leo_alt_km: f64,
    pub leo_inc_deg: f64,
    pub leo_mass: f64,
    pub leo_area: f64,
    pub leo_cd: f64,
    pub leo_cr: f64,
    pub meo_num: usize,
    pub meo_alt_km: f64,
    pub meo_inc_deg: f64,
    pub meo_raans: Vec<f64>,
    pub meo_mass: f64,
    pub meo_area: f64,
    pub meo_cd: f64,
    pub meo_cr: f64,
    pub geo_num: usize,
    pub geo_lons: Vec<f64>,
    pub geo_alt_km: f64,
    pub geo_inc_deg: f64,
    pub geo_mass: f64,
    pub geo_area: f64,
    pub geo_cd: f64,
    pub geo_cr: f64,
    pub stations: Vec<GroundStation>,
    pub atmos_states: Vec<String>,
    pub atmos_k: Vec<f64>,
    pub transition_matrix: Vec<Vec<f64>>,
    pub env: SimEnvironment,
    pub dt_time_step: f64,
    pub ref_dist_isl_km: f64,
    pub ref_dist_sgl_km: f64,
}

// Cross-product helper
pub fn parse_config_from_reader<R: BufRead>(reader: R) -> io::Result<Config> {
    let mut name = String::from("HydRON");
    let mut leo_num = 10;
    let mut leo_alt_km = 550.0;
    let mut leo_inc_deg = 97.6;
    let mut leo_mass = 20.0;
    let mut leo_area = 0.1;
    let mut leo_cd = 2.2;
    let mut leo_cr = 1.2;
    
    let mut meo_num = 4;
    let mut meo_alt_km = 10000.0;
    let mut meo_inc_deg = 55.0;
    let mut meo_raans = vec![0.0, 90.0, 180.0, 270.0];
    let mut meo_mass = 50.0;
    let mut meo_area = 0.25;
    let mut meo_cd = 0.0;
    let mut meo_cr = 1.2;
    
    let mut geo_num = 3;
    let mut geo_lons = vec![0.0, 60.0, -120.0];
    let mut geo_alt_km = 35786.0;
    let mut geo_inc_deg = 0.0;
    let mut geo_mass = 200.0;
    let mut geo_area = 1.5;
    let mut geo_cd = 0.0;
    let mut geo_cr = 1.2;
    
    let mut stations = Vec::new();
    let mut atmos_states = vec!["clear".to_string(), "thin_clouds".to_string(), "thick_clouds".to_string(), "heavy".to_string()];
    let mut atmos_k = vec![0.05, 0.2, 1.5, 5.0];
    let transition_matrix = vec![
        vec![0.85, 0.10, 0.04, 0.01],
        vec![0.15, 0.70, 0.10, 0.05],
        vec![0.05, 0.15, 0.65, 0.15],
        vec![0.02, 0.08, 0.20, 0.70],
    ];
    
    let mut mu = 3.986004418e14;
    let mut r_earth = 6378137.0;
    let mut j2 = 1.08262668e-3;
    let mut rho0 = 3.8e-12;
    let mut h0 = 500.0;
    let mut scale_height = 70.0;
    let mut p_srp = 4.56e-6;
    
    let mut dt_time_step = 1.0;
    let mut ref_dist_isl_km = 1000.0;
    let mut ref_dist_sgl_km = 1000.0;

    let mut current_section = String::new();
    let mut station_id = String::new();
    let mut station_name = String::new();
    let mut station_lat: f64 = 0.0;
    let mut station_lon: f64 = 0.0;
    let mut station_alt: f64 = 0.0;
    let mut station_cap: f64 = 10.0;

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = trimmed[1..trimmed.len()-1].trim().to_string();
            if section == "ground.stations" || section == "[ground.stations]" || section == "[[ground.stations]]" {
                if !station_id.is_empty() {
                    stations.push(GroundStation {
                        id: station_id.clone(),
                        name: station_name.clone(),
                        lat_rad: station_lat.to_radians(),
                        lon_rad: station_lon.to_radians(),
                        alt_m: station_alt,
                        downlink_nominal_gbps: station_cap,
                        atmos_state: 0,
                        k_value: atmos_k[0] / 1000.0, // Convert 1/km to 1/m to keep consistent with meters
                    });
                }
                station_id = String::new();
                station_name = String::new();
                station_lat = 0.0;
                station_lon = 0.0;
                station_alt = 0.0;
                station_cap = 10.0;
                current_section = "ground.stations".to_string();
            } else {
                current_section = section;
            }
            continue;
        }

        if let Some(pos) = trimmed.find('=') {
            let key = trimmed[..pos].trim();
            let val = trimmed[pos+1..].trim();
            
            match current_section.as_str() {
                "constellation" => {
                    if key == "name" { name = val.replace('"', ""); }
                }
                "constellation.leo" => {
                    match key {
                        "num_satellites" => leo_num = val.parse().unwrap_or(leo_num),
                        "altitude_km" => leo_alt_km = val.parse().unwrap_or(leo_alt_km),
                        "inclination_deg" => leo_inc_deg = val.parse().unwrap_or(leo_inc_deg),
                        "mass_kg" => leo_mass = val.parse().unwrap_or(leo_mass),
                        "cross_section_area_m2" => leo_area = val.parse().unwrap_or(leo_area),
                        "cd" => leo_cd = val.parse().unwrap_or(leo_cd),
                        "cr" => leo_cr = val.parse().unwrap_or(leo_cr),
                        _ => {}
                    }
                }
                "constellation.meo" => {
                    match key {
                        "num_satellites" => meo_num = val.parse().unwrap_or(meo_num),
                        "altitude_km" => meo_alt_km = val.parse().unwrap_or(meo_alt_km),
                        "inclination_deg" => meo_inc_deg = val.parse().unwrap_or(meo_inc_deg),
                        "mass_kg" => meo_mass = val.parse().unwrap_or(meo_mass),
                        "cross_section_area_m2" => meo_area = val.parse().unwrap_or(meo_area),
                        "cd" => meo_cd = val.parse().unwrap_or(meo_cd),
                        "cr" => meo_cr = val.parse().unwrap_or(meo_cr),
                        "raans_deg" => {
                            let clean = val.replace('[', "").replace(']', "");
                            meo_raans = clean.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                        }
                        _ => {}
                    }
                }
                "constellation.geo" => {
                    match key {
                        "num_satellites" => geo_num = val.parse().unwrap_or(geo_num),
                        "altitude_km" => geo_alt_km = val.parse().unwrap_or(geo_alt_km),
                        "inclination_deg" => geo_inc_deg = val.parse().unwrap_or(geo_inc_deg),
                        "mass_kg" => geo_mass = val.parse().unwrap_or(geo_mass),
                        "cross_section_area_m2" => geo_area = val.parse().unwrap_or(geo_area),
                        "cd" => geo_cd = val.parse().unwrap_or(geo_cd),
                        "cr" => geo_cr = val.parse().unwrap_or(geo_cr),
                        "longitudes_deg" => {
                            let clean = val.replace('[', "").replace(']', "");
                            geo_lons = clean.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                        }
                        _ => {}
                    }
                }
                "ground.stations" => {
                    match key {
                        "id" => station_id = val.replace('"', "").replace(',', ""),
                        "name" => station_name = val.replace('"', "").replace(',', ""),
                        "lat_deg" => station_lat = val.parse().unwrap_or(0.0),
                        "lon_deg" => station_lon = val.parse().unwrap_or(0.0),
                        "alt_m" => station_alt = val.parse().unwrap_or(0.0),
                        "downlink_nominal_gbps" => {
                            let clean = val.replace('"', "").replace(',', "").trim().to_lowercase();
                            station_cap = if clean == "inf" || clean == "infinity" || clean == "unlimited" {
                                f64::INFINITY
                            } else {
                                clean.parse().unwrap_or(f64::INFINITY)
                            };
                        }
                        _ => {}
                    }
                }
                "atmosphere" => {
                    match key {
                        "states" => {
                            let clean = val.replace('[', "").replace(']', "");
                            atmos_states = clean.split(',').map(|s| s.trim().replace('"', "")).collect();
                        }
                        "k_values_per_km" => {
                            let clean = val.replace('[', "").replace(']', "");
                            atmos_k = clean.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                        }
                        // Transition matrix parsing is bypassed for standard lookups to remain robust.
                        _ => {}
                    }
                }
                "environment" => {
                    match key {
                        "mu" => mu = val.parse().unwrap_or(mu),
                        "r_earth" => r_earth = val.parse().unwrap_or(r_earth),
                        "j2" => j2 = val.parse().unwrap_or(j2),
                        "rho0_500km" => rho0 = val.parse().unwrap_or(rho0),
                        "h0_km" => h0 = val.parse().unwrap_or(h0),
                        "scale_height_km" => scale_height = val.parse().unwrap_or(scale_height),
                        "p_srp" => p_srp = val.parse().unwrap_or(p_srp),
                        _ => {}
                    }
                }
                "digital_twin" => {
                    match key {
                        "time_step_s" => dt_time_step = val.parse().unwrap_or(dt_time_step),
                        "ref_distance_isl_km" => ref_dist_isl_km = val.parse().unwrap_or(ref_dist_isl_km),
                        "ref_distance_sgl_km" => ref_dist_sgl_km = val.parse().unwrap_or(ref_dist_sgl_km),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
    
    // Add the final ground station
    if !station_id.is_empty() {
        stations.push(GroundStation {
            id: station_id.clone(),
            name: station_name.clone(),
            lat_rad: station_lat.to_radians(),
            lon_rad: station_lon.to_radians(),
            alt_m: station_alt,
            downlink_nominal_gbps: station_cap,
            atmos_state: 0,
            k_value: atmos_k[0] / 1000.0,
        });
    }

    Ok(Config {
        name,
        leo_num,
        leo_alt_km,
        leo_inc_deg,
        leo_mass,
        leo_area,
        leo_cd,
        leo_cr,
        meo_num,
        meo_alt_km,
        meo_inc_deg,
        meo_raans,
        meo_mass,
        meo_area,
        meo_cd,
        meo_cr,
        geo_num,
        geo_lons,
        geo_alt_km,
        geo_inc_deg,
        geo_mass,
        geo_area,
        geo_cd,
        geo_cr,
        stations,
        atmos_states,
        atmos_k,
        transition_matrix,
        env: SimEnvironment {
            mu,
            r_earth,
            j2,
            rho0_500km: rho0,
            h0_km: h0,
            scale_height_km: scale_height,
            p_srp,
        },
        dt_time_step,
        ref_dist_isl_km,
        ref_dist_sgl_km,
    })
}

// Simple hand-rolled TOML config loader to keep the application dependency-free
// ponytail: custom config loader that avoids external crate compilation and downloads.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_config<P: AsRef<Path>>(path: P) -> io::Result<Config> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    parse_config_from_reader(reader)
}

pub fn parse_config_from_str(content: &str) -> io::Result<Config> {
    let reader = std::io::Cursor::new(content.as_bytes());
    parse_config_from_reader(reader)
}

// 1. step_orbit: Propagates the satellite orbit using RK4 with two-body gravity + J2 + atmospheric drag + SRP.
