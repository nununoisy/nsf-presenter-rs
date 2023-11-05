pub const DEFAULT_CONFIG: &str = r###"
[piano_roll]
draw_piano_strings = false
key_length = 24
key_thickness = 5
octave_count = 9
scale_factor = 1
speed_multiplier = 1
starting_octave = 0
waveform_height = 48
oscilloscope_glow_thickness = 2.0
oscilloscope_line_thickness = 0.75
"###;

pub const REQUIRED_CONFIG: &str = r###"
[piano_roll]
background_color = "rgba(0, 0, 0, 0)"
canvas_width = 960
canvas_height = 540

[piano_roll.settings.APU."Final Mix"]
hidden = true
"###;