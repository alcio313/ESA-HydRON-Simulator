use std::fs::File;
use eframe::egui;
use crate::models::*;
use crate::config::*;
use crate::simulation::*;
use crate::physics::*;
use crate::network::*;
use crate::math::*;
#[cfg(target_arch = "wasm32")]
use crate::download_file;

// Colorblind-safe status palette (Okabe & Ito). The three levels
// (good / degraded / critical) stay distinguishable under deuteranopia and
// protanopia, and also differ in lightness so state is never encoded by hue
// alone. Used everywhere a color conveys link/weather/throughput status.
const STATUS_GOOD: egui::Color32 = egui::Color32::from_rgb(0, 158, 115); // bluish green
const STATUS_WARN: egui::Color32 = egui::Color32::from_rgb(240, 228, 66); // yellow
const STATUS_BAD: egui::Color32 = egui::Color32::from_rgb(213, 94, 0); // vermillion
const STATUS_IDLE: egui::Color32 = egui::Color32::from_rgb(156, 163, 175); // gray

// Brightened variants of the status palette for the SGL laser beams, whose
// glow rendering needs more luminous colors than the HUD labels.
const BEAM_GOOD: (u8, u8, u8) = (0, 220, 160);
const BEAM_WARN: (u8, u8, u8) = (250, 238, 100);
const BEAM_BAD: (u8, u8, u8) = (255, 130, 40);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RibbonTab {
    Simulation,
    Constellation,
    Network,
    Adcs,
    Weather,
}

pub struct HydronGuiApp {
    config: Config,
    constellation: Constellation,
    ground_stations: Vec<GroundStation>,
    atmos_model: AtmosphereModel,

    active_tab: RibbonTab,
    show_telemetry_hud: bool,
    show_logs_hud: bool,
    show_stations_hud: bool,
    show_leo_list_hud: bool,

    // Control parameters
    is_running: bool,
    current_time: f64,
    time_warp: i32,
    step_size: f64,
    // When true, satellites advance at a constant angular rate on a fixed
    // orbit plane, immune to external perturbations (J2, drag, SRP).
    ideal_orbits: bool,

    // Selection
    selected_satellite_id: String,
    dragging_satellite_id: Option<String>,

    // Form inputs for dynamic configuration edits
    leo_num_input: usize,
    leo_alt_input: f64,
    leo_inc_input: f64,
    meo_num_input: usize,
    meo_alt_input: f64,
    meo_inc_input: f64,
    geo_num_input: usize,
    geo_alt_input: f64,
    geo_inc_input: f64,

    // Satellite dynamic properties fields
    sat_mass_input: f64,
    sat_cd_input: f64,
    sat_cr_input: f64,

    // Noise parameters
    gyro_noise: f64,
    mag_noise: f64,
    sun_noise: f64,
    st_noise: f64,

    // OMTQ / RW command override
    force_disturbance: bool,
    disturbance_val: [f64; 3],

    // Atmosphere dynamic control
    weather_overrides: Vec<Option<usize>>, // None = Markov, Some(index) = Force state

    // Filter displays
    show_leo: bool,
    show_meo: bool,
    show_geo: bool,
    show_sgl: bool,
    prioritize_relay: bool,

    // Log list
    logs: Vec<String>,
    #[allow(dead_code)]
    config_path: String,

    // Throughput history for bottom panel plotting
    history_time: Vec<f32>,
    history_stations: Vec<Vec<f32>>,
    history_total: Vec<f32>,

    // 3D Map rotation and zoom state
    map_pitch: f32,
    map_yaw: f32,
    map_zoom: f32,

    // Add satellite form inputs
    add_sat_orbit_type: OrbitType,
    add_sat_alt_km: f64,
    add_sat_inc_deg: f64,
    add_sat_mass: f64,
    add_sat_area: f64,
    add_sat_cd: f64,
    add_sat_cr: f64,

    // Add custom constellation inputs
    add_const_name: String,
    add_const_orbit_type: OrbitType,
    add_const_num_sats: usize,
    add_const_alt_km: f64,
    add_const_inc_deg: f64,
    add_const_mass: f64,
    add_const_area: f64,
    add_const_cd: f64,
    add_const_cr: f64,
    add_sat_color: [f32; 3],
    add_const_color: [f32; 3],

    earth_texture: Option<egui::TextureHandle>,
    leo_max_bitrate: f64,
    meo_max_bitrate: f64,
    geo_max_bitrate: f64,
}

