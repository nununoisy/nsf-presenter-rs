// Standard Library
use std::ffi::OsString;
use std::fs;

// Third-party
use toml::Value;
use toml::map::Map;

// First-party
use events::Event;


const DEFAULT_CONFIG: &str = r###"
[video]
ntsc_filter = false
simulate_overscan = false
display_fps = false
scale_factor = 2

[piano_roll]
canvas_width = 1280
canvas_height = 720
draw_piano_strings = true
key_length = 32
key_thickness = 11
octave_count = 9
scale_factor = 1
speed_multiplier = 4
starting_octave = 0
waveform_height = 64

background_color = "rgba(0, 0, 0, 255)"

[piano_roll.settings.2A03.DMC]
static = "rgb(96, 32, 192)"

[piano_roll.settings.2A03.Noise]
mode0 = "rgb(192, 192, 192)"
mode1 = "rgb(128, 240, 255)"

[piano_roll.settings.2A03."Pulse 1"]
duty0 = "hsv(340, 25%, 100%)"
duty1 = "hsv(350, 55%, 100%)"
duty2 = "hsv(360, 75%, 100%)"
duty3 = "hsv(350, 55%, 100%)"

[piano_roll.settings.2A03."Pulse 2"]
duty0 = "hsv(40, 25%, 100%)"
duty1 = "hsv(50, 55%, 100%)"
duty2 = "hsv(60, 75%, 100%)"
duty3 = "hsv(50, 55%, 100%)"

[piano_roll.settings.2A03.Triangle]
static = "#40FF40"
[piano_roll.settings.MMC5.PCM]
static = "rgb(224, 24, 64)"

[piano_roll.settings.MMC5."Pulse 1"]
static = "rgb(224, 24, 64)"

[piano_roll.settings.MMC5."Pulse 2"]
static = "rgb(180, 12, 40)"
[piano_roll.settings.N163."NAMCO 1"]
gradient_high = "hsv(35, 22%, 80%)"
gradient_low = "hsv(5, 80%, 60%)"

[piano_roll.settings.N163."NAMCO 2"]
gradient_high = "hsv(35, 22%, 80%)"
gradient_low = "hsv(5, 80%, 60%)"

[piano_roll.settings.N163."NAMCO 3"]
gradient_high = "hsv(35, 22%, 80%)"
gradient_low = "hsv(5, 80%, 60%)"

[piano_roll.settings.N163."NAMCO 4"]
gradient_high = "hsv(35, 22%, 80%)"
gradient_low = "hsv(5, 80%, 60%)"

[piano_roll.settings.N163."NAMCO 5"]
gradient_high = "hsv(35, 22%, 80%)"
gradient_low = "hsv(5, 80%, 60%)"

[piano_roll.settings.N163."NAMCO 6"]
gradient_high = "hsv(35, 22%, 80%)"
gradient_low = "hsv(5, 80%, 60%)"

[piano_roll.settings.N163."NAMCO 7"]
gradient_high = "hsv(35, 22%, 80%)"
gradient_low = "hsv(5, 80%, 60%)"

[piano_roll.settings.N163."NAMCO 8"]
gradient_high = "hsv(35, 22%, 80%)"
gradient_low = "hsv(5, 80%, 60%)"
[piano_roll.settings.VRC6."Pulse 1"]
duty0 = "hsv(330, 22%, 94%)"
duty1 = "hsv(330, 32%, 86%)"
duty2 = "hsv(330, 42%, 78%)"
duty3 = "hsv(330, 52%, 70%)"
duty4 = "hsv(330, 63%, 62%)"
duty5 = "hsv(330, 73%, 54%)"
duty6 = "hsv(330, 83%, 46%)"
duty7 = "hsv(330, 93%, 48%)"

[piano_roll.settings.VRC6."Pulse 2"]
duty0 = "hsv(300, 22%, 94%)"
duty1 = "hsv(300, 32%, 86%)"
duty2 = "hsv(300, 42%, 78%)"
duty3 = "hsv(300, 52%, 70%)"
duty4 = "hsv(300, 63%, 62%)"
duty5 = "hsv(300, 73%, 54%)"
duty6 = "hsv(300, 83%, 46%)"
duty7 = "hsv(300, 93%, 48%)"

[piano_roll.settings.VRC6.Sawtooth]
mode0 = "#077d5a"
mode1 = "#9fb8ed"
[piano_roll.settings.YM2149F.A]
static = "rgb(32, 144, 204)"

[piano_roll.settings.YM2149F.B]
static = "rgb(24, 104, 228)"

[piano_roll.settings.YM2149F.C]
static = "rgb(16, 64, 248)"

[piano_roll.settings.APU."Final Mix"]
static = "rgb(224, 224, 224)"

"###;

pub struct SettingsState {
    pub root: Value
}

impl SettingsState {
    pub fn new() -> SettingsState {
        let default_config = DEFAULT_CONFIG.parse::<Value>().unwrap();
        return SettingsState {
            root: default_config
        }
    }

    pub fn load(&mut self, filename: &OsString) {
        match fs::read_to_string(filename) {
            Ok(config_str) => {
                let config_from_file = config_str.parse::<Value>().unwrap();
                self.root = config_from_file;
            },
            Err(_) => {
                println!("Failed to load settings from: {:?}", filename);
            }
        }
    }

    pub fn load_str(&mut self, config_str: &str) {
        let config = config_str.parse::<Value>().unwrap();
        self.root = config;
    }

