# HydRON Constellation Digital Twin & GUI Monitor

Welcome to the **HydRON Digital Twin (DT) Builder and GUI Monitor**, an interactive simulation environment designed for real-time visualization, configuration, and analysis of multi-layer satellite constellations (LEO, MEO, GEO) and their ground communications network, inspired by the [ESA HydRON Project (High Throughput Optical Network)](https://resilience.esa.int/archives/partnership-projects/hydron).

Developed in Rust using the `egui` immediate-mode GUI framework, this project implements high-fidelity orbital mechanics, attitude control systems (ADCS), atmospheric attenuation models, and dynamic network routing simulation.

🚀🎮 **Try the Live Web Demo**: [alcio313.github.io/hydronTwin](https://alcio313.github.io/hydronTwin/)

---

## 📶 Key Features

### 1. Tabbed Ribbon Toolbar & Interactive HUDs
* **Tabbed Ribbon Interface**: Reorganizes all controls into a top horizontal ribbon toolbar with tabs: *Simulation*, *Constellation*, *Network & Bitrate*, *ADCS & Sensors*, and *Weather & Stations*. This clean structure maximizes the screen space for 3D visualizations.
* **Transparent HUD Floating Windows**: Draggable, resizable, and toggleable overlay windows displaying live telemetry, ground station capacities/connections, all-satellite and ground station bitrates, and system console logs.
* **Textured 3D Globe**: Renders a sphere representing Earth using `earth.jpg` coordinates, projected dynamically based on Greenwich Sidereal Time (GST) to align with inertial coordinates (ECI to ECEF).
* **Multi-Layer Constellation Rendering**: Visualizes circular orbits and positions for LEO, MEO, and GEO segments with configurable visual filters.
* **Camera Controls**: Zoom with the mouse wheel; rotate the globe by clicking and dragging on empty space.
* **Direct Satellite Dragging**: Click and drag any visible satellite directly on the screen to slide *only* the selected satellite along its orbit plane, preserving its nominal altitude and physical velocity.

### 2. Network Link Capacity & Routing Simulation
* **Ground-to-Satellite Links (SGL)**: Simulates atmospheric attenuation on laser links between satellites and ground stations using an exponential atmospheric model and slant-path angles.
* **Inter-Satellite Links (ISL)**: Simulates laser links between adjacent satellites.
* **Laser Link Routing Rule**: Enforces that the only active laser links permitted are those pointing directly to the ground (SGL) or those connecting a satellite to a ground-connected satellite relay (meaning at least one of the endpoints in an ISL must have an active SGL connection). Additionally, any GEO satellite involved in an ISL link must itself have a direct active connection to a ground station (SGL).
* **Dynamic Relay Bottleneck & Handoff**: The capacity of an ISL link is capped by the active SGL ground connection capacity of its relay satellite. If the relay's ground connection speed degrades (e.g., due to atmospheric weather degradation at its ground station), the bottleneck triggers a dynamic handoff, allowing satellites to switch to a faster ground-connected relay.
* **LEO Satellite Laser Terminal Budget**: LEO satellites are restricted to at most 1 active laser connection at any given time (either a single SGL link to ground OR a single ISL link to another satellite).
* **LEO Connection Path Optimization**: LEO satellites dynamically select the fastest overall path to ground (either direct SGL or via a MEO/GEO relay) by comparing all SGL and ISL capacities in a single unified greedy optimization. If *Relay Only* routing is enabled, LEO satellites bypass direct SGL paths and route exclusively via relays.
* **LEO Capacity Overrides**: Inter-satellite links involving at least one LEO satellite operate at a dynamically configured, stable capacity (bypassing free-space path loss attenuation) to simulate advanced laser terminals.
* **Real-Time Telemetry HUD Windows**:
  * **Satellite Telemetry HUD**: Draggable window displaying ECI orbit positions, attitude quaternions, angular velocities, physical properties, and live link geometry (azimuth, elevation, distance) for active connections.
  * **Ground Stations HUD**: Floating window showing real-time throughput, nominal capacity (supporting unlimited), and active links including the azimuth, elevation, and distance to connected satellites.
  * **Bitrates HUD** (formerly LEO Bitrate Channels HUD): Floating window displaying status and live speed values for all LEO/MEO/GEO satellites and Ground Stations (color-coded by throughput).
  * **System Console Logs HUD**: Floating system logs showing routing notifications.
  * **Ground Station Aggregate Throughput**: Live graphs showing station-by-station and total network aggregate data rates.

### 3. Simulation & Time Control
* **Play / Pause**: Toggle real-time propagation.
* **Time Warp Slider**: Accelerate or decelerate simulation time dynamically (from -50x to +50x).
* **System Reset**: Restore the simulation and constellations to initial values specified in `config.toml`.

### 4. Noise & Disturbance Injection (ADCS)
* **Active Attitude Kinematics**: Simulates reaction wheels and magnetorquers stabilizing the satellites.
* **Disturbance Injector**: Inject a 3-axis torque disturbance vector ($T_x, T_y, T_z$) to observe how the ADCS algorithm stabilizes the satellite bus.
* **Sensor Noise Configurations**: Configure noise levels for Gyro, Magnetometer, Sun Sensor, and Star Tracker dynamically.

### 5. 24h CSV Exporter
* Run a full 24-hour simulation sequence using the current configuration and export the results to a CSV file detailing ground station throughputs, link counts, and overall network data rate.

---

## 🛠 Architectural & Mathematical Modeling

### 1. Orbital Mechanics
Satellite orbits are propagated using a **Runge-Kutta 4th-order (RK4)** numerical integrator. The acceleration model incorporates:
* **Two-Body Gravity**: Standard Newtonian gravity around Earth ($\mu$).
* **J2 Oblateness Perturbation**: Accurately models the Earth's non-spherical mass distribution.
* **Atmospheric Drag**: Applied to LEO and lower MEO satellites using an exponential atmospheric density model ($\rho(h)$) and drag coefficient $C_d$.
* **Solar Radiation Pressure (SRP)**: Solar pressure model based on the sun vector and reflectivity coefficient $C_r$.

### 2. Spacecraft Attitude Dynamics & ADCS
Attitude is represented using quaternions $q = [\eta, \epsilon_1, \epsilon_2, \epsilon_3]$ to avoid gimbal lock:
* **Kinematics**: Rotational kinematics integrated via quaternion updates.
* **Stabilization**: Employs reaction wheel torques ($T_{rw}$) and magnetorquer control dipole commands ($m_{mtq}$) interacting with Earth's magnetic field ($B$).

### 3. Laser Link Capacity
Networking bandwidth uses a custom range-based capacity model:
$$C = C_{max} \cdot \left(\frac{d_{ref}}{d}\right)^2 \cdot \alpha_{atmos}$$
Where:
* $C_{max}$ is the dynamic satellite maximum capacity configured in the GUI.
* $d_{ref}$ is the reference link distance.
* $d$ is the actual distance between nodes.
* $\alpha_{atmos}$ is the atmospheric attenuation coefficient (only for SGL, based on local station weather states and slant path length).

---

## 🚀 Getting Started

### Prerequisites
* Rust compiler (MSRV 1.92+, required by egui 0.35)
* Cargo package manager
* **For Web version**: [Trunk](https://trunkrs.dev/) installed (`cargo install trunk`) and the WebAssembly target (`rustup target add wasm32-unknown-unknown`)

### Building and Running

#### 🖥️ Desktop (Native Application)
1. **Clone the repository**:
   ```bash
   git clone https://github.com/alcio313/hydronTwin.git
   cd hydronTwin
   ```
2. **Build and Run**:
   ```bash
   cargo run --release
   ```
   *Make sure `earth.jpg` and `config.toml` (optional) are in the working directory.*

#### 🌐 Web Browser (WebAssembly)
1. **Install prerequisites** (if not already installed):
   ```bash
   cargo install trunk
   rustup target add wasm32-unknown-unknown
   ```
2. **Serve locally**:
   ```bash
   trunk serve
   ```
3. **Open in browser**:
   Navigate to `http://localhost:8080` in your web browser.
4. **Build release static assets**:
   ```bash
   trunk build --release
   ```

   The compiled static website (HTML, JS, WASM) will be generated inside the `dist/` directory, ready to be deployed to GitHub Pages, Vercel, Netlify, or any static server.


---

## ⚙ Configuration (`config.toml`)

The application loads its default parameters from a `config.toml` file in the root directory. You can also import and export custom configuration files dynamically directly from the GUI. The configuration files allow you to configure:
* **Constellations**: Number of satellites, nominal altitudes, orbital inclinations, RAANs, and satellite mass/areas.
* **Ground Stations**: Geographical coordinates (latitude, longitude, altitude) and downlink capacity limits (which can be set to numerical values in Gbps or `"unlimited"`/`"inf"`/`"infinity"` to represent unlimited capacity).
* **Atmosphere**: Transition matrices for Markov weather state models and laser extinction values.
* **Environment Constants**: Earth gravity parameters, J2 coefficient, SRP constants, and atmospheric scale heights.

---

## 🎮 Interactive Controls Guide

### Left Panel (Configuration & Limits)
* **⚙ Visual Filters**: Checkboxes to toggle LEO ISL, MEO ISL, GEO ISL, or Ground Links (SGL) on/off. Includes a logarithmic map zoom slider.
* **📁 CONFIGURATION**: Allows loading/saving custom TOML configurations. On Desktop, it uses native file dialog pickers. On Web, drag & drop a TOML file anywhere on the browser window to import it, and click "Export" to download the current configuration directly to your downloads folder.
* **🛰 LEO Routing Priority**: Toggle between Ground First (SGL) and Relay Only (ISL) to prioritize routing satellite data through MEO/GEO relays instead of direct SGL paths.
* **📶 Bitrate Massimo Satelliti**: Dynamically adjust the peak bitrate capacity (Gbps) for LEO, MEO, and GEO satellites. Changes take effect instantly across all simulation calculations and the CSV exporter.
* **📡 Modifica Costellazione**: Change constellation sizes, altitudes, and inclinations on the fly.
* **🏠 Stazioni di Terra**: Add new ground stations or manually override local weather states (e.g., Clear Sky, Light Rain, Heavy Rain, Storm) to observe SGL link degradation.

### Central Panel (3D Map & Plot)
* **3D Visualizer**:
  * Drag empty space to rotate the Earth.
  * Use mouse scroll to zoom in/out.
  * **Drag Satellites**: Click directly on a satellite and drag it to rotate *only* the selected satellite along its orbit plane.
* **📊 Station Throughput Plot**: Live graph of ground station and total network aggregate data rates.

### Right Panel (Telemetry & Console)
* **📶 Bitrates**: Monitor live throughput for all LEO/MEO/GEO satellites and Ground Stations (color-coded by active throughput). Click a satellite's name in the list to select it.
* **Satellite Telemetry**: Read exact ECI position/velocity coordinates, attitude quaternions, ADCS actuator states, and detailed link geometry (azimuth, elevation, distance) for active connections.
* **Iniettore Disturbi ADCS**: Inject 3D torques to test stabilization.
* **Rumore Sensori**: Slide values to increase sensor noise, introducing jitter to the stabilization algorithm.
* **System Logs**: Live event feed tracking connections, disconnections, and export triggers.

---

## 🤖 Development & Credits
This project was developed with the **Gemini AI Coding Agent** (Google DeepMind's Advanced Agentic Coding system, *Antigravity*).