impl HydronGuiApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        // Setup visual theme matching high-end digital twins (dark slate palette)
        let mut visuals = egui::Visuals::dark();
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(10, 15, 30);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(20, 27, 45);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(30, 41, 59);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(51, 65, 85);
        visuals.window_fill = egui::Color32::from_rgb(15, 23, 42);
        cc.egui_ctx.set_visuals(visuals);

        // Load NotoEmoji font for full emoji support (e.g. 🛰)
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "NotoEmoji".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!("NotoEmoji-Regular.ttf"))),
        );
        // Append as fallback for both proportional and monospace families
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push("NotoEmoji".to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("NotoEmoji".to_owned());
        cc.egui_ctx.set_fonts(fonts);

        let constellation = create_satellites_from_config(&config);
        let ground_stations = config.stations.clone();
        
        let mut selected_id = "None".to_string();
        for seg in &constellation.segments {
            if !seg.satellites.is_empty() {
                selected_id = seg.satellites[0].id.clone();
                break;
            }
        }
        
        let mut app = Self {
            leo_num_input: config.leo_num,
            leo_alt_input: config.leo_alt_km,
            leo_inc_input: config.leo_inc_deg,
            meo_num_input: config.meo_num,
            meo_alt_input: config.meo_alt_km,
            meo_inc_input: config.meo_inc_deg,
            geo_num_input: config.geo_num,
            geo_alt_input: config.geo_alt_km,
            geo_inc_input: config.geo_inc_deg,
            
            sat_mass_input: config.leo_mass,
            sat_cd_input: config.leo_cd,
            sat_cr_input: config.leo_cr,
            
            gyro_noise: 1e-6,
            mag_noise: 1e-8,
            sun_noise: 1e-3,
            st_noise: 1e-4,
            
            force_disturbance: false,
            disturbance_val: [0.0, 0.0, 0.0],
            
            weather_overrides: vec![Some(0); ground_stations.len()],
            active_tab: RibbonTab::Simulation,
            ideal_orbits: false,
            show_telemetry_hud: true,
            show_logs_hud: true,
            show_stations_hud: true,
            show_leo_list_hud: true,
            
            show_leo: true,
            show_meo: true,
            show_geo: true,
            show_sgl: true,
            prioritize_relay: false,
            
            logs: vec!["System Digital Twin Initialized.".to_string()],
            config_path: "config.toml".to_string(),
            
            selected_satellite_id: selected_id,
            dragging_satellite_id: None,
            constellation,
            ground_stations: ground_stations.clone(),
            atmos_model: AtmosphereModel {
                states: config.atmos_states.clone(),
                k_values: config.atmos_k.clone(),
                transition_matrix: config.transition_matrix.clone(),
                lcg: Lcg::new(42),
            },
            config,
            is_running: true,
            current_time: 0.0,
            time_warp: 1,
            step_size: 1.0,
            
            history_time: Vec::new(),
            history_stations: vec![Vec::new(); ground_stations.len()],
            history_total: Vec::new(),
            map_pitch: 0.4,
            map_yaw: 0.6,
            map_zoom: 1.0,
            add_sat_orbit_type: OrbitType::LEO,
            add_sat_alt_km: 550.0,
            add_sat_inc_deg: 97.6,
            add_sat_mass: 20.0,
            add_sat_area: 0.1,
            add_sat_cd: 2.2,
            add_sat_cr: 1.2,
            add_const_name: "CustomConst".to_string(),
            add_const_orbit_type: OrbitType::LEO,
            add_const_num_sats: 6,
            add_const_alt_km: 600.0,
            add_const_inc_deg: 45.0,
            add_const_mass: 25.0,
            add_const_area: 0.15,
            add_const_cd: 2.2,
            add_const_cr: 1.2,
            add_sat_color: [0.18, 0.83, 0.75],   // default teal
            add_const_color: [0.91, 0.47, 0.98],  // default magenta
            earth_texture: None, // Will load below
            leo_max_bitrate: 100.0,
            meo_max_bitrate: 400.0,
            geo_max_bitrate: 800.0,
        };

        // Load Earth texture map (embedded at compile-time to work seamlessly on web & desktop)
        let img_bytes = include_bytes!("../earth.jpg");
        if let Ok(img) = image::load_from_memory_with_format(img_bytes, image::ImageFormat::Jpeg) {
            let rgba = img.to_rgba8();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [img.width() as usize, img.height() as usize],
                rgba.as_raw(),
            );
            app.earth_texture = Some(cc.egui_ctx.load_texture(
                "earth-texture",
                color_image,
                egui::TextureOptions::default(),
            ));
            app.log("Loaded Earth surface texture successfully.");
        } else {
            app.log("Warning: embedded earth.jpg could not be decoded.");
        }
        app.update_input_fields_for_selected();
        app
    }

    fn log(&mut self, msg: &str) {
        self.logs.push(format!("[{:.1}s] {}", self.current_time, msg));
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }

    fn update_input_fields_for_selected(&mut self) {
        let mut mass = 20.0;
        let mut cd = 2.2;
        let mut cr = 1.2;
        if let Some(sat) = self.find_satellite(&self.selected_satellite_id) {
            mass = sat.mass;
            cd = sat.cd;
            cr = sat.cr;
        }
        self.sat_mass_input = mass;
        self.sat_cd_input = cd;
        self.sat_cr_input = cr;
    }

    fn find_satellite(&self, id: &str) -> Option<&Satellite> {
        for seg in &self.constellation.segments {
            for sat in &seg.satellites {
                if sat.id == *id {
                    return Some(sat);
                }
            }
        }
        None
    }

    fn run_and_export_24h(&self) -> Result<String, std::io::Error> {
        use std::io::Write;

        let filename = "simulation_export.csv";
        let mut file = File::create(filename)?;

        // Write header
        let mut header = String::from("Time_s");
        for gs in &self.ground_stations {
            header.push_str(&format!(",{}", gs.id));
        }
        header.push_str(",Total_Throughput_Gbps,Active_ISL_Links,Active_SGL_Links\n");
        file.write_all(header.as_bytes())?;

        // Initialize temp states for 24h simulation run
        let mut constellation = create_satellites_from_config(&self.config);
        let mut ground_stations = self.config.stations.clone();
        let mut atmos_model = AtmosphereModel {
            states: self.config.atmos_states.clone(),
            k_values: self.config.atmos_k.clone(),
            transition_matrix: self.config.transition_matrix.clone(),
            lcg: Lcg::new(42),
        };

        let sim_duration = 86400.0;
        let step_size = 10.0; // 10s steps for excellent resolution
        let mut current_time = 0.0;
        
        let sun_vector = [1.0, 0.0, 0.0];
        let b_eci_mock = [1e-5, 2e-5, -3e-5];

        while current_time <= sim_duration {
            // 1. Step atmosphere
            for (idx, gs) in ground_stations.iter_mut().enumerate() {
                if let Some(forced_idx) = self.weather_overrides[idx] {
                    gs.atmos_state = forced_idx;
                    gs.k_value = atmos_model.k_values[forced_idx] / 1000.0;
                } else {
                    step_atmosphere(gs, &mut atmos_model);
                }
            }

            // 2. Step satellite dynamics
            for segment in &mut constellation.segments {
                for sat in &mut segment.satellites {
                    let rw_torque = [1e-3, -5e-4, 2e-4];
                    let mtq_dipole = [0.1, -0.05, 0.1];
                    if self.ideal_orbits {
                        step_orbit_ideal(sat, step_size);
                    } else {
                        step_orbit(sat, step_size, &self.config.env, sun_vector);
                    }
                    step_attitude(sat, step_size, b_eci_mock, rw_torque, mtq_dipole);
                }
            }

            // 3. Calculate positions and throughputs
            let gst = current_time * 7.292115e-5;
            let rot_mat = eci_to_ecef_matrix(gst);
            let rot_mat_t = [
                [rot_mat[0][0], rot_mat[1][0], rot_mat[2][0]],
                [rot_mat[0][1], rot_mat[1][1], rot_mat[2][1]],
                [rot_mat[0][2], rot_mat[1][2], rot_mat[2][2]],
            ];

            let all_sats: Vec<(String, OrbitType, [f64; 3])> = constellation.segments.iter()
                .flat_map(|seg| seg.satellites.iter().map(|s| (s.id.clone(), s.orbit_type.clone(), s.r)))
                .collect();

            let gs_eci_list: Vec<[f64; 3]> = ground_stations.iter().map(|gs| {
                let ecef = lla_to_ecef(gs.lat_rad, gs.lon_rad, gs.alt_m);
                mat_vec_mult(rot_mat_t, ecef)
            }).collect();

            let mut gs_throughputs = vec![0.0; ground_stations.len()];
            let mut total_throughput = 0.0;
            let mut active_sgl_links = 0;
            let mut active_isl_links = 0;

            // Track best SGL for LEO satellites
            let mut leo_best_gs = vec![usize::MAX; all_sats.len()];
            let mut leo_best_gs_cap = vec![0.0; all_sats.len()];

            // SGL links capacity
            for (sat_idx, (_sat_id, orbit_type, sat_r)) in all_sats.iter().enumerate() {
                let sat_max = match orbit_type {
                    OrbitType::LEO => self.leo_max_bitrate,
                    OrbitType::MEO => self.meo_max_bitrate,
                    OrbitType::GEO => self.geo_max_bitrate,
                };
                let sat_ref_dist = match orbit_type {
                    OrbitType::LEO => self.config.ref_dist_sgl_km,
                    OrbitType::MEO => self.config.meo_alt_km,
                    OrbitType::GEO => self.config.geo_alt_km,
                };

                let mut best_cap = 0.0_f64;
                let mut best_idx = usize::MAX;
                for (i, other_eci) in gs_eci_list.iter().enumerate() {
                    let cap = compute_link_capacity(
                        *sat_r, *other_eci, true,
                        ground_stations[i].k_value,
                        sat_ref_dist, sat_max, &self.config.env
                    ).min(sat_max);
                    if cap > best_cap {
                        best_cap = cap;
                        best_idx = i;
                    }
                }

                if best_idx < ground_stations.len() && best_cap > 0.0 {
                    if orbit_type == &OrbitType::LEO {
                        leo_best_gs[sat_idx] = best_idx;
                        leo_best_gs_cap[sat_idx] = best_cap;
                    } else {
                        // MEO and GEO SGL links are allocated immediately
                        gs_throughputs[best_idx] += best_cap;
                        total_throughput += best_cap;
                        active_sgl_links += 1;
                    }
                }
            }

            // ISL links
            // ponytail: greedy satellite link allocation. Limits LEO satellites to 1 connection.
            // O(N^2 log N) sort of candidates, which is fine for small constellations (<100 satellites).
            // Widest-path reach to ground, allowing multi-hop relaying through MEO/GEO relays.
            let cap_to_ground = self.ground_reach(&all_sats, &gs_eci_list, &ground_stations);

            let mut candidate_isls = Vec::new();
            for i in 0..all_sats.len() {
                for j in (i + 1)..all_sats.len() {
                    let (_id1, type1, r1) = &all_sats[i];
                    let (_id2, type2, r2) = &all_sats[j];

                    // An endpoint may carry traffic if it can reach the ground directly or by
                    // relaying through a chain of relays (encoded by cap_to_ground).
                    let id1_has_sgl = cap_to_ground[i] > 0.0;
                    let id2_has_sgl = cap_to_ground[j] > 0.0;
                    let is_allowed = id1_has_sgl || id2_has_sgl;

                    if is_allowed && visible(*r1, *r2, self.config.env.r_earth) {
                        let is_leo = type1 == &OrbitType::LEO || type2 == &OrbitType::LEO;
                        let capacity = if is_leo {
                            self.leo_max_bitrate
                        } else {
                            let sat_max1 = match type1 {
                                OrbitType::LEO => self.leo_max_bitrate,
                                OrbitType::MEO => self.meo_max_bitrate,
                                OrbitType::GEO => self.geo_max_bitrate,
                            };
                            let sat_max2 = match type2 {
                                OrbitType::LEO => self.leo_max_bitrate,
                                OrbitType::MEO => self.meo_max_bitrate,
                                OrbitType::GEO => self.geo_max_bitrate,
                            };
                            let nominal_capacity = sat_max1.min(sat_max2);
                            let sat_ref_dist = match type1 {
                                OrbitType::LEO => self.config.ref_dist_isl_km,
                                OrbitType::MEO => self.config.meo_alt_km,
                                OrbitType::GEO => self.config.geo_alt_km,
                            };
                            compute_link_capacity(*r1, *r2, false, 0.0, sat_ref_dist, nominal_capacity, &self.config.env)
                        };
                        let mut capacity = capacity;
                        // Bound by the downstream reach-to-ground so the relay chain bottleneck
                        // (and the relay's own max capacity) limits the LEO's usable bitrate.
                        let cap1 = cap_to_ground[i];
                        let cap2 = cap_to_ground[j];

                        if id1_has_sgl && id2_has_sgl {
                            capacity = capacity.min(cap1.max(cap2));
                        } else if id1_has_sgl {
                            capacity = capacity.min(cap1);
                        } else if id2_has_sgl {
                            capacity = capacity.min(cap2);
                        }
                        if capacity > 0.0 {
                            let class = match (type1, type2) {
                                (OrbitType::LEO, OrbitType::LEO) => 2,
                                (OrbitType::LEO, _) | (_, OrbitType::LEO) => 1,
                                _ => 0,
                            };
                            candidate_isls.push((class, capacity, i, j));
                        }
                    }
                }
            }

            // Add LEO SGL candidates — only if prioritize_relay (Relay Only) is inactive.
            if !self.prioritize_relay {
                for i in 0..all_sats.len() {
                    let (_, type_i, _) = &all_sats[i];
                    if type_i == &OrbitType::LEO && leo_best_gs_cap[i] > 0.0 {
                        candidate_isls.push((0, leo_best_gs_cap[i], i, usize::MAX));
                    }
                }
            }

            candidate_isls.sort_by(|a, b| {
                a.0.cmp(&b.0) // class ascending
                    .then_with(|| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)) // capacity descending
                    .then_with(|| a.2.cmp(&b.2))
                    .then_with(|| a.3.cmp(&b.3))
            });

            let mut leo_isl_count = std::collections::HashMap::new();
            for (_class, capacity, i, j) in candidate_isls {
                let (id1, type1, _) = &all_sats[i];

                if j == usize::MAX {
                    if *leo_isl_count.entry(id1.clone()).or_insert(0) >= 1 {
                        continue;
                    }
                    *leo_isl_count.entry(id1.clone()).or_insert(0) += 1;

                    let gs_idx = leo_best_gs[i];
                    gs_throughputs[gs_idx] += capacity;
                    total_throughput += capacity;
                    active_sgl_links += 1;
                } else {
                    let (id2, type2, _) = &all_sats[j];

                    if type1 == &OrbitType::LEO && *leo_isl_count.entry(id1.clone()).or_insert(0) >= 1 {
                        continue;
                    }
                    if type2 == &OrbitType::LEO && *leo_isl_count.entry(id2.clone()).or_insert(0) >= 1 {
                        continue;
                    }

                    if type1 == &OrbitType::LEO {
                        *leo_isl_count.entry(id1.clone()).or_insert(0) += 1;
                    }
                    if type2 == &OrbitType::LEO {
                        *leo_isl_count.entry(id2.clone()).or_insert(0) += 1;
                    }

                    active_isl_links += 1;
                }
            }

            // Write CSV row
            let mut row_str = format!("{:.1}", current_time);
            for val in &gs_throughputs {
                row_str.push_str(&format!(",{}", val));
            }
            row_str.push_str(&format!(",{},{},{}\n", total_throughput, active_isl_links, active_sgl_links));
            file.write_all(row_str.as_bytes())?;

            current_time += step_size;
        }

        Ok(filename.to_string())
    }

    /// Widest-path ground-reach routing for the current satellite/ground geometry.
    /// Relays (MEO/GEO) forward traffic for satellites that have no direct ground link, so a
    /// LEO's reachable bitrate is bounded by the bottleneck of the relay chain to the ground.
    fn ground_reach(
        &self,
        all_sats: &[(String, OrbitType, [f64; 3])],
        gs_eci: &[[f64; 3]],
        stations: &[GroundStation],
    ) -> Vec<f64> {
        let nodes: Vec<RouteNode> = all_sats
            .iter()
            .map(|(_, orbit_type, r)| {
                let (max_cap, sgl_ref_dist, isl_ref_dist, is_relay) = match orbit_type {
                    OrbitType::LEO => (self.leo_max_bitrate, self.config.ref_dist_sgl_km, self.config.ref_dist_isl_km, false),
                    OrbitType::MEO => (self.meo_max_bitrate, self.config.meo_alt_km, self.config.meo_alt_km, true),
                    OrbitType::GEO => (self.geo_max_bitrate, self.config.geo_alt_km, self.config.geo_alt_km, true),
                };
                RouteNode { is_relay, max_cap, sgl_ref_dist, isl_ref_dist, r: *r }
            })
            .collect();
        let gs_k: Vec<f64> = stations.iter().map(|g| g.k_value).collect();
        compute_ground_reach(&nodes, gs_eci, &gs_k, &self.config.env)
    }

    fn drag_satellite_to(&mut self, sat_id: &str, mouse_pos: egui::Pos2, center: egui::Pos2, scale_factor: f64) {
        let mut target_sat_pos = None;
        let mut target_sat_vel = None;
        let mut segment_idx = usize::MAX;
        
        for (seg_i, seg) in self.constellation.segments.iter().enumerate() {
            for sat in &seg.satellites {
                if sat.id == *sat_id {
                    target_sat_pos = Some(sat.r);
                    target_sat_vel = Some(sat.v);
                    segment_idx = seg_i;
                    break;
                }
            }
            if segment_idx != usize::MAX {
                break;
            }
        }

        if let (Some(r), Some(v), true) = (target_sat_pos, target_sat_vel, segment_idx < self.constellation.segments.len()) {
            let r_len = norm(r);
            let v_len = norm(v);
            if r_len > 0.0 && v_len > 0.0 {
                let u_r = scale(r, 1.0 / r_len);
                let u_v = scale(v, 1.0 / v_len);

                let cos_yaw = (self.map_yaw as f64).cos();
                let sin_yaw = (self.map_yaw as f64).sin();
                let cos_pitch = (self.map_pitch as f64).cos();
                let sin_pitch = (self.map_pitch as f64).sin();

                let project_pos = |pos: [f64; 3]| -> egui::Pos2 {
                    let x = pos[0];
                    let y = -pos[1];
                    let z = pos[2];
                    let x1 = x * cos_yaw - z * sin_yaw;
                    let z1 = x * sin_yaw + z * cos_yaw;
                    let y2 = y * cos_pitch - z1 * sin_pitch;
                    egui::pos2(
                        center.x + (x1 * scale_factor) as f32,
                        center.y + (y2 * scale_factor) as f32,
                    )
                };

                let mut best_theta = 0.0;
                let mut min_dist = f32::MAX;

                let steps = 120;
                for step in 0..steps {
                    let theta = (step as f64 * 2.0 * std::f64::consts::PI) / (steps as f64);
                    let r_sample = add(scale(u_r, r_len * theta.cos()), scale(u_v, r_len * theta.sin()));
                    let screen_pos = project_pos(r_sample);
                    let dist = screen_pos.distance(mouse_pos);
                    if dist < min_dist {
                        min_dist = dist;
                        best_theta = theta;
                    }
                }

                let cos_t = best_theta.cos();
                let sin_t = best_theta.sin();

                // Move only the dragged satellite (not the whole segment)
                'outer: for seg in &mut self.constellation.segments {
                    for sat in &mut seg.satellites {
                        if sat.id != sat_id { continue; }
                        let r_curr = sat.r;
                        let v_curr = sat.v;
                        let r_c_len = norm(r_curr);
                        let v_c_len = norm(v_curr);
                        if r_c_len > 0.0 && v_c_len > 0.0 {
                            let u_rc = scale(r_curr, 1.0 / r_c_len);
                            let u_vc = scale(v_curr, 1.0 / v_c_len);
                            sat.r = add(scale(u_rc, r_c_len * cos_t), scale(u_vc, r_c_len * sin_t));
                            sat.v = add(scale(u_vc, v_c_len * cos_t), scale(u_rc, -v_c_len * sin_t));
                        }
                        break 'outer;
                    }
                }
            }
        }
    }

    fn import_config_content(&mut self, content: &str, source_name: &str) -> Result<(), String> {
        match parse_config_from_str(content) {
            Ok(new_config) => {
                self.config = new_config;
                // Reinitialize simulation state matching load
                self.current_time = 0.0;
                self.selected_satellite_id = "None".to_string();
                self.dragging_satellite_id = None;
                self.constellation = create_satellites_from_config(&self.config);
                self.ground_stations = self.config.stations.clone();
                // Find a selected satellite ID
                for seg in &self.constellation.segments {
                    if !seg.satellites.is_empty() {
                        self.selected_satellite_id = seg.satellites[0].id.clone();
                        break;
                    }
                }
                self.update_input_fields_for_selected();
                self.weather_overrides = vec![Some(0); self.ground_stations.len()];
                self.history_stations = vec![vec![0.0f32; self.history_time.len()]; self.ground_stations.len()];
                self.log(&format!("Configurazione importata correttamente da {}", source_name));
                Ok(())
            }
            Err(e) => {
                let err_msg = format!("Errore caricamento configurazione: {}", e);
                self.log(&err_msg);
                Err(err_msg)
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn import_config(&mut self, path: &str) -> Result<(), String> {
        match std::fs::read_to_string(path) {
            Ok(content) => self.import_config_content(&content, path),
            Err(e) => {
                let err_msg = format!("Errore lettura file: {}", e);
                self.log(&err_msg);
                Err(err_msg)
            }
        }
    }

    fn generate_toml_string(&self) -> String {
        let c = &self.config;
        let mut toml = String::new();
        
        toml.push_str("# ESA HydRON Digital Twin Config file\n\n");
        toml.push_str("[constellation]\n");
        toml.push_str(&format!("name = \"{}\"\n\n", c.name));
        
        toml.push_str("[constellation.leo]\n");
        toml.push_str(&format!("num_satellites = {}\n", c.leo_num));
        toml.push_str(&format!("altitude_km = {:.1}\n", c.leo_alt_km));
        toml.push_str(&format!("inclination_deg = {:.4}\n", c.leo_inc_deg));
        toml.push_str(&format!("mass_kg = {:.1}\n", c.leo_mass));
        toml.push_str(&format!("cross_section_area_m2 = {:.4}\n", c.leo_area));
        toml.push_str(&format!("cd = {:.2}\n", c.leo_cd));
        toml.push_str(&format!("cr = {:.2}\n\n", c.leo_cr));

        toml.push_str("[constellation.meo]\n");
        toml.push_str(&format!("num_satellites = {}\n", c.meo_num));
        toml.push_str(&format!("altitude_km = {:.1}\n", c.meo_alt_km));
        toml.push_str(&format!("inclination_deg = {:.4}\n", c.meo_inc_deg));
        let raans_str = c.meo_raans.iter().map(|v| format!("{:.1}", v)).collect::<Vec<_>>().join(", ");
        toml.push_str(&format!("raans_deg = [{}]\n", raans_str));
        toml.push_str(&format!("mass_kg = {:.1}\n", c.meo_mass));
        toml.push_str(&format!("cross_section_area_m2 = {:.4}\n", c.meo_area));
        toml.push_str(&format!("cd = {:.2}\n", c.meo_cd));
        toml.push_str(&format!("cr = {:.2}\n\n", c.meo_cr));

        toml.push_str("[constellation.geo]\n");
        toml.push_str(&format!("num_satellites = {}\n", c.geo_num));
        let geo_lons_str = c.geo_lons.iter().map(|v| format!("{:.1}", v)).collect::<Vec<_>>().join(", ");
        toml.push_str(&format!("longitudes_deg = [{}]\n", geo_lons_str));
        toml.push_str(&format!("altitude_km = {:.1}\n", c.geo_alt_km));
        toml.push_str(&format!("inclination_deg = {:.4}\n", c.geo_inc_deg));
        toml.push_str(&format!("mass_kg = {:.1}\n", c.geo_mass));
        toml.push_str(&format!("cross_section_area_m2 = {:.4}\n", c.geo_area));
        toml.push_str(&format!("cd = {:.2}\n", c.geo_cd));
        toml.push_str(&format!("cr = {:.2}\n\n", c.geo_cr));

        toml.push_str("[ground]\n\n");
        for gs in &self.ground_stations {
            toml.push_str("[[ground.stations]]\n");
            toml.push_str(&format!("id = \"{}\"\n", gs.id));
            toml.push_str(&format!("name = \"{}\"\n", gs.name));
            toml.push_str(&format!("lat_deg = {:.4}\n", gs.lat_rad.to_degrees()));
            toml.push_str(&format!("lon_deg = {:.4}\n", gs.lon_rad.to_degrees()));
            toml.push_str(&format!("alt_m = {:.1}\n", gs.alt_m));
            let cap_val = if gs.downlink_nominal_gbps.is_infinite() {
                "\"unlimited\"".to_string()
            } else {
                format!("{:.1}", gs.downlink_nominal_gbps)
            };
            toml.push_str(&format!("downlink_nominal_gbps = {}\n\n", cap_val));
        }

        toml.push_str("[atmosphere]\n");
        let states_str = c.atmos_states.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ");
        toml.push_str(&format!("states = [{}]\n", states_str));
        let k_str = c.atmos_k.iter().map(|v| format!("{:.2}", v)).collect::<Vec<_>>().join(", ");
        toml.push_str(&format!("k_values_per_km = [{}]\n", k_str));
        toml.push_str("transition_matrix = [\n");
        for row in &c.transition_matrix {
            let row_str = row.iter().map(|v| format!("{:.2}", v)).collect::<Vec<_>>().join(", ");
            toml.push_str(&format!("    [{}],\n", row_str));
        }
        toml.push_str("]\n\n");

        toml.push_str("[adcs]\n");
        toml.push_str("rw_max_torque_nm = 0.01\n");
        toml.push_str("rw_max_momentum_nms = 0.1\n");
        toml.push_str("mtq_max_dipole_am2 = 0.2\n\n");

        toml.push_str("[sensors]\n");
        toml.push_str("gyro_bias_rad_s = [1e-5, 1e-5, 1e-5]\n");
        toml.push_str("gyro_noise_rad_s = 1e-6\n");
        toml.push_str("mag_noise_tesla = 1e-8\n");
        toml.push_str("sun_noise_rad = 1e-3\n");
        toml.push_str("star_tracker_noise_rad = 1e-4\n\n");

        toml.push_str("[environment]\n");
        toml.push_str(&format!("mu = {:.10e}\n", c.env.mu));
        toml.push_str(&format!("r_earth = {:.1}\n", c.env.r_earth));
        toml.push_str(&format!("j2 = {:.10e}\n", c.env.j2));
        toml.push_str(&format!("rho0_500km = {:.10e}\n", c.env.rho0_500km));
        toml.push_str(&format!("h0_km = {:.1}\n", c.env.h0_km));
        toml.push_str(&format!("scale_height_km = {:.1}\n", c.env.scale_height_km));
        toml.push_str(&format!("p_srp = {:.10e}\n\n", c.env.p_srp));

        toml.push_str("[digital_twin]\n");
        toml.push_str(&format!("time_step_s = {:.1}\n", c.dt_time_step));
        toml.push_str(&format!("sim_duration_s = 86400.0\n"));
        toml.push_str(&format!("ref_distance_isl_km = {:.1}\n", c.ref_dist_isl_km));
        toml.push_str(&format!("ref_distance_sgl_km = {:.1}\n", c.ref_dist_sgl_km));
        
        toml
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn export_config(&mut self, path: &str) -> Result<(), String> {
        let toml = self.generate_toml_string();
        match std::fs::write(path, toml) {
            Ok(_) => {
                self.log(&format!("Configurazione esportata in {}", path));
                Ok(())
            }
            Err(e) => {
                let err_msg = format!("Errore esportazione configurazione: {}", e);
                self.log(&err_msg);
                Err(err_msg)
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn export_config(&mut self, path: &str) -> Result<(), String> {
        let toml = self.generate_toml_string();
        download_file(path, &toml);
        self.log(&format!("Configurazione scaricata correttamente come {}", path));
        Ok(())
    }

    fn draw_throughput_chart(&self, ui: &mut egui::Ui, rect: egui::Rect) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(10, 15, 30));
        painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(30, 41, 59)), egui::StrokeKind::Inside);

        if self.history_time.len() < 2 {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, "In attesa di dati di simulazione...", egui::FontId::proportional(12.0), egui::Color32::GRAY);
            return;
        }

        let mut max_y = 100.0_f32;
        for val in &self.history_total {
            if *val > max_y {
                max_y = *val;
            }
        }
        max_y *= 1.1;

        let mut min_x = self.history_time[0];
        let mut max_x = self.history_time[0];
        for &t in &self.history_time {
            if t < min_x { min_x = t; }
            if t > max_x { max_x = t; }
        }
        let dx = max_x - min_x;

        let margin_left = 65.0f32;
        let margin_right = 15.0f32;
        let margin_top = 22.0f32;
        let margin_bottom = 15.0f32;

        let plot_width = rect.width() - margin_left - margin_right;
        let plot_height = rect.height() - margin_top - margin_bottom;

        let to_screen = |x: f32, y: f32| -> egui::Pos2 {
            let x_frac = if dx > 0.0 { (x - min_x) / dx } else { 0.0 };
            let y_frac = y / max_y;
            egui::pos2(
                rect.min.x + margin_left + x_frac * plot_width,
                rect.max.y - margin_bottom - y_frac * plot_height,
            )
        };

        let grid_lines = 3;
        for k in 0..=grid_lines {
            let y_val = (k as f32 / grid_lines as f32) * max_y;
            let pos_left = to_screen(min_x, y_val);
            let pos_right = to_screen(max_x, y_val);
            painter.line_segment([pos_left, pos_right], egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(100, 100, 100, 30)));
            painter.text(
                egui::pos2(rect.min.x + margin_left - 5.0, pos_left.y),
                egui::Align2::RIGHT_CENTER,
                format!("{:.0} Gbps", y_val),
                egui::FontId::proportional(9.0),
                egui::Color32::GRAY
            );
        }

        let grid_lines_x = 5;
        for k in 0..=grid_lines_x {
            let x_val = min_x + (k as f32 / grid_lines_x as f32) * dx;
            let pos_bottom = to_screen(x_val, 0.0);
            let pos_top = to_screen(x_val, max_y);
            painter.line_segment([pos_bottom, pos_top], egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(100, 100, 100, 30)));
            painter.text(
                egui::pos2(pos_bottom.x, rect.max.y - margin_bottom + 8.0),
                egui::Align2::CENTER_CENTER,
                format!("{:.0}s", x_val),
                egui::FontId::proportional(9.0),
                egui::Color32::GRAY
            );
        }

        // Okabe-Ito categorical set: mutually distinguishable under the common
        // color-vision deficiencies (the legend names remain the fallback cue).
        let colors = [
            egui::Color32::from_rgb(86, 180, 233), // sky blue
            egui::Color32::from_rgb(230, 159, 0),  // orange
            egui::Color32::from_rgb(0, 158, 115),  // bluish green
            egui::Color32::from_rgb(204, 121, 167), // reddish purple
        ];

        for i in 0..self.ground_stations.len() {
            let color = colors[i % colors.len()];
            let mut points = Vec::new();
            for k in 0..self.history_time.len() {
                points.push(to_screen(self.history_time[k], self.history_stations[i][k]));
            }
            for w in points.windows(2) {
                painter.line_segment([w[0], w[1]], egui::Stroke::new(1.2, color));
            }
        }

        let mut total_points = Vec::new();
        for k in 0..self.history_time.len() {
            total_points.push(to_screen(self.history_time[k], self.history_total[k]));
        }
        for w in total_points.windows(2) {
            painter.line_segment([w[0], w[1]], egui::Stroke::new(2.2, egui::Color32::WHITE));
        }

        let mut legend_x = rect.min.x + margin_left + 15.0;
        let legend_y = rect.min.y + 12.0;
        
        painter.circle_filled(egui::pos2(legend_x, legend_y), 3.0, egui::Color32::WHITE);
        painter.text(egui::pos2(legend_x + 8.0, legend_y), egui::Align2::LEFT_CENTER, "Totale Aggregato", egui::FontId::proportional(9.0), egui::Color32::WHITE);
        legend_x += 105.0;

        for i in 0..self.ground_stations.len() {
            let name = &self.ground_stations[i].name;
            let color = colors[i % colors.len()];
            painter.circle_filled(egui::pos2(legend_x, legend_y), 3.0, color);
            painter.text(egui::pos2(legend_x + 8.0, legend_y), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(9.0), egui::Color32::LIGHT_GRAY);
            legend_x += 70.0;
        }
    }

}

