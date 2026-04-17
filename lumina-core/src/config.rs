//! Configuration module - parses LLM/Gemini JSON intent and particle parameters

use serde::{Deserialize, Serialize};

/// Physics preset types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    CosmicDust,
    Rain,
    Snow,
    Fireflies,
    SunDust,
    Embers,
}

impl Default for Preset {
    fn default() -> Self {
        Self::CosmicDust
    }
}

impl Preset {
    /// Convert to shader index (0-5)
    pub fn to_shader_index(&self) -> u32 {
        match self {
            Preset::CosmicDust => 0,
            Preset::Rain => 1,
            Preset::Snow => 2,
            Preset::Fireflies => 3,
            Preset::SunDust => 4,
            Preset::Embers => 5,
        }
    }
}

/// Main input from LLM/Gemini
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayConfig {
    pub preset: Preset,
    pub params: ParticleParams,
}

/// Wrapper for nested JSON format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentWrapper {
    pub overlay_config: OverlayConfig,
}

/// Rain behavior type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RainType {
    Drizzle,  // Light, slow rain
    Normal,   // Regular rain
    Heavy,    // Dense, fast rain
    Storm,    // Very fast with gusts
}

impl Default for RainType {
    fn default() -> Self {
        Self::Normal
    }
}

/// Particle physics parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticleParams {
    /// Density multiplier (0.1 to 2.0)
    #[serde(default = "default_density")]
    pub density_multiplier: f32,
    
    /// Velocity range [min, max]
    #[serde(default = "default_velocity")]
    pub velocity_scale: [f32; 2],
    
    /// Base color in HEX format (#RRGGBB)
    #[serde(default = "default_color")]
    pub base_color_hex: String,
    
    /// Turbulence intensity (0.0 to 1.0)
    #[serde(default = "default_turbulence")]
    pub turbulence: f32,
    
    /// Flicker speed for light-emitting particles (0.0 to 2.0)
    #[serde(default)]
    pub flicker_speed: f32,
    
    /// Size range [min_px, max_px]
    #[serde(default = "default_size")]
    pub size_range: [f32; 2],
    
    /// Interaction logic description
    #[serde(default)]
    pub interaction_logic: Option<String>,
    
    // === RAIN-SPECIFIC PARAMETERS ===
    
    /// Wind direction in degrees (0 = down, 90 = right, 180 = up, 270 = left)
    #[serde(default = "default_wind_direction")]
    pub wind_direction: f32,
    
    /// Base wind strength (0.0 to 2.0)
    #[serde(default = "default_wind_strength")]
    pub wind_strength: f32,
    
    /// Enable wind gusts (sudden strong wind bursts)
    #[serde(default)]
    pub gust_enabled: bool,
    
    /// Gust frequency (0.0 to 1.0, higher = more frequent)
    #[serde(default = "default_gust_frequency")]
    pub gust_frequency: f32,
    
    /// Gust strength multiplier (1.0 to 3.0)
    #[serde(default = "default_gust_strength")]
    pub gust_strength: f32,
    
    /// Gust duration in seconds
    #[serde(default = "default_gust_duration")]
    pub gust_duration: f32,
    
    /// Rain type for preset behavior
    #[serde(default)]
    pub rain_type: Option<RainType>,
    
    /// Enable splash/bounce when rain hits bottom
    #[serde(default)]
    pub splash_enabled: bool,
    
    /// Splash velocity multiplier
    #[serde(default = "default_splash_velocity")]
    pub splash_velocity: f32,
}

fn default_color() -> String {
    "#FFFFFF".to_string()
}

fn default_velocity() -> [f32; 2] {
    [1.0, 3.0]
}

fn default_density() -> f32 {
    1.0
}

fn default_turbulence() -> f32 {
    0.1
}

fn default_size() -> [f32; 2] {
    [2.0, 6.0]
}

fn default_wind_direction() -> f32 {
    0.0 // Default: straight down
}

fn default_wind_strength() -> f32 {
    0.3
}

fn default_gust_frequency() -> f32 {
    0.15
}

fn default_gust_strength() -> f32 {
    2.0
}

fn default_gust_duration() -> f32 {
    0.8
}

fn default_splash_velocity() -> f32 {
    2.0
}

impl Default for ParticleParams {
    fn default() -> Self {
        Self {
            density_multiplier: default_density(),
            velocity_scale: default_velocity(),
            base_color_hex: default_color(),
            turbulence: default_turbulence(),
            flicker_speed: 0.0,
            size_range: default_size(),
            interaction_logic: None,
            wind_direction: default_wind_direction(),
            wind_strength: default_wind_strength(),
            gust_enabled: false,
            gust_frequency: default_gust_frequency(),
            gust_strength: default_gust_strength(),
            gust_duration: default_gust_duration(),
            rain_type: None,
            splash_enabled: false,
            splash_velocity: default_splash_velocity(),
        }
    }
}

/// Convert hex color to RGBA components (0.0-1.0)
pub fn hex_to_rgba(hex: &str) -> [f32; 4] {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 {
        return [1.0, 1.0, 1.0, 1.0];
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255) as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255) as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255) as f32 / 255.0;
    [r, g, b, 1.0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_rgba() {
        let rgba = hex_to_rgba("#FF0000");
        assert!((rgba[0] - 1.0).abs() < 0.01);
        assert!(rgba[1] < 0.01);
        assert!(rgba[2] < 0.01);
    }

    #[test]
    fn test_preset_index() {
        assert_eq!(Preset::Rain.to_shader_index(), 1);
        assert_eq!(Preset::Fireflies.to_shader_index(), 3);
        assert_eq!(Preset::Embers.to_shader_index(), 5);
    }
}
