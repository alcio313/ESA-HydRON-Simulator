# Guida alla Configurazione di `config.toml` per HydRON Digital Twin

Questo documento descrive la struttura del file di configurazione `config.toml` utilizzato dal software **HydRON Constellation Digital Twin & GUI Monitor** per inizializzare e importare scenari di simulazione.

---

## Caratteristiche del Parser TOML del Software

Il software utilizza un **parser TOML personalizzato ("hand-rolled")** scritto in Rust per evitare dipendenze esterne. Di conseguenza, il file deve seguire regole sintattiche rigide e supporta solo specifiche sezioni e chiavi. Qualsiasi elemento non riconosciuto o non mappato all'interno del codice sorgente viene ignorato senza generare errori fatali (viene caricato il valore predefinito).

### Regole Sintattiche Importanti:
1. **Commenti**: Le righe che iniziano con `#` vengono ignorate.
2. **Spazi**: Gli spazi intorno a `=` e ai valori vengono ignorati.
3. **Stringhe**: I valori di tipo stringa possono contenere doppie virgolette (`"valore"`), che vengono rimosse automaticamente dal parser durante il caricamento.
4. **Array**: Gli array devono essere racchiusi tra parentesi quadre `[...]` e i valori interni devono essere separati da virgole (es. `[0.0, 90.0, 180.0]`).
5. **Sezioni Multi-istanza (Stazioni di Terra)**: Ogni stazione di terra deve iniziare con l'intestazione di sezione `[[ground.stations]]`. Il parser rileva questa intestazione per istanziare una nuova stazione e salvare in memoria la precedente.

---

## Sezioni e Parametri Supportati

### 1. `[constellation]`
Inizializza le informazioni generali sulla costellazione.
* **`name`**: *(Stringa)* Il nome identificativo della costellazione.

```toml
[constellation]
name = "HydRON-Like-Net"
```

### 2. `[constellation.leo]`
Definisce i parametri fisici e orbitali dello strato orbitale LEO (Low Earth Orbit).
I satelliti LEO vengono generati e posizionati automaticamente in modo uniforme in un singolo piano orbitale circolare.
* **`num_satellites`**: *(Intero)* Numero di satelliti da generare.
* **`altitude_km`**: *(Decimale)* Altitudine dell'orbita in chilometri.
* **`inclination_deg`**: *(Decimale)* Inclinazione dell'orbita in gradi.
* **`mass_kg`**: *(Decimale)* Massa del singolo satellite in kg (usata per il calcolo delle perturbazioni orbitali).
* **`cross_section_area_m2`**: *(Decimale)* Area della sezione trasversale del satellite in m² (usata per la resistenza atmosferica e la pressione di radiazione solare).
* **`cd`**: *(Decimale)* Coefficiente di resistenza aerodinamica (*drag coefficient*).
* **`cr`**: *(Decimale)* Coefficiente di riflettività (*radiation pressure coefficient*).

```toml
[constellation.leo]
num_satellites = 10
altitude_km = 550.0
inclination_deg = 97.6
mass_kg = 20.0
cross_section_area_m2 = 0.1000
cd = 2.20
cr = 1.20
```

### 3. `[constellation.meo]`
Definisce i parametri dello strato orbitale MEO (Medium Earth Orbit).
I satelliti MEO vengono posizionati in un piano orbitale.
* **`num_satellites`**: *(Intero)* Numero di satelliti.
* **`altitude_km`**: *(Decimale)* Altitudine dell'orbita in km.
* **`inclination_deg`**: *(Decimale)* Inclinazione dell'orbita in gradi.
* **`mass_kg`**: *(Decimale)* Massa del singolo satellite in kg.
* **`cross_section_area_m2`**: *(Decimale)* Area della sezione trasversale in m².
* **`cd`**: *(Decimale)* Coefficiente di resistenza.
* **`cr`**: *(Decimale)* Coefficiente di riflettività.
* **`raans_deg`**: *(Array di decimali)* Valori dell'Ascensione Retta del Nodo Ascendente (RAAN) in gradi.
  > [!NOTE]
  > Il codice attuale del simulatore utilizza solo il primo valore dell'array (`raans_deg[0]`) per definire la RAAN del piano di tutti i satelliti generati.

```toml
[constellation.meo]
num_satellites = 4
altitude_km = 10000.0
inclination_deg = 55.0000
raans_deg = [0.0, 90.0, 180.0, 270.0]
mass_kg = 50.0
cross_section_area_m2 = 0.2500
cd = 0.00
cr = 1.20
```