impl eframe::App for HydronGuiApp {
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // eframe 0.35 hands us the root Ui; keep a Context handle for the
        // floating windows and input/memory access below.
        let ctx = &root_ui.ctx().clone();
        // Run continuous animation/repaint loop
        ctx.request_repaint();

        // Check for dropped files (drag & drop config import)
        ctx.input(|i| {
            if let Some(file) = i.raw.dropped_files.first() {
                if let Some(bytes) = &file.bytes {
                    if let Ok(content) = std::str::from_utf8(bytes) {
                        let _ = self.import_config_content(content, &file.name);
                    }
                } else {
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Some(path) = &file.path {
                        let _ = self.import_config(&path.to_string_lossy());
                    }
                }
            }
        });

        let mut pending_remove = None;
        let mut pending_add = false;
        let mut pending_reset = false;

        // 1. Core simulation physics steps
        if self.is_running {
            let mut pending_logs = Vec::new();
            let loops = self.time_warp.abs();
            let dt = if self.time_warp < 0 { -self.step_size } else { self.step_size };

            for _ in 0..loops {
                if self.current_time + dt < 0.0 {
                    self.current_time = 0.0;
                    break;
                }
                self.current_time += dt;
                let sun_vector = [1.0, 0.0, 0.0];
                let b_eci_mock = [1e-5, 2e-5, -3e-5];

                // Step atmosphere
                for (idx, gs) in &mut self.ground_stations.iter_mut().enumerate() {
                    if let Some(forced_idx) = self.weather_overrides[idx] {
                        if gs.atmos_state != forced_idx {
                            gs.atmos_state = forced_idx;
                            gs.k_value = self.atmos_model.k_values[forced_idx] / 1000.0;
                            let state_name = &self.atmos_model.states[forced_idx];
                            pending_logs.push(format!("Weather at {} forced to {}", gs.name, state_name));
                        }
                    } else {
                        let prev_state = gs.atmos_state;
                        step_atmosphere(gs, &mut self.atmos_model);
                        if gs.atmos_state != prev_state {
                            let state_name = &self.atmos_model.states[gs.atmos_state];
                            pending_logs.push(format!("Weather at {} transitioned to {}", gs.name, state_name));
                        }
                    }
                }

                // Step satellite dynamics
                for segment in &mut self.constellation.segments {
                    for sat in &mut segment.satellites {
                        let rw_torque = [1e-3, -5e-4, 2e-4];
                        let mut mtq_dipole = [0.1, -0.05, 0.1];

                        if sat.id == self.selected_satellite_id && self.force_disturbance {
                            mtq_dipole = add(mtq_dipole, self.disturbance_val);
                            self.force_disturbance = false;
                            pending_logs.push(format!("Injected attitude disturbance into satellite {}", sat.id));
                        }

                        if self.ideal_orbits {
                            step_orbit_ideal(sat, dt);
                        } else {
                            step_orbit(sat, dt, &self.config.env, sun_vector);
                        }
                        step_attitude(sat, dt, b_eci_mock, rw_torque, mtq_dipole);
                    }
                }
            }
            for msg in pending_logs {
                self.log(&msg);
            }
        }

        // Pre-calculate positions and throughputs for all ground stations
        let gst = self.current_time * 7.292115e-5;
        let rot_mat = eci_to_ecef_matrix(gst);
        let rot_mat_t = [
            [rot_mat[0][0], rot_mat[1][0], rot_mat[2][0]],
            [rot_mat[0][1], rot_mat[1][1], rot_mat[2][1]],
            [rot_mat[0][2], rot_mat[1][2], rot_mat[2][2]],
        ];

        // Gather all active satellite ECI positions
        let all_sats: Vec<(String, OrbitType, [f64; 3])> = self.constellation.segments.iter()
            .flat_map(|seg| seg.satellites.iter().map(|s| (s.id.clone(), s.orbit_type.clone(), s.r)))
            .collect();

        // Gather all GS ECI positions
        let gs_eci_list: Vec<[f64; 3]> = self.ground_stations.iter().map(|gs| {
            let ecef = lla_to_ecef(gs.lat_rad, gs.lon_rad, gs.alt_m);
            mat_vec_mult(rot_mat_t, ecef)
        }).collect();

        // Pre-calculate connected satellites for each GS and throughputs
        let mut connected_sats_per_gs = vec![Vec::new(); self.ground_stations.len()];
        let mut gs_throughputs = vec![0.0f32; self.ground_stations.len()];
        let mut total_throughput = 0.0f32;

        // Track best SGL for LEO satellites
        let mut leo_best_gs = vec![usize::MAX; all_sats.len()];
        let mut leo_best_gs_cap = vec![0.0; all_sats.len()];

        for (sat_idx, (sat_id, orbit_type, sat_r)) in all_sats.iter().enumerate() {
            let sat_max = match orbit_type {
                OrbitType::LEO => self.leo_max_bitrate,
                OrbitType::MEO => self.meo_max_bitrate,
                OrbitType::GEO => self.geo_max_bitrate,
            };
            let sat_ref_dist = match orbit_type {
                OrbitType::LEO => self.config.ref_dist_sgl_km,
                OrbitType::MEO => self.config.meo_alt_km,
                OrbitType::GEO => self.config.geo_alt_km,
            };
            let orbit_label = match orbit_type {
                OrbitType::LEO => "LEO",
                OrbitType::MEO => "MEO",
                OrbitType::GEO => "GEO",
            };

            let mut best_cap = 0.0_f64;
            let mut best_idx = usize::MAX;
            for (i, other_eci) in gs_eci_list.iter().enumerate() {
                let cap = compute_link_capacity(
                    *sat_r, *other_eci, true,
                    self.ground_stations[i].k_value,
                    sat_ref_dist, sat_max, &self.config.env
                ).min(sat_max);
                if cap > best_cap {
                    best_cap = cap;
                    best_idx = i;
                }
            }

            if best_idx < self.ground_stations.len() && best_cap > 0.0 {
                if orbit_type == &OrbitType::LEO {
                    leo_best_gs[sat_idx] = best_idx;
                    leo_best_gs_cap[sat_idx] = best_cap;
                } else {
                    connected_sats_per_gs[best_idx].push((sat_id.clone(), orbit_label, best_cap, sat_max));
                    gs_throughputs[best_idx] += best_cap as f32;
                    total_throughput += best_cap as f32;
                }
            }
        }

        let mut sat_sgl_link = std::collections::HashMap::new();
        for (gs_idx, gs_conn) in connected_sats_per_gs.iter().enumerate() {
            let gs_name = &self.ground_stations[gs_idx].name;
            for (sat_id, _, cap, _) in gs_conn {
                sat_sgl_link.insert(sat_id.clone(), (gs_name.clone(), *cap));
            }
        }

        // Widest-path reach to ground, allowing multi-hop relaying through MEO/GEO relays:
        // a relay with no direct ground link can still reach the ground through another relay.
        // cap_to_ground[k] is the resulting bottleneck bitrate satellite k can push to ground.
        let cap_to_ground = self.ground_reach(&all_sats, &gs_eci_list, &self.ground_stations);

        // Pre-calculate active ISL links
        let mut candidate_isls = Vec::new();
        for i in 0..all_sats.len() {
            for j in (i + 1)..all_sats.len() {
                let (_id1, type1, r1) = &all_sats[i];
                let (_id2, type2, r2) = &all_sats[j];

                // An endpoint may carry traffic if it can reach the ground — directly or by
                // relaying through a chain of relays (that is what cap_to_ground encodes).
                let id1_has_sgl = cap_to_ground[i] > 0.0;
                let id2_has_sgl = cap_to_ground[j] > 0.0;
                let is_allowed = id1_has_sgl || id2_has_sgl;

                let show_link = match (type1, type2) {
                    (OrbitType::LEO, OrbitType::LEO) => self.show_leo,
                    (OrbitType::MEO, OrbitType::MEO) => self.show_meo,
                    (OrbitType::GEO, OrbitType::GEO) => self.show_geo,
                    _ => self.show_meo || self.show_geo || self.show_leo,
                } && is_allowed;

                if show_link && visible(*r1, *r2, self.config.env.r_earth) {
                    let is_leo = type1 == &OrbitType::LEO || type2 == &OrbitType::LEO;
                    let capacity = if is_leo {
                        self.leo_max_bitrate
                    } else {
                        let sat_max1 = match type1 {
                            OrbitType::LEO => self.leo_max_bitrate,
                            OrbitType::MEO => self.meo_max_bitrate,
                            OrbitType::GEO => self.geo_max_bitrate,
                        };
                        let sat_max2 = match type2 {
                            OrbitType::LEO => self.leo_max_bitrate,
                            OrbitType::MEO => self.meo_max_bitrate,
                            OrbitType::GEO => self.geo_max_bitrate,
                        };
                        let nominal_capacity = sat_max1.min(sat_max2);
                        let sat_ref_dist = match type1 {
                            OrbitType::LEO => self.config.ref_dist_isl_km,
                            OrbitType::MEO => self.config.meo_alt_km,
                            OrbitType::GEO => self.config.geo_alt_km,
                        };
                        compute_link_capacity(*r1, *r2, false, 0.0, sat_ref_dist, nominal_capacity, &self.config.env)
                    };
                    let mut capacity = capacity;
                    // Bound the link by the downstream reach-to-ground of whichever endpoint
                    // carries the traffic onward, so the relay chain's bottleneck (and the
                    // relay's own max capacity) limits the bitrate available to LEO terminals.
                    let cap1 = cap_to_ground[i];
                    let cap2 = cap_to_ground[j];

                    if id1_has_sgl && id2_has_sgl {
                        capacity = capacity.min(cap1.max(cap2));
                    } else if id1_has_sgl {
                        capacity = capacity.min(cap1);
                    } else if id2_has_sgl {
                        capacity = capacity.min(cap2);
                    }
                    if capacity > 0.0 {
                        let class = match (type1, type2) {
                            (OrbitType::LEO, OrbitType::LEO) => 2,
                            (OrbitType::LEO, _) | (_, OrbitType::LEO) => 1,
                            _ => 0,
                        };
                        candidate_isls.push((class, capacity, i, j));
                    }
                }
            }
        }

        // Add LEO SGL candidates — only if prioritize_relay (Relay Only) is inactive.
        if !self.prioritize_relay {
            for i in 0..all_sats.len() {
                let (_, type_i, _) = &all_sats[i];
                if type_i == &OrbitType::LEO && leo_best_gs_cap[i] > 0.0 {
                    candidate_isls.push((0, leo_best_gs_cap[i], i, usize::MAX));
                }
            }
        }