    pub fn save(&self, filename: &OsString) {
        let config_str = toml::to_string(&self.root).unwrap();
        fs::write(filename, config_str).expect("Unable to write settings!");
        println!("Wrote settings to {:?}", filename);
    }

    fn _emit_events(value: Value, prefix: String) -> Vec<Event> {
        let mut events: Vec<Event> = Vec::new();
        match value {
            Value::Table(table) => {
                for key in table.keys() {
                    let new_prefix = if prefix == "" {key.to_string()} else {format!("{}.{}", prefix, key)};
                    events.extend(SettingsState::_emit_events(table[key].clone(), new_prefix));
                }
            },
            Value::Boolean(boolean_value) => {events.push(Event::ApplyBooleanSetting(prefix, boolean_value));},
            Value::Float(float_value) => {events.push(Event::ApplyFloatSetting(prefix, float_value));},
            Value::Integer(integer_value) => {events.push(Event::ApplyIntegerSetting(prefix, integer_value));},
            Value::String(string_value) => {events.push(Event::ApplyStringSetting(prefix, string_value));},
            _ => {
                /* Unimplemented! */
            }
        }
        return events;
    }

    pub fn apply_settings(&self) -> Vec<Event> {
        return SettingsState::_emit_events(self.root.clone(), "".to_string());
    }

    fn _ensure_path_exists(path: String, current_table: &mut Map<String, Value>, default_value: Value) {
        let components = path.split(".").collect::<Vec<&str>>();
        if components.len() == 1 {
            // This is the last path element. Either confirm the existence of this key
            // or create it with the default value.
            if current_table.contains_key(components[0]) {
                // we're done!
                return;
            } else {
                current_table.insert(components[0].to_string(), default_value);
            }
        } else {
            if !current_table.contains_key(components[0]) {
                current_table.insert(components[0].to_string(), Value::try_from(Map::new()).unwrap());
            }
            let child_table = current_table[components[0]].as_table_mut().unwrap();
            let remaining_path = components[1..].join(".");
            SettingsState::_ensure_path_exists(remaining_path, child_table, default_value);
        }
    }

    pub fn ensure_path_exists(&mut self, path: String, default_value: Value) {
        let root_table = self.root.as_table_mut().unwrap();
        SettingsState::_ensure_path_exists(path, root_table, default_value);
    }

    pub fn _get(path: String, current_table: &Map<String, Value>) -> Option<&Value> {
        let components = path.split(".").collect::<Vec<&str>>();
        if components.len() == 1 {
            if current_table.contains_key(components[0]) {
                return Some(&current_table[&components[0].to_string()]);
            }
        } else {
            if current_table.contains_key(components[0]) {
                let child = &current_table[&components[0].to_string()];
                if child.is_table() {
                    let child_table = current_table[components[0]].as_table().unwrap();
                    let remaining_path = components[1..].join(".");
                    return SettingsState::_get(remaining_path, child_table);
                }
            }
        }
        return None;
    }

    pub fn get(&self, path: String) -> Option<&Value> {
        let root_table = self.root.as_table().unwrap();
        return SettingsState::_get(path, root_table);
    }

    pub fn _set(path: String, current_table: &mut Map<String, Value>, new_value: Value) {
        let components = path.split(".").collect::<Vec<&str>>();
        if components.len() == 1 {
            if current_table.contains_key(components[0]) {
                current_table[&components[0].to_string()] = new_value;
            }
        } else {
            if current_table.contains_key(components[0]) {
                let child = &current_table[&components[0].to_string()];
                if child.is_table() {
                    let child_table = current_table[components[0]].as_table_mut().unwrap();
                    let remaining_path = components[1..].join(".");
                    SettingsState::_set(remaining_path, child_table, new_value);
                }
            }
        }
    }

    pub fn set(&mut self, path: String, new_value: Value) {
        let root_table = self.root.as_table_mut().unwrap();
        return SettingsState::_set(path, root_table, new_value);
    }

    pub fn handle_event(&mut self, event: Event) -> Vec<Event> {
        let mut events: Vec<Event> = Vec::new();
        match event {
            Event::StoreBooleanSetting(path, value) => {
                self.ensure_path_exists(path.clone(), Value::from(false));
                self.set(path.clone(), Value::from(value));
                events.push(Event::ApplyBooleanSetting(path, value));
            },
            Event::StoreFloatSetting(path, value) => {
                self.ensure_path_exists(path.clone(), Value::from(false));
                self.set(path.clone(), Value::from(value));
                events.push(Event::ApplyFloatSetting(path, value));
            },
            Event::StoreIntegerSetting(path, value) => {
                self.ensure_path_exists(path.clone(), Value::from(false));
                self.set(path.clone(), Value::from(value));
                events.push(Event::ApplyIntegerSetting(path, value));
            },
            Event::StoreStringSetting(path, value) => {
                self.ensure_path_exists(path.clone(), Value::from(false));
                self.set(path.clone(), Value::from(value.clone()));
                events.push(Event::ApplyStringSetting(path, value.clone()));
            },
            Event::ToggleBooleanSetting(path) => {
                self.ensure_path_exists(path.clone(), Value::from(false));
                let current_value = self.get(path.clone()).unwrap().as_bool().unwrap();
                self.set(path.clone(), Value::from(!current_value));
                events.push(Event::ApplyBooleanSetting(path, !current_value));
            },
            _ => {}
        }
        return events;
    }
}