### 4. `[constellation.geo]`
Definisce lo strato orbitale GEO (Geostationary Earth Orbit).
* **`num_satellites`**: *(Intero)* Numero di satelliti.
* **`altitude_km`**: *(Decimale)* Altitudine dell'orbita in km (tipicamente ~35786.0).
* **`inclination_deg`**: *(Decimale)* Inclinazione dell'orbita in gradi (tipicamente 0.0).
* **`mass_kg`**: *(Decimale)* Massa del satellite in kg.
* **`cross_section_area_m2`**: *(Decimale)* Area della sezione trasversale in m².
* **`cd`**: *(Decimale)* Coefficiente di resistenza.
* **`cr`**: *(Decimale)* Coefficiente di riflettività.
* **`longitudes_deg`**: *(Array di decimali)* Longitudini in gradi per i satelliti GEO.
  > [!NOTE]
  > Il software distribuisce i satelliti GEO uniformemente lungo l'equatore indipendentemente dai valori specificati in questo array, che viene comunque parsed ed esportato per mantenere la compatibilità.

```toml
[constellation.geo]
num_satellites = 3
longitudes_deg = [0.0, 60.0, -120.0]
altitude_km = 35786.0
inclination_deg = 0.0000
mass_kg = 200.0
cross_section_area_m2 = 1.5000
cd = 0.00
cr = 1.20
```

### 5. `[[ground.stations]]`
Definisce una singola stazione di terra. Questa sezione può essere ripetuta più volte per definire più stazioni.
* **`id`**: *(Stringa)* Identificatore unico della stazione (es. `"GS_SVA"`).
* **`name`**: *(Stringa)* Nome visualizzato della stazione (es. `"Svalbard"`).
* **`lat_deg`**: *(Decimale)* Latitudine geografica della stazione in gradi.
* **`lon_deg`**: *(Decimale)* Longitudine geografica della stazione in gradi.
* **`alt_m`**: *(Decimale)* Altitudine sul livello del mare della stazione in metri.
* **`downlink_nominal_gbps`**: *(Decimale o Stringa)* Banda nominale per il downlink in Gbps. Può essere espresso come valore decimale oppure tramite le stringhe speciali `"unlimited"`, `"inf"` o `"infinity"` per indicare una capacità infinita.

```toml
[[ground.stations]]
id = "GS_SVA"
name = "Svalbard"
lat_deg = 78.2307
lon_deg = 15.6472
alt_m = 130.0
downlink_nominal_gbps = "unlimited"

[[ground.stations]]
id = "GS_ZRH"
name = "Zurich"
lat_deg = 47.4647
lon_deg = 8.5492
alt_m = 400.0
downlink_nominal_gbps = 100.0
```

### 6. `[atmosphere]`
Definisce le impostazioni del modello atmosferico per la simulazione del tempo meteorologico e del relativo coefficiente di estinzione atmosferica per i collegamenti Terra-Spazio (SGL).
* **`states`**: *(Array di stringhe)* Elenco dei nomi degli stati meteorologici (es. `["clear", "thin_clouds", "thick_clouds", "heavy"]`).
* **`k_values_per_km`**: *(Array di decimali)* Coefficienti di estinzione atmosferica associati a ciascun stato in $1/\text{km}$.
* **`transition_matrix`**: *(Array di array)* Matrice delle probabilità di transizione per il modello di Markov meteorologico.
  > [!NOTE]
  > Il parser personalizzato salta l'elaborazione di `transition_matrix` durante l'importazione per scopi di robustezza del codice, e utilizza i valori predefiniti definiti nel software. Viene comunque salvata in fase di esportazione.

```toml
[atmosphere]
states = ["clear", "thin_clouds", "thick_clouds", "heavy"]
k_values_per_km = [0.05, 0.20, 1.50, 5.00]
transition_matrix = [
    [0.85, 0.10, 0.04, 0.01],
    [0.15, 0.70, 0.10, 0.05],
    [0.05, 0.15, 0.65, 0.15],
    [0.02, 0.08, 0.20, 0.70],
]
```

### 7. `[environment]`
Definisce i parametri fisici terrestri e orbitali.
* **`mu`**: *(Decimale)* Costante gravitazionale standard terrestre ($\mu = G \cdot M_\oplus$) in $\text{m}^3/\text{s}^2$.
* **`r_earth`**: *(Decimale)* Raggio medio equatoriale della Terra in metri.
* **`j2`**: *(Decimale)* Coefficiente zonale armonico $J_2$ (correzione per lo schiacciamento terrestre).
* **`rho0_500km`**: *(Decimale)* Densità atmosferica di riferimento all'altezza $h_0$ (500 km) in $\text{kg}/\text{m}^3$.
* **`h0_km`**: *(Decimale)* Altitudine di riferimento in km per il modello esponenziale di densità atmosferica.
* **`scale_height_km`**: *(Decimale)* Altezza di scala dell'atmosfera in km.
* **`p_srp`**: *(Decimale)* Pressione di radiazione solare in $\text{N}/\text{m}^2$.

```toml
[environment]
mu = 3.9860044180e14
r_earth = 6378137.0
j2 = 1.0826266800e-3
rho0_500km = 3.8000000000e-12
h0_km = 500.0
scale_height_km = 70.0
p_srp = 4.5600000000e-6
```