        candidate_isls.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| a.2.cmp(&b.2))
                .then_with(|| a.3.cmp(&b.3))
        });

        let mut leo_isl_count = std::collections::HashMap::new();
        let mut active_isls = Vec::new();
        let mut sat_isl_link = std::collections::HashMap::new();

        for (_class, capacity, i, j) in candidate_isls {
            let (id1, type1, _) = &all_sats[i];

            if j == usize::MAX {
                if *leo_isl_count.entry(id1.clone()).or_insert(0) >= 1 {
                    continue;
                }
                *leo_isl_count.entry(id1.clone()).or_insert(0) += 1;

                let gs_idx = leo_best_gs[i];
                let gs_name = &self.ground_stations[gs_idx].name;
                connected_sats_per_gs[gs_idx].push((id1.clone(), "LEO", capacity, self.leo_max_bitrate));
                gs_throughputs[gs_idx] += capacity as f32;
                total_throughput += capacity as f32;
                sat_sgl_link.insert(id1.clone(), (gs_name.clone(), capacity));
            } else {
                let (id2, type2, _) = &all_sats[j];

                if type1 == &OrbitType::LEO && *leo_isl_count.entry(id1.clone()).or_insert(0) >= 1 {
                    continue;
                }
                if type2 == &OrbitType::LEO && *leo_isl_count.entry(id2.clone()).or_insert(0) >= 1 {
                    continue;
                }

                if type1 == &OrbitType::LEO {
                    *leo_isl_count.entry(id1.clone()).or_insert(0) += 1;
                }
                if type2 == &OrbitType::LEO {
                    *leo_isl_count.entry(id2.clone()).or_insert(0) += 1;
                }

                active_isls.push((i, j, capacity));
                sat_isl_link.insert(id1.clone(), (id2.clone(), capacity));
                sat_isl_link.insert(id2.clone(), (id1.clone(), capacity));
            }
        }

        // Update history if running
        if self.is_running {
            self.history_time.push(self.current_time as f32);
            for i in 0..self.ground_stations.len() {
                self.history_stations[i].push(gs_throughputs[i]);
            }
            self.history_total.push(total_throughput);

            // Limit history size to 300 points (e.g. 5 minutes at 1Hz)
            let max_history = 300;
            if self.history_time.len() > max_history {
                self.history_time.remove(0);
                for i in 0..self.ground_stations.len() {
                    self.history_stations[i].remove(0);
                }
                self.history_total.remove(0);
            }
        }

        // 2. GUI panels layout
        // 2. GUI panels layout - Tabbed Ribbon Interface
        egui::Panel::top("top_panel").show(root_ui, |ui| {
            ui.add_space(4.0);
            // Tab selector: single fixed-height row that scrolls horizontally when the
            // window is too narrow to show all tabs, instead of wrapping onto a second row.
            egui::ScrollArea::horizontal()
                .id_salt("tab_bar_scroll")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.active_tab, RibbonTab::Simulation, "💻 Simulation");
                        ui.selectable_value(&mut self.active_tab, RibbonTab::Constellation, "🛰 Constellation");
                        ui.selectable_value(&mut self.active_tab, RibbonTab::Network, "📶 Network");
                        ui.selectable_value(&mut self.active_tab, RibbonTab::Adcs, "⚙ ADCS");
                        ui.selectable_value(&mut self.active_tab, RibbonTab::Weather, "☁ Weather");
                    });
                });

            ui.separator();

            // Ribbon Contents Grouped in Horizontal Blocks (wrapped into multiple rows if screen is narrow to prevent squeezing)
            macro_rules! render_ribbon_contents {
                ($ui:ident) => {
                    let ui = &mut *$ui;
                    match self.active_tab {
                    RibbonTab::Simulation => {
                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("CONTROL").strong().color(egui::Color32::LIGHT_BLUE));
                                ui.horizontal(|ui| {
                                    if ui.button(if self.is_running { "⏸ Pause" } else { "▶ Play" }).clicked() {
                                        self.is_running = !self.is_running;
                                        self.log(if self.is_running { "Simulation Resumed" } else { "Simulation Paused" });
                                    }
                                    if ui.button("⏭ Step").clicked() {
                                        self.is_running = false;
                                        self.current_time += self.step_size;
                                        let sun_vector = [1.0, 0.0, 0.0];
                                        let b_eci_mock = [1e-5, 2e-5, -3e-5];
                                        for gs in &mut self.ground_stations {
                                            step_atmosphere(gs, &mut self.atmos_model);
                                        }
                                        for segment in &mut self.constellation.segments {
                                            for sat in &mut segment.satellites {
                                                if self.ideal_orbits {
                                                    step_orbit_ideal(sat, self.step_size);
                                                } else {
                                                    step_orbit(sat, self.step_size, &self.config.env, sun_vector);
                                                }
                                                step_attitude(sat, self.step_size, b_eci_mock, [1e-3, -5e-4, 2e-4], [0.1, -0.05, 0.1]);
                                            }
                                        }
                                        self.log("Single Step Executed");
                                    }
                                    if ui.button("↺ Reset").clicked() {
                                        pending_reset = true;
                                    }
                                });
                                let ideal_resp = ui.toggle_value(&mut self.ideal_orbits, "🔒 Orbite Fisse")
                                    .on_hover_text("Blocca la geometria orbitale: i satelliti avanzano a velocità angolare costante sul piano orbitale attuale, senza risentire di fattori esterni (J2, attrito atmosferico, pressione solare)");
                                if ideal_resp.changed() {
                                    self.log(if self.ideal_orbits {
                                        "Fixed ideal orbits ENABLED: external perturbations ignored"
                                    } else {
                                        "Fixed ideal orbits DISABLED: full physics restored"
                                    });
                                }
                            });
                        });

                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("TIME WARP").strong().color(egui::Color32::LIGHT_BLUE));
                                ui.horizontal(|ui| {
                                    ui.add(egui::Slider::new(&mut self.time_warp, -50..=50).text("x"));
                                    ui.separator();
                                    ui.label(format!("Epoch: {:.1}s", self.current_time));
                                });
                            });
                        });

                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("REPORTS").strong().color(egui::Color32::LIGHT_BLUE));
                                if ui.button("📥 Esporta 24h CSV").clicked() {
                                    match self.run_and_export_24h() {
                                        Ok(file) => {
                                            self.log(&format!("Dati di 24h esportati in '{}'", file));
                                        }
                                        Err(e) => {
                                            self.log(&format!("Errore esportazione dati: {}", e));
                                        }
                                    }
                                }
                            });
                        });

                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("📂 CONFIGURATION").strong().color(egui::Color32::LIGHT_BLUE));
                                ui.horizontal(|ui| {
                                    #[cfg(not(target_arch = "wasm32"))]
                                    {
                                        ui.add(egui::TextEdit::singleline(&mut self.config_path).desired_width(120.0));
                                        if ui.button("📥 Import").on_hover_text("Sfoglia e carica un file TOML").clicked() {
                                            if let Some(path) = rfd::FileDialog::new()
                                                .add_filter("TOML Configuration", &["toml"])
                                                .pick_file() {
                                                self.config_path = path.display().to_string();
                                                let _ = self.import_config(&self.config_path.clone());
                                            }
                                        }
                                        if ui.button("📤 Export").on_hover_text("Seleziona cartella e nome file per esportare").clicked() {
                                            if let Some(path) = rfd::FileDialog::new()
                                                .add_filter("TOML Configuration", &["toml"])
                                                .set_file_name("config.toml")
                                                .save_file() {
                                                self.config_path = path.display().to_string();
                                                let _ = self.export_config(&self.config_path.clone());
                                            }
                                        }
                                    }
                                    #[cfg(target_arch = "wasm32")]
                                    {
                                        ui.label("📥 Trascina TOML qui per importare").on_hover_text("Rilascia un file config.toml in qualsiasi punto della finestra");
                                        if ui.button("📤 Export").on_hover_text("Scarica la configurazione corrente come file config.toml").clicked() {
                                            let _ = self.export_config("config.toml");
                                        }
                                    }
                                });
                            });
                        });

                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("HUD WINDOWS").strong().color(egui::Color32::LIGHT_BLUE));
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut self.show_telemetry_hud, "Telemetry");
                                    ui.checkbox(&mut self.show_stations_hud, "Stations");
                                    ui.checkbox(&mut self.show_leo_list_hud, "Bitrates");
                                    ui.checkbox(&mut self.show_logs_hud, "Console Logs");
                                });
                                if ui.button("🔄 Reset Layout")
                                    .on_hover_text("Riapre tutte le finestre HUD e le riporta alla posizione iniziale")
                                    .clicked()
                                {
                                    self.show_telemetry_hud = true;
                                    self.show_stations_hud = true;
                                    self.show_leo_list_hud = true;
                                    self.show_logs_hud = true;
                                    // Forget every stored window position/size so each
                                    // egui::Window falls back to its default_pos/default_size.
                                    ui.ctx().memory_mut(|mem| mem.reset_areas());
                                }
                            });
                        });
                    }

                    RibbonTab::Constellation => {
                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("LEO SEGMENT").strong().color(egui::Color32::LIGHT_BLUE));
                                ui.add(egui::Slider::new(&mut self.leo_num_input, 0..=64).text("Sats"));
                                ui.add(egui::Slider::new(&mut self.leo_alt_input, 200.0..=1200.0).text("Alt (km)"));
                                ui.add(egui::Slider::new(&mut self.leo_inc_input, 0.0..=180.0).text("Inc (°)"));
                            });
                        });

                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("MEO SEGMENT").strong().color(egui::Color32::LIGHT_BLUE));
                                ui.add(egui::Slider::new(&mut self.meo_num_input, 0..=32).text("Sats"));
                                ui.add(egui::Slider::new(&mut self.meo_alt_input, 5000.0..=15000.0).text("Alt (km)"));
                                ui.add(egui::Slider::new(&mut self.meo_inc_input, 0.0..=180.0).text("Inc (°)"));
                            });
                        });

                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("GEO SEGMENT").strong().color(egui::Color32::LIGHT_BLUE));
                                ui.add(egui::Slider::new(&mut self.geo_num_input, 0..=16).text("Sats"));
                                ui.add(egui::Slider::new(&mut self.geo_alt_input, 30000.0..=40000.0).text("Alt (km)"));
                                ui.add(egui::Slider::new(&mut self.geo_inc_input, 0.0..=90.0).text("Inc (°)"));
                            });
                        });

                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("🛠️ CUSTOM ITEMS").strong().color(egui::Color32::LIGHT_BLUE));
                                
                                let r_earth = self.config.env.r_earth;
                                let mut custom_sats = Vec::new();
                                for (seg_idx, seg) in self.constellation.segments.iter().enumerate() {
                                    if seg_idx < 3 {
                                        for sat in &seg.satellites {
                                            if sat.is_custom {
                                                let r = sat.r;
                                                let v = sat.v;
                                                let r_mag = (r[0]*r[0] + r[1]*r[1] + r[2]*r[2]).sqrt();
                                                let alt_km = (r_mag - r_earth) / 1000.0;
                                                
                                                let h_x = r[1]*v[2] - r[2]*v[1];
                                                let h_y = r[2]*v[0] - r[0]*v[2];
                                                let h_z = r[0]*v[1] - r[1]*v[0];
                                                let h_mag = (h_x*h_x + h_y*h_y + h_z*h_z).sqrt();
                                                let inc_deg = if h_mag > 1e-9 {
                                                    (h_z / h_mag).acos().to_degrees()
                                                } else {
                                                    0.0
                                                };
                                                custom_sats.push((seg_idx, sat.id.clone(), alt_km, inc_deg, sat.mass, sat.area));
                                            }
                                        }
                                    }
                                }

                                let mut custom_consts = Vec::new();
                                for i in 3..self.constellation.segments.len() {
                                    let seg = &self.constellation.segments[i];
                                    let const_name = if let Some(first_sat) = seg.satellites.first() {
                                        if let Some(pos) = first_sat.id.rfind('_') {
                                            first_sat.id[..pos].to_string()
                                        } else {
                                            first_sat.id.clone()
                                        }
                                    } else {
                                        format!("Const_{}", i)
                                    };
                                    
                                    let (alt_km, inc_deg, mass, area) = if let Some(sat) = seg.satellites.first() {
                                        let r = sat.r;
                                        let v = sat.v;
                                        let r_mag = (r[0]*r[0] + r[1]*r[1] + r[2]*r[2]).sqrt();
                                        let alt = (r_mag - r_earth) / 1000.0;
                                        let h_x = r[1]*v[2] - r[2]*v[1];
                                        let h_y = r[2]*v[0] - r[0]*v[2];
                                        let h_z = r[0]*v[1] - r[1]*v[0];
                                        let h_mag = (h_x*h_x + h_y*h_y + h_z*h_z).sqrt();
                                        let inc = if h_mag > 1e-9 {
                                            (h_z / h_mag).acos().to_degrees()
                                        } else {
                                            0.0
                                        };
                                        (alt, inc, sat.mass, sat.area)
                                    } else {
                                        (0.0, 0.0, 0.0, 0.0)
                                    };
                                    custom_consts.push((i, const_name, seg.satellites.len(), alt_km, inc_deg, mass, area));
                                }

                                if custom_sats.is_empty() && custom_consts.is_empty() {
                                    ui.label(egui::RichText::new("None").weak().italics());
                                } else {
                                    egui::ScrollArea::vertical()
                                        .id_salt("custom_items_scroll")
                                        .max_height(80.0)
                                        .show(ui, |ui| {
                                            let mut sat_to_remove = None;
                                            for &(seg_idx, ref sat_id, alt, inc, mass, area) in &custom_sats {
                                                ui.horizontal(|ui| {
                                                    ui.label(format!("🛰️ {} ({:.0}km, {:.1}°, {:.0}kg, {:.2}m²)", sat_id, alt, inc, mass, area));
                                                    if ui.button(egui::RichText::new("❌").color(egui::Color32::LIGHT_RED)).on_hover_text("Rimuovi satellite custom").clicked() {
                                                        sat_to_remove = Some((seg_idx, sat_id.clone()));
                                                    }
                                                });
                                            }
                                            if let Some((seg_idx, sat_id)) = sat_to_remove {
                                                if let Some(pos) = self.constellation.segments[seg_idx].satellites.iter().position(|s| s.id == sat_id) {
                                                    self.constellation.segments[seg_idx].satellites.remove(pos);
                                                    match seg_idx {
                                                        0 => {
                                                            if self.config.leo_num > 0 { self.config.leo_num -= 1; }
                                                            self.leo_num_input = self.config.leo_num;
                                                        }
                                                        1 => {
                                                            if self.config.meo_num > 0 { self.config.meo_num -= 1; }
                                                            self.meo_num_input = self.config.meo_num;
                                                        }
                                                        2 => {
                                                            if self.config.geo_num > 0 { self.config.geo_num -= 1; }
                                                            self.geo_num_input = self.config.geo_num;
                                                        }
                                                        _ => {}
                                                    }
                                                    if self.selected_satellite_id == sat_id {
                                                        self.selected_satellite_id = "None".to_string();
                                                        self.update_input_fields_for_selected();
                                                    }
                                                    self.log(&format!("Removed custom satellite: {}", sat_id));
                                                }
                                            }

                                            let mut const_to_remove = None;
                                            for &(seg_idx, ref const_name, sat_count, alt, inc, mass, area) in &custom_consts {
                                                ui.horizontal(|ui| {
                                                    ui.label(format!("🌌 {} ({}sats, {:.0}km, {:.1}°, {:.0}kg, {:.2}m²)", const_name, sat_count, alt, inc, mass, area));
                                                    if ui.button(egui::RichText::new("❌").color(egui::Color32::LIGHT_RED)).on_hover_text("Rimuovi costellazione custom").clicked() {
                                                        const_to_remove = Some(seg_idx);
                                                    }
                                                });
                                            }
                                            if let Some(seg_idx) = const_to_remove {
                                                let seg = self.constellation.segments.remove(seg_idx);
                                                for sat in &seg.satellites {
                                                    if self.selected_satellite_id == sat.id {
                                                        self.selected_satellite_id = "None".to_string();
                                                        self.update_input_fields_for_selected();
                                                    }
                                                }
                                                let const_name = if let Some(first_sat) = seg.satellites.first() {
                                                    if let Some(pos) = first_sat.id.rfind('_') {
                                                        first_sat.id[..pos].to_string()
                                                    } else {
                                                        first_sat.id.clone()
                                                    }
                                                } else {
                                                    "Custom Constellation".to_string()
                                                };
                                                self.log(&format!("Removed custom constellation: {}", const_name));
                                            }
                                        });
                                }
                            });
                        });


                        // Check for changes to apply configuration dynamically
                        let changed = self.config.leo_num != self.leo_num_input
                            || self.config.leo_alt_km != self.leo_alt_input
                            || self.config.leo_inc_deg != self.leo_inc_input
                            || self.config.meo_num != self.meo_num_input
                            || self.config.meo_alt_km != self.meo_alt_input
                            || self.config.meo_inc_deg != self.meo_inc_input
                            || self.config.geo_num != self.geo_num_input
                            || self.config.geo_alt_km != self.geo_alt_input
                            || self.config.geo_inc_deg != self.geo_inc_input;

                        if changed {
                            self.config.leo_num = self.leo_num_input;
                            self.config.leo_alt_km = self.leo_alt_input;
                            self.config.leo_inc_deg = self.leo_inc_input;
                            self.config.meo_num = self.meo_num_input;
                            self.config.meo_alt_km = self.meo_alt_input;
                            self.config.meo_inc_deg = self.meo_inc_input;
                            self.config.geo_num = self.geo_num_input;
                            self.config.geo_alt_km = self.geo_alt_input;
                            self.config.geo_inc_deg = self.geo_inc_input;

                            // 1. Gather all custom segments (index >= 3)
                            let custom_segments: Vec<Segment> = if self.constellation.segments.len() > 3 {
                                self.constellation.segments[3..].to_vec()
                            } else {
                                Vec::new()
                            };

                            // 2. Gather all custom satellites in standard segments (0, 1, 2)
                            let custom_leo: Vec<Satellite> = self.constellation.segments[0].satellites.iter()
                                .filter(|sat| sat.is_custom)
                                .cloned()
                                .collect();
                            let custom_meo: Vec<Satellite> = self.constellation.segments[1].satellites.iter()
                                .filter(|sat| sat.is_custom)
                                .cloned()
                                .collect();
                            let custom_geo: Vec<Satellite> = self.constellation.segments[2].satellites.iter()
                                .filter(|sat| sat.is_custom)
                                .cloned()
                                .collect();

                            // 3. Recreate standard constellation
                            self.constellation = create_satellites_from_config(&self.config);

                            // Helper closure to insert custom satellites while avoiding ID clashes
                            let insert_custom_avoiding_clash = |seg_idx: usize, custom_sats: Vec<Satellite>, segments: &mut Vec<Segment>| {
                                for mut sat in custom_sats {
                                    let mut final_id = sat.id.clone();
                                    let mut sat_idx_counter = segments[seg_idx].satellites.len();
                                    loop {
                                        let mut clash = false;
                                        for s in &segments[seg_idx].satellites {
                                            if s.id == final_id {
                                                clash = true;
                                                break;
                                            }
                                        }
                                        if !clash {
                                            break;
                                        }
                                        final_id = format!("{:?}_{:02}", sat.orbit_type, sat_idx_counter);
                                        sat_idx_counter += 1;
                                    }
                                    sat.id = final_id;
                                    segments[seg_idx].satellites.push(sat);
                                }
                            };

                            // 4. Restore custom satellites to standard segments
                            let segments_mut = &mut self.constellation.segments;
                            insert_custom_avoiding_clash(0, custom_leo, segments_mut);
                            insert_custom_avoiding_clash(1, custom_meo, segments_mut);
                            insert_custom_avoiding_clash(2, custom_geo, segments_mut);

                            // 5. Restore custom segments
                            self.constellation.segments.extend(custom_segments);

                            let mut found_any = false;
                            for seg in &self.constellation.segments {
                                if !seg.satellites.is_empty() {
                                    self.selected_satellite_id = seg.satellites[0].id.clone();
                                    found_any = true;
                                    break;
                                }
                            }
                            if !found_any {
                                self.selected_satellite_id = "None".to_string();
                            }
                            self.update_input_fields_for_selected();
                            self.log("Constellation reconfigured dynamically");
                        }

                        {
                            ui.group(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new("➕ ADD CUSTOM SATELLITE").strong().color(egui::Color32::LIGHT_BLUE));
                                    ui.horizontal(|ui| {
                                        let mut type_changed = false;
                                        if ui.radio_value(&mut self.add_sat_orbit_type, OrbitType::LEO, "LEO").clicked() { type_changed = true; }
                                        if ui.radio_value(&mut self.add_sat_orbit_type, OrbitType::MEO, "MEO").clicked() { type_changed = true; }
                                        if ui.radio_value(&mut self.add_sat_orbit_type, OrbitType::GEO, "GEO").clicked() { type_changed = true; }

                                        if type_changed {
                                            match self.add_sat_orbit_type {
                                                OrbitType::LEO => {
                                                    self.add_sat_alt_km = 550.0;
                                                    self.add_sat_inc_deg = 97.6;
                                                    self.add_sat_mass = 20.0;
                                                    self.add_sat_area = 0.1;
                                                    self.add_sat_cd = 2.2;
                                                    self.add_sat_cr = 1.2;
                                                }
                                                OrbitType::MEO => {
                                                    self.add_sat_alt_km = 10000.0;
                                                    self.add_sat_inc_deg = 55.0;
                                                    self.add_sat_mass = 50.0;
                                                    self.add_sat_area = 0.25;
                                                    self.add_sat_cd = 0.0;
                                                    self.add_sat_cr = 1.2;
                                                }
                                                OrbitType::GEO => {
                                                    self.add_sat_alt_km = 35786.0;
                                                    self.add_sat_inc_deg = 0.0;
                                                    self.add_sat_mass = 200.0;
                                                    self.add_sat_area = 1.5;
                                                    self.add_sat_cd = 0.0;
                                                    self.add_sat_cr = 1.2;
                                                }
                                            }
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        let (alt_min, alt_max) = match self.add_sat_orbit_type {
                                            OrbitType::LEO => (200.0, 1200.0),
                                            OrbitType::MEO => (5000.0, 15000.0),
                                            OrbitType::GEO => (30000.0, 40000.0),
                                        };
                                        ui.vertical(|ui| {
                                            ui.add(egui::Slider::new(&mut self.add_sat_alt_km, alt_min..=alt_max).text("Alt (km)"));
                                            let inc_max = match self.add_sat_orbit_type {
                                                OrbitType::GEO => 90.0,
                                                _ => 180.0,
                                            };
                                            ui.add(egui::Slider::new(&mut self.add_sat_inc_deg, 0.0..=inc_max).text("Inc (°)"));
                                        });
                                        ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.add(egui::DragValue::new(&mut self.add_sat_mass).speed(1.0).range(1.0..=1000.0));
                                                ui.label("Mass (kg)");
                                            });
                                            ui.horizontal(|ui| {
                                                ui.add(egui::DragValue::new(&mut self.add_sat_area).speed(0.01).range(0.01..=10.0));
                                                ui.label("Area (m²)");
                                            });
                                        });
                                        ui.vertical(|ui| {
                                            ui.label("Color:");
                                            egui::color_picker::color_edit_button_rgb(ui, &mut self.add_sat_color);
                                        });
                                        if ui.button("➕ Add").clicked() {
                                            let r_earth = self.config.env.r_earth;
                                            let r_mag = r_earth + self.add_sat_alt_km * 1000.0;
                                            let v_mag = (self.config.env.mu / r_mag).sqrt();
                                            let inc = self.add_sat_inc_deg.to_radians();

                                            let segment_idx = match self.add_sat_orbit_type {
                                                OrbitType::LEO => 0,
                                                OrbitType::MEO => 1,
                                                OrbitType::GEO => 2,
                                            };
                                            
                                            let mut sat_idx_counter = self.constellation.segments[segment_idx].satellites.len();
                                            let mut new_id = format!("{:?}_{:02}", self.add_sat_orbit_type, sat_idx_counter);
                                            loop {
                                                let mut clash = false;
                                                for seg in &self.constellation.segments {
                                                    for sat in &seg.satellites {
                                                        if sat.id == new_id {
                                                            clash = true;
                                                            break;
                                                        }
                                                    }
                                                }
                                                if !clash {
                                                    break;
                                                }
                                                sat_idx_counter += 1;
                                                new_id = format!("{:?}_{:02}", self.add_sat_orbit_type, sat_idx_counter);
                                            }

                                            let segment = &mut self.constellation.segments[segment_idx];

                                            let u = 0.0_f64;
                                            let r_plane = [r_mag * u.cos(), r_mag * u.sin(), 0.0];
                                            let v_plane = [-v_mag * u.sin(), v_mag * u.cos(), 0.0];
                                            let c_i = inc.cos();
                                            let s_i = inc.sin();
                                            let r_eci = [r_plane[0], r_plane[1] * c_i, r_plane[1] * s_i];
                                            let v_eci = [v_plane[0], v_plane[1] * c_i, v_plane[1] * s_i];

                                            let new_sat = Satellite {
                                                id: new_id.clone(),
                                                orbit_type: self.add_sat_orbit_type.clone(),
                                                r: r_eci,
                                                v: v_eci,
                                                q: [1.0, 0.0, 0.0, 0.0],
                                                omega: [0.0, 0.0, 0.0],
                                                mass: self.add_sat_mass,
                                                area: self.add_sat_area,
                                                cd: self.add_sat_cd,
                                                cr: self.add_sat_cr,
                                                inertia: match self.add_sat_orbit_type {
                                                    OrbitType::LEO => [0.4, 0.4, 0.5],
                                                    OrbitType::MEO => [1.5, 1.5, 2.0],
                                                    OrbitType::GEO => [15.0, 15.0, 20.0],
                                                },
                                                h_rw: [0.0, 0.0, 0.0],
                                                is_custom: true,
                                                custom_color: Some([
                                                    (self.add_sat_color[0] * 255.0) as u8,
                                                    (self.add_sat_color[1] * 255.0) as u8,
                                                    (self.add_sat_color[2] * 255.0) as u8,
                                                ]),
                                            };

                                            segment.satellites.push(new_sat);
                                            match self.add_sat_orbit_type {
                                                OrbitType::LEO => {
                                                    self.config.leo_num += 1;
                                                    self.leo_num_input = self.config.leo_num;
                                                }
                                                OrbitType::MEO => {
                                                    self.config.meo_num += 1;
                                                    self.meo_num_input = self.config.meo_num;
                                                }
                                                OrbitType::GEO => {
                                                    self.config.geo_num += 1;
                                                    self.geo_num_input = self.config.geo_num;
                                                }
                                            }

                                            self.selected_satellite_id = new_id.clone();
                                            self.update_input_fields_for_selected();
                                            self.log(&format!("Added custom satellite: {}", new_id));
                                        }
                                    });
                                });
                            });

                            ui.group(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new("➕ ADD CUSTOM CONSTELLATION").strong().color(egui::Color32::LIGHT_BLUE));
                                    ui.horizontal(|ui| {
                                        ui.add(egui::TextEdit::singleline(&mut self.add_const_name).desired_width(80.0));
                                        
                                        let mut type_changed = false;
                                        if ui.radio_value(&mut self.add_const_orbit_type, OrbitType::LEO, "LEO").clicked() { type_changed = true; }
                                        if ui.radio_value(&mut self.add_const_orbit_type, OrbitType::MEO, "MEO").clicked() { type_changed = true; }
                                        if ui.radio_value(&mut self.add_const_orbit_type, OrbitType::GEO, "GEO").clicked() { type_changed = true; }

                                        if type_changed {
                                            match self.add_const_orbit_type {
                                                OrbitType::LEO => {
                                                    self.add_const_alt_km = 600.0;
                                                    self.add_const_inc_deg = 45.0;
                                                    self.add_const_mass = 25.0;
                                                    self.add_const_area = 0.15;
                                                    self.add_const_cd = 2.2;
                                                    self.add_const_cr = 1.2;
                                                }
                                                OrbitType::MEO => {
                                                    self.add_const_alt_km = 10000.0;
                                                    self.add_const_inc_deg = 55.0;
                                                    self.add_const_mass = 50.0;
                                                    self.add_const_area = 0.25;
                                                    self.add_const_cd = 0.0;
                                                    self.add_const_cr = 1.2;
                                                }
                                                OrbitType::GEO => {
                                                    self.add_const_alt_km = 35786.0;
                                                    self.add_const_inc_deg = 0.0;
                                                    self.add_const_mass = 200.0;
                                                    self.add_const_area = 1.5;
                                                    self.add_const_cd = 0.0;
                                                    self.add_const_cr = 1.2;
                                                }
                                            }
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        let (alt_min, alt_max) = match self.add_const_orbit_type {
                                            OrbitType::LEO => (200.0, 1200.0),
                                            OrbitType::MEO => (5000.0, 15000.0),
                                            OrbitType::GEO => (30000.0, 40000.0),
                                        };
                                        ui.vertical(|ui| {
                                            ui.spacing_mut().slider_width = 70.0;
                                            ui.horizontal(|ui| {
                                                ui.add(egui::DragValue::new(&mut self.add_const_num_sats).speed(1.0).range(1..=128));
                                                ui.label("Sats");
                                            });
                                            ui.add(egui::Slider::new(&mut self.add_const_alt_km, alt_min..=alt_max).text("Alt"));
                                            let inc_max = match self.add_const_orbit_type {
                                                OrbitType::GEO => 90.0,
                                                _ => 180.0,
                                            };
                                            ui.add(egui::Slider::new(&mut self.add_const_inc_deg, 0.0..=inc_max).text("Inc"));
                                        });
                                        ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.add(egui::DragValue::new(&mut self.add_const_mass).speed(1.0).range(1.0..=1000.0));
                                                ui.label("Mass (kg)");
                                            });
                                            ui.horizontal(|ui| {
                                                ui.add(egui::DragValue::new(&mut self.add_const_area).speed(0.01).range(0.01..=10.0));
                                                ui.label("Area (m²)");
                                            });
                                        });
                                        ui.vertical(|ui| {
                                            ui.label("Color:");
                                            egui::color_picker::color_edit_button_rgb(ui, &mut self.add_const_color);
                                        });
                                        if ui.button("➕ Create").clicked() {
                                            let mut final_const_name = self.add_const_name.clone();
                                            let mut suffix_idx = 1;
                                            loop {
                                                let mut clash = false;
                                                for seg in &self.constellation.segments {
                                                    for sat in &seg.satellites {
                                                        if sat.id.starts_with(&format!("{}_", final_const_name)) {
                                                            clash = true;
                                                            break;
                                                        }
                                                    }
                                                }
                                                if !clash {
                                                    break;
                                                }
                                                final_const_name = format!("{}{}", self.add_const_name, suffix_idx);
                                                suffix_idx += 1;
                                            }

                                            let r_earth = self.config.env.r_earth;
                                            let r_mag = r_earth + self.add_const_alt_km * 1000.0;
                                            let v_mag = (self.config.env.mu / r_mag).sqrt();
                                            let inc = self.add_const_inc_deg.to_radians();

                                            let mut new_sats = Vec::new();
                                            let num_sats = self.add_const_num_sats;
                                            for k in 0..num_sats {
                                                let u = (2.0 * std::f64::consts::PI * k as f64) / num_sats as f64;
                                                let r_plane = [r_mag * u.cos(), r_mag * u.sin(), 0.0];
                                                let v_plane = [-v_mag * u.sin(), v_mag * u.cos(), 0.0];
                                                let c_i = inc.cos();
                                                let s_i = inc.sin();
                                                let r_eci = [r_plane[0], r_plane[1] * c_i, r_plane[1] * s_i];
                                                let v_eci = [v_plane[0], v_plane[1] * c_i, v_plane[1] * s_i];

                                                let new_id = format!("{}_{:02}", final_const_name, k);
                                                new_sats.push(Satellite {
                                                    id: new_id,
                                                    orbit_type: self.add_const_orbit_type.clone(),
                                                    r: r_eci,
                                                    v: v_eci,
                                                    q: [1.0, 0.0, 0.0, 0.0],
                                                    omega: [0.0, 0.0, 0.0],
                                                    mass: self.add_const_mass,
                                                    area: self.add_const_area,
                                                    cd: self.add_const_cd,
                                                    cr: self.add_const_cr,
                                                    inertia: match self.add_const_orbit_type {
                                                        OrbitType::LEO => [0.4, 0.4, 0.5],
                                                        OrbitType::MEO => [1.5, 1.5, 2.0],
                                                        OrbitType::GEO => [15.0, 15.0, 20.0],
                                                    },
                                                    h_rw: [0.0, 0.0, 0.0],
                                                    is_custom: true,
                                                    custom_color: Some([
                                                        (self.add_const_color[0] * 255.0) as u8,
                                                        (self.add_const_color[1] * 255.0) as u8,
                                                        (self.add_const_color[2] * 255.0) as u8,
                                                    ]),
                                                });
                                            }

                                            let new_segment = Segment {
                                                orbit_type: self.add_const_orbit_type.clone(),
                                                satellites: new_sats,
                                            };
                                            self.constellation.segments.push(new_segment);
                                            self.log(&format!("Created custom constellation: {} with {} satellites", self.add_const_name, num_sats));
                                        }
                                    });
                                });
                            });
                        }
                    }

                    RibbonTab::Network => {
                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("MAP FILTERS").strong().color(egui::Color32::LIGHT_BLUE));
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut self.show_leo, "LEO ISL");
                                    ui.checkbox(&mut self.show_meo, "MEO ISL");
                                    ui.checkbox(&mut self.show_geo, "GEO ISL");
                                    ui.checkbox(&mut self.show_sgl, "Ground Links (SGL)");
                                });
                            });
                        });

                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("LEO ROUTING PRIORITY").strong().color(egui::Color32::LIGHT_BLUE));
                                ui.horizontal(|ui| {
                                    ui.radio_value(&mut self.prioritize_relay, false, "Ground First (SGL)");
                                    ui.radio_value(&mut self.prioritize_relay, true, "Relay Only (ISL)");
                                });
                            });
                        });

                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("MAX BITRATES").strong().color(egui::Color32::LIGHT_BLUE));
                                ui.horizontal(|ui| {
                                    ui.add(egui::Slider::new(&mut self.leo_max_bitrate, 10.0..=500.0).text("LEO (Gbps)"));
                                    ui.add(egui::Slider::new(&mut self.meo_max_bitrate, 50.0..=2000.0).text("MEO (Gbps)"));
                                    ui.add(egui::Slider::new(&mut self.geo_max_bitrate, 100.0..=5000.0).text("GEO (Gbps)"));
                                });
                            });
                        });

                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("MAP ZOOM").strong().color(egui::Color32::LIGHT_BLUE));
                                ui.add(egui::Slider::new(&mut self.map_zoom, 0.1..=10.0).logarithmic(true).text("Zoom"));
                            });
                        });
                    }

                    RibbonTab::Adcs => {
                            ui.group(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new("PHYSICAL EDIT").strong().color(egui::Color32::LIGHT_BLUE));
                                    ui.add(egui::Slider::new(&mut self.sat_mass_input, 1.0..=500.0).text("Mass (kg)"));
                                    ui.add(egui::Slider::new(&mut self.sat_cd_input, 0.0..=4.0).text("Cd"));
                                    ui.add(egui::Slider::new(&mut self.sat_cr_input, 0.0..=3.0).text("Cr"));
                                    if ui.button("Apply Parameters").clicked() {
                                        let id = self.selected_satellite_id.clone();
                                        for seg in &mut self.constellation.segments {
                                            for s in &mut seg.satellites {
                                                if s.id == id {
                                                    s.mass = self.sat_mass_input;
                                                    s.cd = self.sat_cd_input;
                                                    s.cr = self.sat_cr_input;
                                                }
                                            }
                                        }
                                        self.log(&format!("Updated physical params for satellite {}", id));
                                    }
                                });
                            });

                            ui.group(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new("DISTURBANCE TORQUE").strong().color(egui::Color32::LIGHT_BLUE));
                                    ui.add(egui::Slider::new(&mut self.disturbance_val[0], -10.0..=10.0).text("Tx"));
                                    ui.add(egui::Slider::new(&mut self.disturbance_val[1], -10.0..=10.0).text("Ty"));
                                    ui.add(egui::Slider::new(&mut self.disturbance_val[2], -10.0..=10.0).text("Tz"));
                                    if ui.button("⚡ Inject Torque").clicked() {
                                        self.force_disturbance = true;
                                    }
                                });
                            });

                            ui.group(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new("SENSOR NOISE").strong().color(egui::Color32::LIGHT_BLUE));
                                    ui.add(egui::Slider::new(&mut self.gyro_noise, 1e-7..=1e-3).logarithmic(true).text("Gyro"));
                                    ui.add(egui::Slider::new(&mut self.mag_noise, 1e-9..=1e-5).logarithmic(true).text("Mag"));
                                    ui.add(egui::Slider::new(&mut self.sun_noise, 1e-5..=1e-1).logarithmic(true).text("Sun"));
                                    ui.add(egui::Slider::new(&mut self.st_noise, 1e-6..=1e-2).logarithmic(true).text("Star"));
                                });
                            });
                    }

                    RibbonTab::Weather => {
                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("WEATHER STATION OVERRIDES").strong().color(egui::Color32::LIGHT_BLUE));
                                let n = self.ground_stations.len();
                                let cols = (n as f64).sqrt().ceil() as usize;
                                let cols = if cols == 0 { 1 } else { cols };

                                egui::Grid::new("weather_grid")
                                    .spacing([15.0, 10.0])
                                    .show(ui, |ui| {
                                        for i in 0..n {
                                            let name = self.ground_stations[i].name.clone();
                                            ui.vertical(|ui| {
                                                ui.small(&name);
                                                ui.horizontal(|ui| {
                                                    let btn_markov = ui.selectable_label(self.weather_overrides[i].is_none(), "🔄");
                                                    let btn_markov = btn_markov.on_hover_text("Markov (Dynamic Auto Weather)");
                                                    if btn_markov.clicked() {
                                                        self.weather_overrides[i] = None;
                                                    }
                                                    for w_idx in 0..self.atmos_model.states.len() {
                                                        let (wx_icon, wx_desc) = match w_idx {
                                                            0 => ("☀", "Clear Sky"),
                                                            1 => ("⛅", "Thin Clouds"),
                                                            2 => ("☁", "Thick Clouds"),
                                                            _ => ("☔", "Heavy Rain / Storm"),
                                                        };
                                                        let btn_wx = ui.selectable_label(self.weather_overrides[i] == Some(w_idx), wx_icon);
                                                        let btn_wx = btn_wx.on_hover_text(wx_desc);
                                                        if btn_wx.clicked() {
                                                            self.weather_overrides[i] = Some(w_idx);
                                                        }
                                                    }
                                                });
                                            });
                                            if (i + 1) % cols == 0 {
                                                ui.end_row();
                                            }
                                        }
                                    });
                            });
                        });

                        {
                            ui.group(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new("EDIT STATIONS").strong().color(egui::Color32::LIGHT_BLUE));
                                    ui.horizontal(|ui| {
                                        let mut to_remove = None;
                                        for i in 0..self.ground_stations.len() {
                                            ui.vertical(|ui| {
                                                ui.group(|ui| {
                                                    ui.spacing_mut().slider_width = 80.0;
                                                    ui.horizontal(|ui| {
                                                        let mut name_edit = self.ground_stations[i].name.clone();
                                                        if ui.add(egui::TextEdit::singleline(&mut name_edit).desired_width(90.0)).changed() {
                                                            self.ground_stations[i].name = name_edit;
                                                        }
                                                        if ui.button("↺").on_hover_text("Reset to defaults").clicked() {
                                                            if let Some(orig) = self.config.stations.iter().find(|s| s.id == self.ground_stations[i].id) {
                                                                self.ground_stations[i].name = orig.name.clone();
                                                                self.ground_stations[i].lat_rad = orig.lat_rad;
                                                                self.ground_stations[i].lon_rad = orig.lon_rad;
                                                                self.ground_stations[i].alt_m = orig.alt_m;
                                                            } else {
                                                                self.ground_stations[i].name = format!("Station_{}", i);
                                                                self.ground_stations[i].lat_rad = 0.0;
                                                                self.ground_stations[i].lon_rad = 0.0;
                                                                self.ground_stations[i].alt_m = 100.0;
                                                            }
                                                        }
                                                        if ui.button("❌").clicked() {
                                                            to_remove = Some(i);
                                                        }
                                                    });
                                                    let mut lat_deg = self.ground_stations[i].lat_rad.to_degrees();
                                                    let mut lon_deg = self.ground_stations[i].lon_rad.to_degrees();
                                                    let mut alt_m = self.ground_stations[i].alt_m;

                                                    if ui.add(egui::Slider::new(&mut lat_deg, -90.0..=90.0).text("Lat")).changed() {
                                                        self.ground_stations[i].lat_rad = lat_deg.to_radians();
                                                    }
                                                    if ui.add(egui::Slider::new(&mut lon_deg, -180.0..=180.0).text("Lon")).changed() {
                                                        self.ground_stations[i].lon_rad = lon_deg.to_radians();
                                                    }
                                                    if ui.add(egui::Slider::new(&mut alt_m, 0.0..=5000.0).text("Alt")).changed() {
                                                        self.ground_stations[i].alt_m = alt_m;
                                                    }
                                                });
                                            });
                                        }
                                        if let Some(idx) = to_remove {
                                            pending_remove = Some(idx);
                                        }
                                        if ui.button("➕ Add Station").clicked() {
                                            pending_add = true;
                                        }
                                    });
                                });
                            });
                        }
                    }
                }
            };
        }

            // Horizontal scroll + non-wrapping row: the ribbon groups keep their natural
            // size and the panel keeps a fixed height. When the window is too narrow to
            // show every group, a horizontal scrollbar appears instead of the groups
            // shrinking or reflowing — identical behaviour on desktop and mobile.
            egui::ScrollArea::horizontal()
                .id_salt("ribbon_scroll")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Disable text wrapping so every group takes its natural width.
                        // Without this, egui hands the row only the viewport width and the
                        // trailing groups squeeze (their labels wrapping char-by-char) instead
                        // of the row overflowing into the horizontal scrollbar.
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        render_ribbon_contents!(ui);
                    });
                });
            ui.add_space(4.0);
        });

        // 3. Floating HUD Windows (egui::Window)
        // Windows are constrained to stay on-screen so they work at any width,
        // from a phone viewport up to a desktop monitor.
        macro_rules! make_window {
            ($title:expr, $open:expr, $def_pos:expr, $def_size:expr) => {{
                egui::Window::new($title)
                    .open($open)
                    .default_pos($def_pos)
                    .default_size($def_size)
                    .constrain(true)
                    .movable(true)
                    .resizable(true)
            }};
        }

        {
            if self.show_telemetry_hud {
            let mut open = self.show_telemetry_hud;
            make_window!("📊 Telemetria Satellite", &mut open, egui::pos2(850.0, 150.0), egui::vec2(280.0, 320.0))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Seleziona:");
                        egui::ComboBox::from_label("")
                            .selected_text(self.selected_satellite_id.clone())
                            .show_ui(ui, |ui| {
                                let sat_ids: Vec<String> = self.constellation.segments.iter()
                                    .flat_map(|seg| seg.satellites.iter().map(|s| s.id.clone()))
                                    .collect();
                                for id in sat_ids {
                                    if ui.selectable_value(&mut self.selected_satellite_id, id.clone(), id.clone()).clicked() {
                                        self.update_input_fields_for_selected();
                                    }
                                }
                            });
                    });

                    ui.separator();

                    let sat_telemetry = self.find_satellite(&self.selected_satellite_id).map(|s| (
                        s.mass,
                        s.inertia,
                        s.r,
                        s.v,
                        s.q,
                        s.omega,
                        s.h_rw,
                        s.orbit_type.clone(),
                    ));

                    if let Some((mass, inertia, r, v, q, omega, h_rw, orbit_type)) = sat_telemetry {
                        let max_spd = match orbit_type {
                            OrbitType::LEO => self.leo_max_bitrate,
                            OrbitType::MEO => self.meo_max_bitrate,
                            OrbitType::GEO => self.geo_max_bitrate,
                        };
                        
                        {
                            ui.label(format!("Vel. Max Canale: {:.0} Gbps", max_spd));
                            ui.label(format!("Massa Bus: {:.1} kg", mass));
                            ui.label(format!("Inerzia: [{:.2}, {:.2}, {:.2}]", inertia[0], inertia[1], inertia[2]));
                            
                            ui.separator();
                            ui.label(egui::RichText::new("Orbita (ECI):").strong());
                            ui.small(format!("Pos: [{:.1}, {:.1}, {:.1}] km", r[0]/1000.0, r[1]/1000.0, r[2]/1000.0));
                            ui.small(format!("Vel: [{:.3}, {:.3}, {:.3}] km/s", v[0]/1000.0, v[1]/1000.0, v[2]/1000.0));

                            ui.separator();
                            ui.label(egui::RichText::new("Attitudine & ADCS:").strong());
                            ui.small(format!("Q: [{:.4}, {:.4}, {:.4}, {:.4}]", q[0], q[1], q[2], q[3]));
                            ui.small(format!("Omega: [{:.4}, {:.4}, {:.4}] rad/s", omega[0], omega[1], omega[2]));
                            ui.small(format!("H_rw: [{:.4}, {:.4}, {:.4}] Nms", h_rw[0], h_rw[1], h_rw[2]));
                        }
                        ui.separator();
                        // Link geometry towards connected GS / ISL partner
                        ui.label(egui::RichText::new("Geometria Link:").strong());
                        let sat_id = &self.selected_satellite_id;
                        // Find satellite ECI position
                        if let Some(sat) = self.find_satellite(sat_id) {
                            let sat_r_eci = sat.r;
                            // SGL link → connected ground station
                            if let Some((gs_name, _cap)) = sat_sgl_link.get(sat_id) {
                                if let Some(gs) = self.ground_stations.iter().find(|g| &g.name == gs_name) {
                                    let gs_ecef = lla_to_ecef(gs.lat_rad, gs.lon_rad, gs.alt_m);
                                    let gst = self.current_time * 7.292115e-5;
                                    let rot = eci_to_ecef_matrix(gst);
                                    let rot_t = [[rot[0][0],rot[1][0],rot[2][0]],[rot[0][1],rot[1][1],rot[2][1]],[rot[0][2],rot[1][2],rot[2][2]]];
                                    let gs_eci = mat_vec_mult(rot_t, gs_ecef);
                                    let (az, el, dist) = az_el_dist(gs_eci, gs.lat_rad, gs.lon_rad + gst, sat_r_eci);
                                    ui.small(format!("📡 GS {} → sat", gs_name));
                                    ui.small(format!("  Az {:.1}°  El {:.1}°  Dist {:.0} km", az, el, dist));
                                }
                            }
                            // ISL link → partner satellite
                            if let Some((partner_id, _cap)) = sat_isl_link.get(sat_id) {
                                if let Some(partner) = self.find_satellite(partner_id) {
                                    let r_len = norm(sat_r_eci);
                                    let sat_lat = if r_len > 0.0 { (sat_r_eci[2] / r_len).asin() } else { 0.0 };
                                    let sat_lon = sat_r_eci[1].atan2(sat_r_eci[0]);
                                    let (az, el, dist) = az_el_dist(sat_r_eci, sat_lat, sat_lon, partner.r);
                                    ui.small(format!("🛰 ISL → {}", partner_id));
                                    ui.small(format!("  Az {:.1}°  El {:.1}°  Dist {:.0} km", az, el, dist));
                                }
                            }
                        }
                    }
                });
            self.show_telemetry_hud = open;
        }

        if self.show_stations_hud {
            let mut open = self.show_stations_hud;
            make_window!("📡 Stazioni di Terra", &mut open, egui::pos2(50.0, 150.0), egui::vec2(280.0, 300.0))
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().id_salt("hud_gs_scroll").show(ui, |ui| {
                        for (gs_idx, gs) in self.ground_stations.iter().enumerate() {
                            let weather_name = &self.atmos_model.states[gs.atmos_state];
                            // The icon is the redundant, non-color cue for the
                            // weather state; the color follows the CVD-safe palette.
                            let (wx_icon, wx_color) = match gs.atmos_state {
                                0 => ("☀", STATUS_GOOD),
                                1 => ("⛅", STATUS_WARN),
                                2 => ("☁", STATUS_IDLE),
                                _ => ("☔", STATUS_BAD),
                            };
                            let connected = &connected_sats_per_gs[gs_idx];
                            let total_gbps = gs_throughputs[gs_idx] as f64;

                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.colored_label(wx_color, wx_icon);
                                    ui.colored_label(egui::Color32::WHITE, &gs.name);
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.colored_label(wx_color, weather_name.to_uppercase());
                                    });
                                });
                                ui.horizontal(|ui| {
                                    ui.small(format!("Throughput: {:.1} Gbps", total_gbps));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        let cap_str = if gs.downlink_nominal_gbps.is_infinite() {
                                            "Illimitata".to_string()
                                        } else {
                                            format!("{:.1} Gbps", gs.downlink_nominal_gbps)
                                        };
                                        ui.small(format!("Cap: {}", cap_str));
                                    });
                                });
                                if !connected.is_empty() {
                                    ui.separator();
                                    for (sat_id, _, speed, _) in connected {
                                        // Compute Az/El/Dist of this satellite as seen from the GS
                                        if let Some(sat) = self.find_satellite(sat_id) {
                                            let gst = self.current_time * 7.292115e-5;
                                            let rot = eci_to_ecef_matrix(gst);
                                            let rot_t = [[rot[0][0],rot[1][0],rot[2][0]],[rot[0][1],rot[1][1],rot[2][1]],[rot[0][2],rot[1][2],rot[2][2]]];
                                            let gs_ecef = lla_to_ecef(gs.lat_rad, gs.lon_rad, gs.alt_m);
                                            let gs_eci_pos = mat_vec_mult(rot_t, gs_ecef);
                                            let (az, el, dist) = az_el_dist(gs_eci_pos, gs.lat_rad, gs.lon_rad + gst, sat.r);
                                            ui.small(format!("  • {} {:.1} Gbps", sat_id, speed));
                                            ui.small(format!("    Az {:.1}°  El {:.1}°  Dist {:.0} km", az, el, dist));
                                        } else {
                                            ui.small(format!("  • {}: {:.1} Gbps", sat_id, speed));
                                        }
                                    }
                                }
                            });
                        }
                    });
                });
            self.show_stations_hud = open;
        }

        if self.show_leo_list_hud {
            let mut open = self.show_leo_list_hud;
            make_window!("📶 Bitrates", &mut open, egui::pos2(50.0, 480.0), egui::vec2(280.0, 200.0))
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().id_salt("hud_bitrates_scroll").show(ui, |ui| {
                        ui.label(egui::RichText::new("SATELLITES").strong().color(egui::Color32::LIGHT_BLUE));
                        
                        let mut all_sats = Vec::new();
                        for seg in &self.constellation.segments {
                            for sat in &seg.satellites {
                                all_sats.push(sat.id.clone());
                            }
                        }
                        all_sats.sort();

                        for sat_id in all_sats {
                            let sgl_info = sat_sgl_link.get(&sat_id);
                            let _isl_info = sat_isl_link.get(&sat_id);
                            let total_speed = sgl_info.map(|(_, cap)| *cap).unwrap_or(0.0) + _isl_info.map(|(_, cap)| *cap).unwrap_or(0.0);
                            
                            // Filled/empty dot marks active vs idle links without
                            // relying on color alone (CVD-safe redundancy). U+26AB/U+26AA:
                            // the only filled/empty circle pair in the loaded fonts.
                            let (marker, color) = if total_speed > 50.0 {
                                ("⚫", STATUS_GOOD)
                            } else if total_speed > 0.0 {
                                ("⚫", STATUS_WARN)
                            } else {
                                ("⚪", STATUS_IDLE)
                            };

                            ui.horizontal(|ui| {
                                let is_selected = sat_id == self.selected_satellite_id;
                                if ui.selectable_label(is_selected, &sat_id).clicked() {
                                    self.selected_satellite_id = sat_id.clone();
                                    self.update_input_fields_for_selected();
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.colored_label(color, format!("{} {:.1} Gbps", marker, total_speed));
                                });
                            });
                        }

                        ui.separator();
                        ui.label(egui::RichText::new("GROUND STATIONS").strong().color(egui::Color32::LIGHT_BLUE));

                        for (gs_idx, gs) in self.ground_stations.iter().enumerate() {
                            let total_speed = gs_throughputs[gs_idx] as f64;
                            let (marker, color) = if total_speed > 50.0 {
                                ("⚫", STATUS_GOOD)
                            } else if total_speed > 0.0 {
                                ("⚫", STATUS_WARN)
                            } else {
                                ("⚪", STATUS_IDLE)
                            };
                            ui.horizontal(|ui| {
                                ui.label(&gs.name);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.colored_label(color, format!("{} {:.1} Gbps", marker, total_speed));
                                });
                            });
                        }
                    });
                });
            self.show_leo_list_hud = open;
        }

        if self.show_logs_hud {
            let mut open = self.show_logs_hud;
            make_window!("💻 Console di Sistema", &mut open, egui::pos2(850.0, 500.0), egui::vec2(280.0, 180.0))
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                        for log_msg in &self.logs {
                            ui.label(log_msg);
                        }
                    });
                });
            self.show_logs_hud = open;
        }
        }

        // Throughput chart panel: height is relative to the window and can be dragged
        // (grab the top edge of the panel). egui persists the user's chosen height across
        // frames via the panel id, while the bounds stay proportional to the window so it
        // never collapses or swallows the whole view when the window is resized.
        let screen_h = ctx.content_rect().height();
        // Clamp the max above the min so a very short window can never invert the range.
        let chart_max_h = (screen_h * 0.6).max(120.0);
        egui::Panel::bottom("bottom_panel")
            .resizable(true)
            .default_size(screen_h * 0.22)
            .size_range(90.0..=chart_max_h)
            .show(root_ui, |ui| {
                let (rect, _response) = ui.allocate_exact_size(
                    ui.available_size(),
                    egui::Sense::hover()
                );
                self.draw_throughput_chart(ui, rect);
            });

        egui::CentralPanel::default().show(root_ui, |ui| {
            let (rect, response) = ui.allocate_exact_size(
                ui.available_size(),
                egui::Sense::drag()
            );

            if response.hovered() {
                let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll_delta != 0.0 {
                    let zoom_factor = (scroll_delta * 0.003).exp();
                    self.map_zoom = (self.map_zoom * zoom_factor).clamp(0.1, 10.0);
                }
            }

            // Pinch-to-zoom: two-finger pinch on touchscreens (and trackpads). zoom_delta()
            // is the multiplicative gesture factor (1.0 = no change). Gated on contains_pointer
            // rather than hovered() because touch input doesn't set the hover state.
            if response.contains_pointer() {
                let pinch = ui.input(|i| i.zoom_delta());
                if (pinch - 1.0).abs() > f32::EPSILON {
                    self.map_zoom = (self.map_zoom * pinch).clamp(0.1, 10.0);
                }
            }

            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(4, 5, 12));

            // Starfield backdrop
            let mut star_lcg = Lcg::new(424242);
            for _ in 0..120 {
                let sx = rect.min.x + (star_lcg.next_f64() as f32) * rect.width();
                let sy = rect.min.y + (star_lcg.next_f64() as f32) * rect.height();
                let size = 0.5 + (star_lcg.next_f64() as f32) * 1.5;
                let b = 100 + (star_lcg.next_f64() * 155.0) as u8;
                let color = egui::Color32::from_rgba_unmultiplied(b, b, 255, b);
                painter.circle_filled(egui::pos2(sx, sy), size, color);
            }

            let center = rect.center();
            
            let max_r = self.config.env.r_earth + self.config.geo_alt_km * 1000.0;
            let screen_dim = rect.width().min(rect.height());
            let scale = ((screen_dim * 0.45) as f64 / max_r) * (self.map_zoom as f64);

            let map_yaw = self.map_yaw;
            let map_pitch = self.map_pitch;
            // 3D projection closure: projects [x, y, z] to screen space and returns (pos2, rotated_z)
            let project_3d = move |pos: [f64; 3]| -> (egui::Pos2, f64) {
                let x = pos[0];
                let y = -pos[1]; // Invert Y to correct longitude coordinate system orientation
                let z = pos[2];

                // 1. Rotate around Z-axis (North-South axis) by map_yaw
                let cos_yaw = (map_yaw as f64).cos();
                let sin_yaw = (map_yaw as f64).sin();
                let x1 = x * cos_yaw - y * sin_yaw;
                let y1 = x * sin_yaw + y * cos_yaw;
                let z1 = z;

                // 2. Rotate around X-axis by map_pitch
                let cos_pitch = (map_pitch as f64).cos();
                let sin_pitch = (map_pitch as f64).sin();
                let x2 = x1;
                let y2 = -z1 * cos_pitch + y1 * sin_pitch;
                let z2 = z1 * sin_pitch + y1 * cos_pitch; // positive is towards camera

                // 3. Screen projection
                let screen_x = center.x + (x2 * scale) as f32;
                let screen_y = center.y + (y2 * scale) as f32;

                (egui::pos2(screen_x, screen_y), z2)
            };

            let mut rotate_globe = true;
            let mut drag_to_perform = None;
            if let Some(ref sat_id) = self.dragging_satellite_id {
                rotate_globe = false;
                if response.dragged() {
                    if let Some(mouse_pos) = ui.input(|i| i.pointer.latest_pos()) {
                        drag_to_perform = Some((sat_id.clone(), mouse_pos));
                    }
                }
            } else {
                if response.drag_started() {
                    if let Some(mouse_pos) = ui.input(|i| i.pointer.press_origin()) {
                        for seg in &self.constellation.segments {
                            for sat in &seg.satellites {
                                let (sat_pos_px, rot_z) = project_3d(sat.r);
                                if rot_z > 0.0 {
                                    if sat_pos_px.distance(mouse_pos) < 12.0 {
                                        self.dragging_satellite_id = Some(sat.id.clone());
                                        rotate_globe = false;
                                        break;
                                    }
                                }
                            }
                            if !rotate_globe {
                                break;
                            }
                        }
                    }
                }
            }

            if let Some((sat_id, mouse_pos)) = drag_to_perform {
                self.drag_satellite_to(&sat_id, mouse_pos, center, scale);
            }

            if !ui.input(|i| i.pointer.any_down()) {
                self.dragging_satellite_id = None;
            }

            if rotate_globe && response.dragged() {
                let delta = response.drag_delta();
                self.map_yaw += delta.x * 0.005;
                self.map_pitch = (self.map_pitch - delta.y * 0.005).clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
            }

            // Draw Earth (textured 3D sphere mesh, or fallback to solid blue circle)
            let r_earth = self.config.env.r_earth;
            let earth_radius_px = (r_earth * scale) as f32;

            // Concentric atmospheric glow
            for i in 1..=6 {
                let alpha = (50 / i) as u8;
                let glow_radius = earth_radius_px + i as f32 * 3.0;
                painter.circle_filled(
                    center,
                    glow_radius,
                    egui::Color32::from_rgba_unmultiplied(56, 189, 248, alpha),
                );
            }

            if let Some(ref texture) = self.earth_texture {
                let n_lat = 32;
                let n_lon = 64;
                let mut projected_vertices = vec![vec![(egui::pos2(0.0, 0.0), 0.0); n_lon + 1]; n_lat + 1];
                for i in 0..=n_lat {
                    let lat_rad = -std::f64::consts::FRAC_PI_2 + (i as f64) * std::f64::consts::PI / (n_lat as f64);
                    let z = r_earth * lat_rad.sin();
                    let r_lat = r_earth * lat_rad.cos();
                    
                    for j in 0..=n_lon {
                        let lon_rad = (j as f64) * 2.0 * std::f64::consts::PI / (n_lon as f64) + gst + 180.0_f64.to_radians();
                        let x = r_lat * lon_rad.cos();
                        let y = r_lat * lon_rad.sin();
                        
                        projected_vertices[i][j] = project_3d([x, y, z]);
                    }
                }

                let mut mesh = egui::Mesh::with_texture(texture.id());
                let mut vertex_indices = vec![vec![u32::MAX; n_lon + 1]; n_lat + 1];

                for i in 0..n_lat {
                    for j in 0..n_lon {
                        let p00 = projected_vertices[i][j];
                        let p10 = projected_vertices[i+1][j];
                        let p01 = projected_vertices[i][j+1];
                        let p11 = projected_vertices[i+1][j+1];

                        let avg_z = (p00.1 + p10.1 + p01.1 + p11.1) / 4.0;
                        if avg_z > 0.0 {
                            let mut add_vertex = |row: usize, col: usize, mesh: &mut egui::Mesh| -> u32 {
                                if vertex_indices[row][col] == u32::MAX {
                                    let (pos, _) = projected_vertices[row][col];
                                    let u = col as f32 / n_lon as f32;
                                    let v = 1.0 - (row as f32 / n_lat as f32);
                                    let idx = mesh.vertices.len() as u32;
                                    mesh.vertices.push(egui::epaint::Vertex {
                                        pos,
                                        uv: egui::pos2(u, v),
                                        color: egui::Color32::WHITE,
                                    });
                                    vertex_indices[row][col] = idx;
                                    idx
                                } else {
                                    vertex_indices[row][col]
                                }
                            };

                            let idx00 = add_vertex(i, j, &mut mesh);
                            let idx10 = add_vertex(i + 1, j, &mut mesh);
                            let idx01 = add_vertex(i, j + 1, &mut mesh);
                            let idx11 = add_vertex(i + 1, j + 1, &mut mesh);

                            mesh.add_triangle(idx00, idx10, idx01);
                            mesh.add_triangle(idx10, idx11, idx01);
                        }
                    }
                }
                painter.add(mesh);
            } else {
                painter.circle_filled(center, earth_radius_px, egui::Color32::from_rgb(15, 76, 129));
            }
            painter.circle_stroke(center, earth_radius_px, egui::Stroke::new(1.5, egui::Color32::from_rgb(56, 189, 248)));

            // Draw Earth's yellow latitude/longitude grid
            let grid_color = egui::Color32::from_rgba_unmultiplied(253, 224, 71, 100); // Yellow grid lines
            let grid_stroke = egui::Stroke::new(1.0, grid_color);
            let r_earth = self.config.env.r_earth;

            // Parallels (latitude lines)
            for lat_deg in (-60..=60).step_by(20) {
                let lat_rad = (lat_deg as f64).to_radians();
                let z = r_earth * lat_rad.sin();
                let r_lat = r_earth * lat_rad.cos();
                
                let mut prev_pt: Option<egui::Pos2> = None;
                let steps = 72;
                for step in 0..=steps {
                    let lon_rad = (step as f64 * 360.0 / steps as f64).to_radians() + gst;
                    let x = r_lat * lon_rad.cos();
                    let y = r_lat * lon_rad.sin();
                    
                    let (screen_pos, rot_z) = project_3d([x, y, z]);
                    if rot_z > 0.0 {
                        if let Some(prev) = prev_pt {
                            painter.line_segment([prev, screen_pos], grid_stroke);
                        }
                        prev_pt = Some(screen_pos);
                    } else {
                        prev_pt = None;
                    }
                }
            }

            // Meridians (longitude lines)
            for lon_deg in (0..360).step_by(30) {
                let lon_rad = (lon_deg as f64).to_radians() + gst;
                
                let mut prev_pt: Option<egui::Pos2> = None;
                let steps = 72;
                for step in -steps/2..=steps/2 {
                    let lat_rad = (step as f64 * 90.0 / (steps as f64 / 2.0)).to_radians();
                    let x = r_earth * lat_rad.cos() * lon_rad.cos();
                    let y = r_earth * lat_rad.cos() * lon_rad.sin();
                    let z = r_earth * lat_rad.sin();
                    
                    let (screen_pos, rot_z) = project_3d([x, y, z]);
                    if rot_z > 0.0 {
                        if let Some(prev) = prev_pt {
                            painter.line_segment([prev, screen_pos], grid_stroke);
                        }
                        prev_pt = Some(screen_pos);
                    } else {
                        prev_pt = None;
                    }
                }
            }

            // Draw Earth's rotation axis
            let axis_len = r_earth * 1.25;
            let (axis_north_px, north_z) = project_3d([0.0, 0.0, axis_len]);
            let (axis_south_px, south_z) = project_3d([0.0, 0.0, -axis_len]);
            painter.line_segment(
                [axis_south_px, axis_north_px],
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(100, 116, 139, 100))
            );
            
            if north_z > 0.0 {
                painter.text(
                    axis_north_px,
                    egui::Align2::CENTER_CENTER,
                    "N",
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_rgb(56, 189, 248)
                );
            }
            if south_z > 0.0 {
                painter.text(
                    axis_south_px,
                    egui::Align2::CENTER_CENTER,
                    "S",
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_rgb(239, 68, 68)
                );
            }

            // Draw Orbit paths dynamically from satellite parameters
            let draw_orbit_from_sat = |painter: &egui::Painter, sat: &Satellite, color: egui::Color32| {
                let r = sat.r;
                let v = sat.v;
                let r_mag = (r[0]*r[0] + r[1]*r[1] + r[2]*r[2]).sqrt();
                let h_x = r[1]*v[2] - r[2]*v[1];
                let h_y = r[2]*v[0] - r[0]*v[2];
                let h_z = r[0]*v[1] - r[1]*v[0];
                let h_mag = (h_x*h_x + h_y*h_y + h_z*h_z).sqrt();
                if h_mag < 1e-9 {
                    return;
                }
                let c_i = h_z / h_mag;
                let s_i = (1.0 - c_i*c_i).max(0.0).sqrt();
                
                // Node vector: n = k x h = [-h_y, h_x, 0]
                let n_x = -h_y;
                let n_y = h_x;
                let n_mag = (n_x*n_x + n_y*n_y).sqrt();
                let (c_r, s_r) = if n_mag > 1e-9 {
                    (n_x / n_mag, n_y / n_mag)
                } else {
                    (1.0, 0.0)
                };
                
                let mut prev_pt: Option<egui::Pos2> = None;
                let steps = 120;
                for step in 0..=steps {
                    let theta = (step as f64 * 360.0 / steps as f64).to_radians();
                    let r_plane = [r_mag * theta.cos(), r_mag * theta.sin(), 0.0];
                    let r_eci = [
                        c_r * r_plane[0] - s_r * c_i * r_plane[1],
                        s_r * r_plane[0] + c_r * c_i * r_plane[1],
                        s_i * r_plane[1]
                    ];
                    let (screen_pos, rot_z) = project_3d(r_eci);
                    
                    let dist = screen_pos.distance(center);
                    let occluded = rot_z < 0.0 && dist < earth_radius_px;
                    
                    let stroke_color = if occluded {
                        color.linear_multiply(0.12)
                    } else {
                        color.linear_multiply(0.4)
                    };
                    
                    if let Some(prev) = prev_pt {
                        painter.line_segment([prev, screen_pos], egui::Stroke::new(1.0, stroke_color));
                    }
                    prev_pt = Some(screen_pos);
                }
            };

            // 1. Draw orbits of all segments using their first satellite
            for (seg_idx, seg) in self.constellation.segments.iter().enumerate() {
                let color = match seg_idx {
                    0 => egui::Color32::from_rgb(56, 189, 248),  // LEO Blue
                    1 => egui::Color32::from_rgb(192, 132, 252), // MEO Purple
                    2 => egui::Color32::from_rgb(251, 146, 60),  // GEO Orange
                    _ => {
                        if let Some(first_sat) = seg.satellites.first() {
                            if let Some(color_rgb) = first_sat.custom_color {
                                egui::Color32::from_rgb(color_rgb[0], color_rgb[1], color_rgb[2])
                            } else {
                                STATUS_GOOD
                            }
                        } else {
                            STATUS_GOOD
                        }
                    }
                };
                if let Some(first_sat) = seg.satellites.first() {
                    draw_orbit_from_sat(&painter, first_sat, color);
                }
            }

            // 2. Draw orbits of individual custom satellites in LEO/MEO/GEO segments
            for seg_idx in 0..3 {
                if seg_idx < self.constellation.segments.len() {
                    for sat in &self.constellation.segments[seg_idx].satellites {
                        if sat.is_custom {
                            let color = if let Some(color_rgb) = sat.custom_color {
                                egui::Color32::from_rgb(color_rgb[0], color_rgb[1], color_rgb[2])
                            } else {
                                STATUS_GOOD
                            };
                            draw_orbit_from_sat(&painter, sat, color);
                        }
                    }
                }
            }

            // Gather all active node screen positions
            let mut satellites_screen = Vec::new();
            for seg in &self.constellation.segments {
                for sat in &seg.satellites {
                    let (sat_pos_px, rot_z) = project_3d(sat.r);
                    satellites_screen.push((sat.id.clone(), sat.orbit_type.clone(), sat_pos_px, sat.r, rot_z, sat.is_custom, sat.custom_color));
                }
            }

            let mut stations_screen = Vec::new();
            for (gs_idx, gs) in self.ground_stations.iter().enumerate() {
                let gs_eci = gs_eci_list[gs_idx];
                let (gs_pos_px, rot_z) = project_3d(gs_eci);
                stations_screen.push((gs.id.clone(), gs_pos_px, gs_eci, gs.k_value, rot_z));
            }

            // Laser links must vanish where they pass behind the planet: sample
            // the 3D chord and keep only the sub-segments whose sampled points
            // are in front of (or beside) the Earth disc.
            let lerp3 = |a: [f64; 3], b: [f64; 3], t: f64| -> [f64; 3] {
                [
                    a[0] + (b[0] - a[0]) * t,
                    a[1] + (b[1] - a[1]) * t,
                    a[2] + (b[2] - a[2]) * t,
                ]
            };
            let visible_link_segments = |a: [f64; 3], b: [f64; 3]| -> Vec<[egui::Pos2; 2]> {
                let steps = 24;
                let mut segments = Vec::new();
                let mut prev: Option<(egui::Pos2, bool)> = None;
                for k in 0..=steps {
                    let p = lerp3(a, b, k as f64 / steps as f64);
                    let (px, rot_z) = project_3d(p);
                    let hidden = rot_z < 0.0 && px.distance(center) < earth_radius_px;
                    if let Some((prev_px, prev_hidden)) = prev {
                        if !prev_hidden && !hidden {
                            segments.push([prev_px, px]);
                        }
                    }
                    prev = Some((px, hidden));
                }
                segments
            };

            // Draw active links between Satellites (ISL) using pre-calculated active_isls
            for &(i, j, capacity) in &active_isls {
                if i >= all_sats.len() || j >= all_sats.len() {
                    continue;
                }
                let (id1, _, _) = &all_sats[i];
                let (id2, _, _) = &all_sats[j];

                let pos1 = satellites_screen.iter().find(|(id, _, _, _, _, _, _)| id == id1);
                let pos2 = satellites_screen.iter().find(|(id, _, _, _, _, _, _)| id == id2);

                if let (Some((_, _, _, r1, _, _, _)), Some((_, _, _, r2, _, _, _))) = (pos1, pos2) {
                    let color = if capacity > 5.0 {
                        STATUS_GOOD
                    } else if capacity > 1.0 {
                        STATUS_WARN
                    } else {
                        STATUS_BAD
                    };

                    let link_stroke = egui::Stroke::new(1.0, color.linear_multiply(0.4));

                    // Critical links (< 1 Gbps) are dashed: ISL lines carry no
                    // Gbps label, so line style is the non-color capacity cue.
                    for seg in visible_link_segments(*r1, *r2) {
                        if capacity > 1.0 {
                            painter.line_segment(seg, link_stroke);
                        } else {
                            painter.extend(egui::Shape::dashed_line(&seg, link_stroke, 6.0, 4.0));
                        }
                    }

                    // Animated signal traveling along the link, hidden while
                    // it passes behind the planet
                    let pulse_t = (self.current_time * 2.0) % 1.0;
                    let p = lerp3(*r1, *r2, pulse_t);
                    let (ppx, prot) = project_3d(p);
                    if !(prot < 0.0 && ppx.distance(center) < earth_radius_px) {
                        painter.circle_filled(ppx, 2.0, color);
                    }
                }
            }

            // Draw active laser links between Satellites and their best Ground Station (SGL)
            if self.show_sgl {
                for (_sat_id, _type, _, sat_r, _, _, _) in &satellites_screen {
                    if self.prioritize_relay && _type == &OrbitType::LEO {
                        continue;
                    }
                    let sat_max_speed = match _type {
                        OrbitType::LEO => self.leo_max_bitrate,
                        OrbitType::MEO => self.meo_max_bitrate,
                        OrbitType::GEO => self.geo_max_bitrate,
                    };

                    let mut best_gs: Option<String> = None;
                    let mut max_capacity = 0.0;
                    let mut best_gs_r = [0.0f64; 3];
                    let sat_ref_dist = match _type {
                        OrbitType::LEO => self.config.ref_dist_sgl_km,
                        OrbitType::MEO => self.config.meo_alt_km,
                        OrbitType::GEO => self.config.geo_alt_km,
                    };

                    for (gs_id, _, gs_r, gs_k, _) in &stations_screen {
                        let capacity = compute_link_capacity(
                            *sat_r, *gs_r, true, *gs_k,
                            sat_ref_dist, sat_max_speed, &self.config.env
                        ).min(sat_max_speed);

                        if capacity > max_capacity {
                            max_capacity = capacity;
                            best_gs = Some(gs_id.clone());
                            best_gs_r = *gs_r;
                        }
                    }

                    if best_gs.is_some() && max_capacity > 0.0 {
                        let (beam_r, beam_g, beam_b) = if max_capacity > (sat_max_speed * 0.5) {
                            BEAM_GOOD
                        } else if max_capacity > (sat_max_speed * 0.1) {
                            BEAM_WARN
                        } else {
                            BEAM_BAD
                        };

                        let base_color = egui::Color32::from_rgb(beam_r, beam_g, beam_b);

                        // Only the portions of the beam in front of the planet
                        // are drawn; anything behind the Earth disc is skipped.
                        for seg in visible_link_segments(*sat_r, best_gs_r) {
                            // Outer glow
                            painter.line_segment(
                                seg,
                                egui::Stroke::new(5.0, egui::Color32::from_rgba_unmultiplied(beam_r, beam_g, beam_b, 25))
                            );
                            // Mid glow
                            painter.line_segment(
                                seg,
                                egui::Stroke::new(2.5, egui::Color32::from_rgba_unmultiplied(beam_r, beam_g, beam_b, 60))
                            );
                            // Core laser line
                            painter.line_segment(seg, egui::Stroke::new(1.0, base_color));
                        }

                        // Animated signals traveling along active SGL links,
                        // hidden while they pass behind the planet
                        let pulse_t = (self.current_time * 2.5) % 1.0;
                        for p_idx in 0..2 {
                            let progress = (pulse_t + p_idx as f64 * 0.5) % 1.0;
                            let p = lerp3(*sat_r, best_gs_r, progress);
                            let (ppx, prot) = project_3d(p);
                            if !(prot < 0.0 && ppx.distance(center) < earth_radius_px) {
                                painter.circle_filled(ppx, 2.5, base_color);
                            }
                        }

                        // Speed label at the link midpoint, only when visible
                        let (mid_px, mid_rot) = project_3d(lerp3(*sat_r, best_gs_r, 0.5));
                        if !(mid_rot < 0.0 && mid_px.distance(center) < earth_radius_px) {
                            let label = format!("{:.1} Gbps", max_capacity);
                            painter.text(
                                egui::pos2(mid_px.x + 5.0, mid_px.y - 6.0),
                                egui::Align2::LEFT_BOTTOM,
                                &label,
                                egui::FontId::proportional(9.0),
                                base_color,
                            );
                        }
                    }
                }
            }

            // Draw Ground Stations
            for (gs_id, gs_pos_px, _gs_r, gs_k, rot_z) in &stations_screen {
                if *rot_z <= 0.0 {
                    continue; // behind Earth
                }
                let color = if *gs_k < 0.1 / 1000.0 {
                    STATUS_GOOD
                } else if *gs_k < 1.0 / 1000.0 {
                    STATUS_WARN
                } else {
                    STATUS_BAD
                };
                
                painter.rect_filled(
                    egui::Rect::from_center_size(*gs_pos_px, egui::vec2(8.0, 8.0)),
                    1.0,
                    color
                );
                
                painter.text(
                    egui::pos2(gs_pos_px.x + 8.0, gs_pos_px.y - 4.0),
                    egui::Align2::LEFT_TOP,
                    gs_id,
                    egui::FontId::proportional(10.0),
                    egui::Color32::LIGHT_GRAY
                );
            }

            for (sat_id, _type, sat_pos_px, _r, rot_z, is_custom, custom_color) in &satellites_screen {
                let color = if let (true, Some([r, g, b])) = (*is_custom, custom_color) {
                    egui::Color32::from_rgb(*r, *g, *b)
                } else if *is_custom {
                    match _type {
                        OrbitType::LEO => egui::Color32::from_rgb(45, 212, 191),
                        OrbitType::MEO => egui::Color32::from_rgb(232, 121, 249),
                        OrbitType::GEO => egui::Color32::from_rgb(248, 113, 113),
                    }
                } else {
                    match _type {
                        OrbitType::LEO => egui::Color32::from_rgb(56, 189, 248),
                        OrbitType::MEO => egui::Color32::from_rgb(192, 132, 252),
                        OrbitType::GEO => egui::Color32::from_rgb(251, 146, 60),
                    }
                };

                let is_selected = *sat_id == self.selected_satellite_id;
                let size = if is_selected { 6.0 } else { 4.0 };
                
                // Occlusion check
                let dist_from_center = sat_pos_px.distance(center);
                let occluded = *rot_z < 0.0 && dist_from_center < earth_radius_px;
                
                let alpha = if occluded { 40 } else { 255 };
                let color_with_alpha = color.linear_multiply(alpha as f32 / 255.0);

                if is_selected {
                    let ring_alpha = if occluded { 60 } else { 255 };
                    painter.circle_stroke(
                        *sat_pos_px,
                        size + 3.0,
                        egui::Stroke::new(1.5, egui::Color32::from_rgb(250, 204, 21).linear_multiply(ring_alpha as f32 / 255.0))
                    );
                }

                painter.circle_filled(*sat_pos_px, size, color_with_alpha);

                if is_selected || satellites_screen.len() <= 20 {
                    let text_color = if is_selected {
                        egui::Color32::from_rgb(250, 204, 21).linear_multiply(alpha as f32 / 255.0)
                    } else {
                        egui::Color32::WHITE.linear_multiply(alpha as f32 / 255.0)
                    };
                    painter.text(
                        egui::pos2(sat_pos_px.x + size + 2.0, sat_pos_px.y - 4.0),
                        egui::Align2::LEFT_TOP,
                        sat_id,
                        egui::FontId::proportional(10.0),
                        text_color
                    );
                }
            }
        });

        // Apply deferred mutations to avoid index mismatches during UI drawing
        if let Some(idx) = pending_remove {
            let name = self.ground_stations[idx].name.clone();
            self.ground_stations.remove(idx);
            if idx < self.weather_overrides.len() {
                self.weather_overrides.remove(idx);
            }
            if idx < self.history_stations.len() {
                self.history_stations.remove(idx);
            }
            self.log(&format!("Rimossa stazione {}", name));
        }
        if pending_add {
            let new_id = format!("GS_{}", self.ground_stations.len() + 1);
            let new_name = format!("Station {}", self.ground_stations.len() + 1);
            self.ground_stations.push(GroundStation {
                id: new_id.clone(),
                name: new_name.clone(),
                lat_rad: 0.0,
                lon_rad: 0.0,
                alt_m: 100.0,
                downlink_nominal_gbps: f64::INFINITY,
                atmos_state: 0,
                k_value: self.config.atmos_k[0] / 1000.0,
            });
            self.weather_overrides.push(Some(0));
            self.history_stations.push(vec![0.0f32; self.history_time.len()]);
            self.log(&format!("Aggiunta stazione {}", new_name));
        }

        // Check for constellation changes to apply configuration dynamically (runs on both mobile and desktop)
        let changed = self.config.leo_num != self.leo_num_input
            || self.config.leo_alt_km != self.leo_alt_input
            || self.config.leo_inc_deg != self.leo_inc_input
            || self.config.meo_num != self.meo_num_input
            || self.config.meo_alt_km != self.meo_alt_input
            || self.config.meo_inc_deg != self.meo_inc_input
            || self.config.geo_num != self.geo_num_input
            || self.config.geo_alt_km != self.geo_alt_input
            || self.config.geo_inc_deg != self.geo_inc_input;

        if changed {
            self.config.leo_num = self.leo_num_input;
            self.config.leo_alt_km = self.leo_alt_input;
            self.config.leo_inc_deg = self.leo_inc_input;
            self.config.meo_num = self.meo_num_input;
            self.config.meo_alt_km = self.meo_alt_input;
            self.config.meo_inc_deg = self.meo_inc_input;
            self.config.geo_num = self.geo_num_input;
            self.config.geo_alt_km = self.geo_alt_input;
            self.config.geo_inc_deg = self.geo_inc_input;

            let custom_segments: Vec<Segment> = if self.constellation.segments.len() > 3 {
                self.constellation.segments[3..].to_vec()
            } else {
                Vec::new()
            };

            let custom_leo: Vec<Satellite> = self.constellation.segments[0].satellites.iter()
                .filter(|sat| sat.is_custom)
                .cloned()
                .collect();
            let custom_meo: Vec<Satellite> = self.constellation.segments[1].satellites.iter()
                .filter(|sat| sat.is_custom)
                .cloned()
                .collect();
            let custom_geo: Vec<Satellite> = self.constellation.segments[2].satellites.iter()
                .filter(|sat| sat.is_custom)
                .cloned()
                .collect();

            self.constellation = create_satellites_from_config(&self.config);

            let insert_custom_avoiding_clash = |seg_idx: usize, custom_sats: Vec<Satellite>, segments: &mut Vec<Segment>| {
                for mut sat in custom_sats {
                    let mut final_id = sat.id.clone();
                    let mut sat_idx_counter = segments[seg_idx].satellites.len();
                    loop {
                        let mut clash = false;
                        for s in &segments[seg_idx].satellites {
                            if s.id == final_id {
                                clash = true;
                                break;
                            }
                        }
                        if !clash {
                            break;
                        }
                        final_id = format!("{:?}_{:02}", sat.orbit_type, sat_idx_counter);
                        sat_idx_counter += 1;
                    }
                    sat.id = final_id;
                    segments[seg_idx].satellites.push(sat);
                }
            };

            let segments_mut = &mut self.constellation.segments;
            insert_custom_avoiding_clash(0, custom_leo, segments_mut);
            insert_custom_avoiding_clash(1, custom_meo, segments_mut);
            insert_custom_avoiding_clash(2, custom_geo, segments_mut);

            self.constellation.segments.extend(custom_segments);

            let mut found_any = false;
            for seg in &self.constellation.segments {
                if !seg.satellites.is_empty() {
                    self.selected_satellite_id = seg.satellites[0].id.clone();
                    found_any = true;
                    break;
                }
            }
            if !found_any {
                self.selected_satellite_id = "None".to_string();
            }
            self.update_input_fields_for_selected();
            self.log("Constellation reconfigured dynamically");
        }

        if pending_reset {
            self.current_time = 0.0;
            self.is_running = true;
            self.time_warp = 1;
            self.selected_satellite_id = "LEO_00".to_string();
            self.dragging_satellite_id = None;
            self.constellation = create_satellites_from_config(&self.config);
            self.ground_stations = self.config.stations.clone();
            self.weather_overrides = vec![Some(0); self.ground_stations.len()];
            self.history_time.clear();
            self.history_stations = vec![Vec::new(); self.ground_stations.len()];
            self.history_total.clear();
            self.map_zoom = 1.0;
            self.leo_max_bitrate = 100.0;
            self.meo_max_bitrate = 400.0;
            self.geo_max_bitrate = 800.0;
            self.log("Simulation State Reset to initial values");
        }
    }
}