### 8. `[digital_twin]`
Contiene le impostazioni relative all'integrazione e alle metriche del motore di simulazione.
* **`time_step_s`**: *(Decimale)* Passo temporale di integrazione della fisica ($\Delta t$) in secondi (usato dal solutore Runge-Kutta 4).
* **`ref_distance_isl_km`**: *(Decimale)* Distanza di riferimento per il calcolo della capacità dei link Inter-Satellitari (ISL) in km.
* **`ref_distance_sgl_km`**: *(Decimale)* Distanza di riferimento per il calcolo della capacità dei link Spazio-Terra (SGL) in km.

```toml
[digital_twin]
time_step_s = 1.0
sim_duration_s = 86400.0
ref_distance_isl_km = 1000.0
ref_distance_sgl_km = 1000.0
```

---

## Parametri Esportati ma Ignorati all'Importazione

Quando si esporta una configurazione direttamente dall'interfaccia grafica (GUI), il software genera anche le seguenti sezioni. Tuttavia, queste sezioni **vengono ignorate** durante il caricamento e importazione del file `.toml` (il software le salta e utilizza i parametri cablati internamente per mantenere l'affidabilità):

* **`[adcs]`**: Contiene i valori di coppia massima delle reaction wheels (`rw_max_torque_nm`, `rw_max_momentum_nms`) e il momento di dipolo dei magnetorquer (`mtq_max_dipole_am2`).
* **`[sensors]`**: Contiene i parametri di rumore dei sensori giroscopici, magnetici, solari e star tracker.

---

## Esempio di File `config.toml` Completo e Funzionante

Puoi copiare e incollare il seguente testo per creare un file `config.toml` predefinito e perfettamente funzionante, compatibile con il caricamento iniziale e l'importazione manuale all'interno del software.

```toml
# ESA HydRON Digital Twin Config file

[constellation]
name = "HydRON-Like-Net"

[constellation.leo]
num_satellites = 10
altitude_km = 550.0
inclination_deg = 97.6000
mass_kg = 20.0
cross_section_area_m2 = 0.1000
cd = 2.20
cr = 1.20

[constellation.meo]
num_satellites = 4
altitude_km = 10000.0
inclination_deg = 55.0000
raans_deg = [0.0, 90.0, 180.0, 270.0]
mass_kg = 50.0
cross_section_area_m2 = 0.2500
cd = 0.00
cr = 1.20

[constellation.geo]
num_satellites = 3
longitudes_deg = [0.0, 60.0, -120.0]
altitude_km = 35786.0
inclination_deg = 0.0000
mass_kg = 200.0
cross_section_area_m2 = 1.5000
cd = 0.00
cr = 1.20

[ground]

[[ground.stations]]
id = "GS_SVA"
name = "Svalbard"
lat_deg = 78.2307
lon_deg = 15.6472
alt_m = 130.0
downlink_nominal_gbps = "unlimited"

[[ground.stations]]
id = "GS_ZRH"
name = "Zurich"
lat_deg = 47.4647
lon_deg = 8.5492
alt_m = 400.0
downlink_nominal_gbps = "unlimited"

[[ground.stations]]
id = "GS_REU"
name = "Reunion"
lat_deg = -20.9089
lon_deg = 55.5136
alt_m = 95.0
downlink_nominal_gbps = "unlimited"

[[ground.stations]]
id = "GS_MAU"
name = "Maui"
lat_deg = 20.7067
lon_deg = -156.2570
alt_m = 100.0
downlink_nominal_gbps = "unlimited"

[atmosphere]
states = ["clear", "thin", "thick", "heavy"]
k_values_per_km = [0.05, 0.20, 1.50, 5.00]
transition_matrix = [
    [0.85, 0.10, 0.04, 0.01],
    [0.15, 0.70, 0.10, 0.05],
    [0.05, 0.15, 0.65, 0.15],
    [0.02, 0.08, 0.20, 0.70],
]

[adcs]
rw_max_torque_nm = 0.01
rw_max_momentum_nms = 0.1
mtq_max_dipole_am2 = 0.2

[sensors]
gyro_bias_rad_s = [1e-5, 1e-5, 1e-5]
gyro_noise_rad_s = 1e-6
mag_noise_tesla = 1e-8
sun_noise_rad = 1e-3
star_tracker_noise_rad = 1e-4

[environment]
mu = 3.9860044180e14
r_earth = 6378137.0
j2 = 1.0826266800e-3
rho0_500km = 3.8000000000e-12
h0_km = 500.0
scale_height_km = 70.0
p_srp = 4.5600000000e-6

[digital_twin]
time_step_s = 1.0
sim_duration_s = 86400.0
ref_distance_isl_km = 1000.0
ref_distance_sgl_km = 1000.0