pub fn default_config() -> Config {
    Config {
        name: "HydRON-Like-Net".to_string(),
        leo_num: 10,
        leo_alt_km: 550.0,
        leo_inc_deg: 97.6,
        leo_mass: 20.0,
        leo_area: 0.1,
        leo_cd: 2.2,
        leo_cr: 1.2,
        meo_num: 4,
        meo_alt_km: 10000.0,
        meo_inc_deg: 55.0,
        meo_raans: vec![0.0, 90.0, 180.0, 270.0],
        meo_mass: 50.0,
        meo_area: 0.25,
        meo_cd: 0.0,
        meo_cr: 1.2,
        geo_num: 3,
        geo_lons: vec![0.0, 60.0, -120.0],
        geo_alt_km: 35786.0,
        geo_inc_deg: 0.0,
        geo_mass: 200.0,
        geo_area: 1.5,
        geo_cd: 0.0,
        geo_cr: 1.2,
        stations: vec![
            GroundStation { id: "GS_SVA".to_string(), name: "Svalbard".to_string(), lat_rad: 78.2307f64.to_radians(), lon_rad: 15.6472f64.to_radians(), alt_m: 130.0, downlink_nominal_gbps: f64::INFINITY, atmos_state: 0, k_value: 0.05 / 1000.0 },
            GroundStation { id: "GS_ZRH".to_string(), name: "Zurich".to_string(), lat_rad: 47.4647f64.to_radians(), lon_rad:  8.5492f64.to_radians(), alt_m: 400.0, downlink_nominal_gbps: f64::INFINITY, atmos_state: 0, k_value: 0.05 / 1000.0 },
            GroundStation { id: "GS_REU".to_string(), name: "Reunion".to_string(), lat_rad: -20.9089f64.to_radians(), lon_rad: 55.5136f64.to_radians(), alt_m: 95.0, downlink_nominal_gbps: f64::INFINITY, atmos_state: 0, k_value: 0.05 / 1000.0 },
            GroundStation { id: "GS_MAU".to_string(), name: "Maui".to_string(), lat_rad: 20.7067f64.to_radians(), lon_rad: -156.257f64.to_radians(), alt_m: 100.0, downlink_nominal_gbps: f64::INFINITY, atmos_state: 0, k_value: 0.05 / 1000.0 },
        ],
        atmos_states: vec!["clear".to_string(), "thin".to_string(), "thick".to_string(), "heavy".to_string()],
        atmos_k: vec![0.05, 0.2, 1.5, 5.0],
        transition_matrix: vec![
            vec![0.85, 0.10, 0.04, 0.01],
            vec![0.15, 0.70, 0.10, 0.05],
            vec![0.05, 0.15, 0.65, 0.15],
            vec![0.02, 0.08, 0.20, 0.70],
        ],
        env: SimEnvironment {
            mu: 3.986004418e14,
            r_earth: 6378137.0,
            j2: 1.08262668e-3,
            rho0_500km: 3.8e-12,
            h0_km: 500.0,
            scale_height_km: 70.0,
            p_srp: 4.56e-6,
        },
        dt_time_step: 1.0,
        ref_dist_isl_km: 1000.0,
        ref_dist_sgl_km: 1000.0,
    }
}

// Versione Desktop (Nativa)
